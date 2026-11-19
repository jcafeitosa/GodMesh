//! Perfil QUIC selecionado pelo handler do enlace autenticado E↔R da fixture.

use super::{
    Arc, Future, Instant, NonZeroU32, OperationalSession, OwnedSemaphorePermit, RecvStream,
    STREAM_ABORTED, SendStream, SessionError, SessionState, before_deadline, checked_length,
    ensure_deadline, exact_error, read_error, valid_length, write_error,
};
use crate::relay_wire::{Control, Open, decode_control, validate_reply};
use std::time::Duration;
use tokio::sync::{Mutex, watch};

impl OperationalSession {
    /// Seleciona o perfil relay para esta stream de saída.
    pub async fn open_relay(
        &self,
        deadline: Instant,
        max_control_bytes: NonZeroU32,
    ) -> Result<RelayOpening, SessionError> {
        self.open_stream(deadline, |state, slot, send, recv, deadline| {
            RelayOpening::new(
                state,
                slot,
                send,
                recv,
                deadline,
                max_control_bytes,
                Role::Initiator,
            )
        })
        .await
    }

    /// Aceita uma stream relay e inicia o prazo antes da reautorização.
    pub async fn accept_relay(
        &self,
        opening_timeout: Duration,
        max_control_bytes: NonZeroU32,
    ) -> Result<RelayOpening, SessionError> {
        self.accept_stream(opening_timeout, |state, slot, send, recv, deadline| {
            RelayOpening::new(
                state,
                slot,
                send,
                recv,
                deadline,
                max_control_bytes,
                Role::Responder,
            )
        })
        .await
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Initiator,
    Responder,
}

enum OpeningPhase {
    Request,
    Reply(Open),
    Token { bytes: [u8; 16], cap: NonZeroU32 },
    Confirmed(NonZeroU32),
    Refused,
}

struct RouteStream {
    session: Arc<SessionState>,
    cancelled: watch::Sender<bool>,
    _slot: OwnedSemaphorePermit,
}

impl RouteStream {
    fn check(&self, deadline: Instant) -> Result<(), SessionError> {
        if *self.cancelled.borrow() {
            return Err(SessionError::Closed);
        }
        ensure_deadline(deadline)?;
        self.session.check_ready()
    }

    async fn io<T>(
        &self,
        deadline: Instant,
        operation: impl Future<Output = Result<T, SessionError>>,
    ) -> Result<T, SessionError> {
        self.check(deadline)?;
        let mut cancelled = self.cancelled.subscribe();
        let result = tokio::select! {
            biased;
            _ = cancelled.wait_for(|value| *value) => Err(SessionError::Closed),
            _ = self.session.connection().closed() => Err(SessionError::Closed),
            result = before_deadline(deadline, operation) => result?,
        };
        self.check(deadline)?;
        result
    }

    fn cancel(&self) {
        self.cancelled.send_replace(true);
    }
}

/// Sinal de cancelamento da instância, sem reter o crédito QUIC.
#[derive(Clone)]
pub struct RelayCancellation(watch::Sender<bool>);

impl RelayCancellation {
    /// Acorda as operações pendentes nas duas direções.
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }

    /// Permite que tarefas ociosas também observem o cancelamento da rota.
    pub async fn cancelled(&self) {
        let _ = self.0.subscribe().wait_for(|value| *value).await;
    }
}

/// Controle e token na mesma stream, sob o prazo absoluto de abertura.
pub struct RelayOpening {
    send: RelaySend,
    recv: RelayRecv,
    deadline: Instant,
    role: Role,
    phase: OpeningPhase,
}

