//! Perfil QUIC selecionado pelo handler do enlace autenticado E↔R da fixture.

use super::{
    Arc, Future, Instant, NonZeroU32, OperationalSession, OwnedSemaphorePermit, RecvStream,
    STREAM_ABORTED, SendStream, SessionError, SessionState, before_deadline, checked_length,
    ensure_deadline, exact_error, read_error, valid_length, write_error,
};
use std::time::Duration;
use tokio::sync::watch;

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
                Token::Send,
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
                Token::Expect,
            )
        })
        .await
    }
}

#[derive(PartialEq, Eq)]
enum Token {
    Send,
    Expect,
    Confirmed,
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
    token: Token,
}

impl RelayOpening {
    fn new(
        session: &Arc<SessionState>,
        slot: OwnedSemaphorePermit,
        send: SendStream,
        recv: RecvStream,
        deadline: Instant,
        cap: NonZeroU32,
        token: Token,
    ) -> Self {
        let route = Arc::new(RouteStream {
            session: Arc::clone(session),
            cancelled: watch::channel(false).0,
            _slot: slot,
        });
        Self {
            send: RelaySend {
                route: Arc::clone(&route),
                stream: send,
                cap,
                finished: false,
            },
            recv: RelayRecv {
                route,
                stream: recv,
                cap,
                finished: false,
            },
            deadline,
            token,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub async fn read_control(&mut self) -> Result<Vec<u8>, SessionError> {
        self.require_control()?;
        self.recv.read_datagram(self.deadline).await
    }

    pub async fn write_control(&mut self, body: &[u8]) -> Result<(), SessionError> {
        self.require_control()?;
        self.send.write_datagram(body, self.deadline).await
    }

    /// Envia a confirmação no lado que abriu a stream.
    pub async fn send_token(&mut self, token: [u8; 16]) -> Result<(), SessionError> {
        self.require_token(Token::Send)?;
        self.send.write(&token, None, self.deadline).await?;
        self.token = Token::Confirmed;
        Ok(())
    }

    /// Confere exatamente os 16 bytes de confirmação no lado que aceitou.
    pub async fn expect_token(&mut self, expected: [u8; 16]) -> Result<(), SessionError> {
        self.require_token(Token::Expect)?;
        let mut token = [0; 16];
        let deadline = self.deadline;
        let route = &self.recv.route;
        let result = route
            .io(deadline, async {
                self.recv
                    .stream
                    .read_exact(&mut token)
                    .await
                    .map_err(exact_error)?;
                (token == expected)
                    .then_some(())
                    .ok_or(SessionError::InvalidFrame)
            })
            .await;
        self.recv.handle_result(result)?;
        self.token = Token::Confirmed;
        Ok(())
    }

    /// Emite FIN após uma recusa. Descartar a abertura preserva esse FIN.
    pub fn finish_send(&mut self) -> Result<(), SessionError> {
        self.require_control()?;
        self.send.route.check(self.deadline)?;
        self.send
            .stream
            .finish()
            .map_err(|_| SessionError::Transport)?;
        self.send.finished = true;
        Ok(())
    }

    /// Valida o FIN da recusa sem aceitar bytes adicionais.
    pub async fn expect_fin(&mut self) -> Result<(), SessionError> {
        self.require_control()?;
        let result = self
            .recv
            .route
            .io(self.deadline, async {
                match self.recv.stream.read(&mut [0]).await.map_err(read_error)? {
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
        self.require_token(Token::Confirmed)?;
        self.send.route.check(self.deadline)?;
        self.send.cap = max_datagram_bytes;
        self.recv.cap = max_datagram_bytes;
        let cancellation = RelayCancellation(self.send.route.cancelled.clone());
        Ok((self.send, self.recv, cancellation))
    }

    fn require_token(&mut self, expected: Token) -> Result<(), SessionError> {
        if self.token == expected && !self.send.finished && !self.recv.finished {
            return Ok(());
        }
        self.send.abort();
        self.recv.abort();
        Err(SessionError::InvalidFrame)
    }

    fn require_control(&mut self) -> Result<(), SessionError> {
        if self.token != Token::Confirmed && !self.send.finished && !self.recv.finished {
            return Ok(());
        }
        self.send.abort();
        self.recv.abort();
        Err(SessionError::InvalidFrame)
    }
}

/// Direção de envio. Descartar após cancelamento parcial de uma future de I/O.
pub struct RelaySend {
    route: Arc<RouteStream>,
    stream: SendStream,
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
                if let Some(prefix) = prefix {
                    self.stream.write_all(&prefix).await.map_err(write_error)?;
                }
                self.stream.write_all(body).await.map_err(write_error)
            })
            .await;
        if result.is_err() {
            self.abort();
        }
        result
    }

    fn abort(&mut self) {
        self.route.cancel();
        if !self.finished {
            let _ = self.stream.reset(STREAM_ABORTED.into());
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
    stream: RecvStream,
    cap: NonZeroU32,
    finished: bool,
}

impl RelayRecv {
    pub async fn read_datagram(&mut self, deadline: Instant) -> Result<Vec<u8>, SessionError> {
        let result = self
            .route
            .io(deadline, async {
                let mut prefix = [0; 4];
                self.stream
                    .read_exact(&mut prefix)
                    .await
                    .map_err(exact_error)?;
                let length = u32::from_be_bytes(prefix);
                if !valid_length(length, self.cap.get(), None) {
                    return Err(SessionError::InvalidFrame);
                }
                let size = usize::try_from(length).map_err(|_| SessionError::InvalidFrame)?;
                let mut body = vec![0; size];
                self.stream
                    .read_exact(&mut body)
                    .await
                    .map_err(exact_error)?;
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
        if !self.finished {
            let _ = self.stream.stop(STREAM_ABORTED.into());
        }
    }
}

impl Drop for RelayRecv {
    fn drop(&mut self) {
        self.abort();
    }
}