impl RelayOpening {
    fn new(
        session: &Arc<SessionState>,
        slot: OwnedSemaphorePermit,
        send: SendStream,
        recv: RecvStream,
        deadline: Instant,
        cap: NonZeroU32,
        role: Role,
    ) -> Self {
        let route = Arc::new(RouteStream {
            session: Arc::clone(session),
            cancelled: watch::channel(false).0,
            _slot: slot,
        });
        Self {
            send: RelaySend {
                route: Arc::clone(&route),
                stream: Arc::new(Mutex::new(send)),
                cap,
                finished: false,
            },
            recv: RelayRecv {
                route,
                stream: Arc::new(Mutex::new(recv)),
                cap,
                finished: false,
            },
            deadline,
            role,
            phase: OpeningPhase::Request,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Recebe o controle esperado para o papel e a fase atuais.
    pub async fn read_control(&mut self) -> Result<Vec<u8>, SessionError> {
        self.require_control_direction(Role::Responder)?;
        let body = self.recv.read_datagram(self.deadline).await?;
        self.phase = self.next_control(&body)?;
        Ok(body)
    }

    /// Escreve o controle esperado para o papel e a fase atuais.
    pub async fn write_control(&mut self, body: &[u8]) -> Result<(), SessionError> {
        self.require_control_direction(Role::Initiator)?;
        let next = self.next_control(body)?;
        self.send.write_datagram(body, self.deadline).await?;
        self.phase = next;
        Ok(())
    }

    fn require_control_direction(&mut self, request_role: Role) -> Result<(), SessionError> {
        let allowed = match self.phase {
            OpeningPhase::Request => self.role == request_role,
            OpeningPhase::Reply(_) => self.role != request_role,
            _ => false,
        };
        if allowed { Ok(()) } else { Err(self.invalid()) }
    }

    fn next_control(&mut self, body: &[u8]) -> Result<OpeningPhase, SessionError> {
        match self.decode_next_control(body) {
            Some(phase) => Ok(phase),
            None => Err(self.invalid()),
        }
    }

    fn decode_next_control(&self, body: &[u8]) -> Option<OpeningPhase> {
        let control = decode_control(body, self.send.cap).ok()?;
        match (&self.phase, control) {
            (OpeningPhase::Request, Control::Open(request)) => Some(OpeningPhase::Reply(request)),
            (OpeningPhase::Reply(request), reply) => {
                let cap = match &reply {
                    Control::Accept(accept) => accept.max_datagram_bytes,
                    _ => self.send.cap, // O codec só compara o teto em Accept.
                };
                validate_reply(request, &reply, cap).ok()?;
                match reply {
                    Control::Accept(accept) => Some(OpeningPhase::Token {
                        bytes: accept.token_bytes().ok()?,
                        cap,
                    }),
                    Control::Refuse(_) => Some(OpeningPhase::Refused),
                    Control::Open(_) => None,
                }
            }
            _ => None,
        }
    }

    /// Envia a confirmação do aceite correlacionado no lado solicitante.
    pub async fn send_token(&mut self, token: [u8; 16]) -> Result<(), SessionError> {
        let cap = self.require_token(Role::Initiator, token)?;
        self.send.write(&token, None, self.deadline).await?;
        self.phase = OpeningPhase::Confirmed(cap);
        Ok(())
    }

    /// Confere os 16 bytes do aceite já enviado pelo relay.
    pub async fn expect_token(&mut self, expected: [u8; 16]) -> Result<(), SessionError> {
        let cap = self.require_token(Role::Responder, expected)?;
        let mut token = [0; 16];
        let result = self
            .recv
            .route
            .io(self.deadline, async {
                self.recv
                    .stream
                    .lock()
                    .await
                    .read_exact(&mut token)
                    .await
                    .map_err(exact_error)?;
                (token == expected)
                    .then_some(())
                    .ok_or(SessionError::InvalidFrame)
            })
            .await;
        if let Err(error) = result {
            self.send.abort();
            self.recv.abort();
            return Err(error);
        }
        self.phase = OpeningPhase::Confirmed(cap);
        Ok(())
    }

    fn require_token(
        &mut self,
        role: Role,
        expected: [u8; 16],
    ) -> Result<NonZeroU32, SessionError> {
        if self.role == role
            && let OpeningPhase::Token { bytes, cap } = self.phase
            && bytes == expected
        {
            return Ok(cap);
        }
        Err(self.invalid())
    }

    /// Emite FIN após uma recusa. Descartar a abertura preserva esse FIN.
    pub fn finish_send(&mut self) -> Result<(), SessionError> {
        if self.role != Role::Responder || !matches!(self.phase, OpeningPhase::Refused) {
            return Err(self.invalid());
        }
        self.send.route.check(self.deadline)?;
        self.send
            .stream
            .try_lock()
            .map_err(|_| SessionError::Transport)?
            .finish()
            .map_err(|_| SessionError::Transport)?;
        self.send.finished = true;
        Ok(())
    }

    /// Valida o FIN da recusa sem aceitar bytes adicionais.
    pub async fn expect_fin(&mut self) -> Result<(), SessionError> {
        if self.role != Role::Initiator || !matches!(self.phase, OpeningPhase::Refused) {
            return Err(self.invalid());
        }
        let result = self
            .recv
            .route
            .io(self.deadline, async {
                match self
                    .recv
                    .stream
                    .lock()
                    .await
                    .read(&mut [0])
                    .await
                    .map_err(read_error)?
                {
                    None => Ok(()),
                    Some(_) => Err(SessionError::InvalidFrame),
                }
            })
            .await;
        self.recv.handle_result(result)?;
        self.recv.finished = true;
        Ok(())
    }

    /// Chamar após revalidar autoridade e reserva sob a proteção da rota.
    /// A partir daqui cada operação usa somente seu próprio prazo absoluto.
    pub fn into_forwarding(
        mut self,
        max_datagram_bytes: NonZeroU32,
    ) -> Result<(RelaySend, RelayRecv, RelayCancellation), SessionError> {
        if !matches!(self.phase, OpeningPhase::Confirmed(cap) if cap == max_datagram_bytes) {
            return Err(self.invalid());
        }
        self.send.route.check(self.deadline)?;
        self.send.cap = max_datagram_bytes;
        self.recv.cap = max_datagram_bytes;
        let cancellation = RelayCancellation(self.send.route.cancelled.clone());
        let supervisor = cancellation.clone();
        let connection = self.send.route.session.connection().clone();
        let send = Arc::clone(&self.send.stream);
        let recv = Arc::clone(&self.recv.stream);
        tokio::spawn(async move {
            tokio::select! {
                _ = supervisor.cancelled() => {}
                _ = connection.closed() => supervisor.cancel(),
            }
            let _ = send.lock().await.reset(STREAM_ABORTED.into());
            let _ = recv.lock().await.stop(STREAM_ABORTED.into());
        });
        Ok((self.send, self.recv, cancellation))
    }

    fn invalid(&mut self) -> SessionError {
        self.send.abort();
        self.recv.abort();
        SessionError::InvalidFrame
    }
}

/// Direção de envio. Descartar após cancelamento parcial de uma future de I/O.
pub struct RelaySend {
    route: Arc<RouteStream>,
    stream: Arc<Mutex<SendStream>>,
    cap: NonZeroU32,
    finished: bool,
}

impl RelaySend {
    pub async fn write_datagram(
        &mut self,
        body: &[u8],
        deadline: Instant,
    ) -> Result<(), SessionError> {
        let length = checked_length(body, self.cap.get(), None)?;
        self.write(body, Some(length.to_be_bytes()), deadline).await
    }

    async fn write(
        &mut self,
        body: &[u8],
        prefix: Option<[u8; 4]>,
        deadline: Instant,
    ) -> Result<(), SessionError> {
        let result = self
            .route
            .io(deadline, async {
                let mut stream = self.stream.lock().await;
                if let Some(prefix) = prefix {
                    stream.write_all(&prefix).await.map_err(write_error)?;
                }
                stream.write_all(body).await.map_err(write_error)
            })
            .await;
        if result.is_err() {
            self.abort();
        }
        result
    }

    fn abort(&mut self) {
        self.route.cancel();
        if !self.finished
            && let Ok(mut stream) = self.stream.try_lock()
        {
            let _ = stream.reset(STREAM_ABORTED.into());
        }
    }
}

impl Drop for RelaySend {
    fn drop(&mut self) {
        self.abort();
    }
}

/// Direção de recepção. Descartar após cancelamento parcial de uma future de I/O.
pub struct RelayRecv {
    route: Arc<RouteStream>,
    stream: Arc<Mutex<RecvStream>>,
    cap: NonZeroU32,
    finished: bool,
}

impl RelayRecv {
    pub async fn read_datagram(&mut self, deadline: Instant) -> Result<Vec<u8>, SessionError> {
        let result = self
            .route
            .io(deadline, async {
                let mut stream = self.stream.lock().await;
                let mut prefix = [0; 4];
                stream.read_exact(&mut prefix).await.map_err(exact_error)?;
                let length = u32::from_be_bytes(prefix);
                if !valid_length(length, self.cap.get(), None) {
                    return Err(SessionError::InvalidFrame);
                }
                let size = usize::try_from(length).map_err(|_| SessionError::InvalidFrame)?;
                let mut body = vec![0; size];
                stream.read_exact(&mut body).await.map_err(exact_error)?;
                Ok(body)
            })
            .await;
        self.handle_result(result)
    }

    fn handle_result<T>(&mut self, result: Result<T, SessionError>) -> Result<T, SessionError> {
        if result.is_err() {
            self.abort();
        }
        result
    }

    fn abort(&mut self) {
        self.route.cancel();
        if !self.finished
            && let Ok(mut stream) = self.stream.try_lock()
        {
            let _ = stream.stop(STREAM_ABORTED.into());
        }
    }
}

impl Drop for RelayRecv {
    fn drop(&mut self) {
        self.abort();
    }
}
