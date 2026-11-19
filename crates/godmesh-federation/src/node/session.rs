//! Streams de aplicação de uma conexão promovida a operacional.

use std::fmt;
use std::future::Future;
use std::num::{NonZeroU32, NonZeroUsize};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use quinn::{Connection, ReadError, ReadExactError, RecvStream, SendStream, VarInt, WriteError};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::{Instant, sleep_until, timeout_at};

use super::{
    AUTHORIZATION_TIMEOUT, LinkAuthorizationError, LinkIdentity, OperationalLink,
    close_with_rejection,
};
use crate::handshake::Rejection;
use crate::link::PeerLinks;

// Faixa de controle de streams, separada dos motivos de fechamento da conexão.
const STREAM_BUSY: u32 = 0x474D_0101;
const STREAM_ABORTED: u32 = 0x474D_0102;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Limites locais do dispatcher pós-manifesto.
#[derive(Debug, Clone, Copy)]
pub struct SessionLimits {
    /// Orçamento da tentativa de entrada, contado a partir do aceite da stream.
    pub attempt_timeout: Duration,
    /// Tamanho máximo do corpo, sem o prefixo de comprimento.
    pub max_frame_bytes: NonZeroU32,
    /// Cota total de streams abertas e aceitas por conexão.
    pub max_streams_per_connection: NonZeroUsize,
}

/// Identifica uma conexão operacional durante a vida do processo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u64);

/// Callback invocado uma vez para cada conexão promovida a operacional.
pub trait OperationalSessionHandler: Send + Sync {
    /// O retorno do callback não encerra o enlace. O trabalho vinculado à sessão
    /// deve observar o cancelamento e concluir sua limpeza cooperativamente.
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// Resultado de admitir ou operar uma stream da sessão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// Cota local ou remota esgotada.
    Busy,
    /// Prazo esgotado nesta stream, sem invalidar a conexão.
    Timeout,
    /// Sessão encerrada ou sem autorização operacional.
    Closed,
    /// Enquadramento incompleto, excedente ou inválido.
    InvalidFrame,
    /// Outra falha de transporte da stream.
    Transport,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "cota de streams esgotada",
            Self::Timeout => "prazo da tentativa esgotado",
            Self::Closed => "sessão operacional encerrada",
            Self::InvalidFrame => "frame de sessão inválido",
            Self::Transport => "falha no transporte da stream",
        })
    }
}

impl std::error::Error for SessionError {}

struct SessionState {
    id: SessionId,
    link: Arc<OperationalLink>,
    links: Arc<PeerLinks>,
    max_frame_bytes: u32,
    attempt_timeout: Duration,
    slots: Arc<Semaphore>,
    accept: Mutex<()>,
}

impl SessionState {
    fn connection(&self) -> &Connection {
        &self.link.validity.connection
    }

    fn check_ready(&self) -> Result<(), SessionError> {
        self.links
            .operational_is_ready(&self.link.remote.node_id, self.connection())
            .then_some(())
            .ok_or(SessionError::Closed)
    }

    async fn authorize(&self, deadline: Instant) -> Result<(), SessionError> {
        self.check_ready()?;
        let deadline = deadline.min(Instant::now() + AUTHORIZATION_TIMEOUT);
        ensure_deadline(deadline)?;
        let connection = self.connection();
        let result = tokio::select! {
            biased;
            _ = connection.closed() => return Err(SessionError::Closed),
            _ = sleep_until(deadline) => return Err(SessionError::Timeout),
            result = self.link.authorizer.authorize_until(&self.link.local, &self.link.remote, deadline) => result,
        };
        // Um resultado pronto depois do prazo não autoriza nem invalida o enlace.
        ensure_deadline(deadline)?;
        if matches!(result, Err(LinkAuthorizationError::Timeout)) {
            return Err(SessionError::Timeout);
        }
        let _permit = result
            .map_err(|_| Rejection::NotAuthorized)
            .and_then(|permit| self.link.keep_permit(permit))
            .map_err(|rejection| {
                close_with_rejection(connection, rejection);
                SessionError::Closed
            })?;
        self.check_ready()
    }

    fn slot(&self) -> Result<OwnedSemaphorePermit, SessionError> {
        Arc::clone(&self.slots)
            .try_acquire_owned()
            .map_err(|_| SessionError::Busy)
    }
}

/// Capacidade de I/O vinculada a uma única conexão autenticada.
pub struct OperationalSession {
    state: Arc<SessionState>,
}

impl OperationalSession {
    pub(super) fn new(
        link: Arc<OperationalLink>,
        links: Arc<PeerLinks>,
        limits: SessionLimits,
    ) -> Option<Self> {
        let id = NEXT_SESSION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .ok()?;
        let state = Arc::new(SessionState {
            id: SessionId(id),
            link,
            links,
            max_frame_bytes: limits.max_frame_bytes.get(),
            attempt_timeout: limits.attempt_timeout,
            slots: Arc::new(Semaphore::new(limits.max_streams_per_connection.get())),
            accept: Mutex::new(()),
        });
        state.check_ready().ok()?;
        Some(Self { state })
    }

    /// ID opaco para associar créditos e trabalho a esta conexão.
    pub fn id(&self) -> SessionId {
        self.state.id
    }

    /// Identidade autenticada do nó local.
    pub fn local(&self) -> &LinkIdentity {
        &self.state.link.local
    }

    /// Identidade do par derivada da chave autenticada pelo mTLS.
    pub fn remote(&self) -> &LinkIdentity {
        &self.state.link.remote
    }

    /// ALPN efetivo, conferido antes da promoção operacional.
    pub fn negotiated_alpn(&self) -> &str {
        &self.state.link.alpn
    }

    /// Aguarda a invalidação ou o fechamento desta conexão.
    pub async fn cancelled(&self) {
        self.state.connection().closed().await;
    }

    /// Abre uma stream usando o prazo iniciado pelo chamador antes do pull.
    pub async fn open_bi(&self, deadline: Instant) -> Result<OperationalBiStream, SessionError> {
        self.state.check_ready()?;
        ensure_deadline(deadline)?;
        let slot = self.state.slot()?;
        self.state.authorize(deadline).await?;
        ensure_deadline(deadline)?;
        let (send, recv) = timeout_at(deadline, self.state.connection().open_bi())
            .await
            .map_err(|_| SessionError::Timeout)?
            .map_err(|_| SessionError::Closed)?;
        let stream = OperationalBiStream::new(&self.state, slot, send, recv, deadline);
        self.state.check_ready()?;
        ensure_deadline(deadline)?;
        Ok(stream)
    }

    /// Retira uma stream da fila pós-manifesto e confere autorização antes da
    /// entrega. Em saturação, encerra só a stream com o código de cota.
    pub async fn accept_bi(&self) -> Result<OperationalBiStream, SessionError> {
        let _accept = self.state.accept.lock().await;
        self.state.check_ready()?;
        let (mut send, mut recv) = self
            .state
            .connection()
            .accept_bi()
            .await
            .map_err(|_| SessionError::Closed)?;
        let Some(deadline) = Instant::now().checked_add(self.state.attempt_timeout) else {
            abort_stream(&mut send, &mut recv, STREAM_ABORTED);
            return Err(SessionError::Timeout);
        };
        let slot = match self.state.slot() {
            Ok(slot) => slot,
            Err(error) => {
                abort_stream(&mut send, &mut recv, STREAM_BUSY);
                return Err(error);
            }
        };
        let stream = OperationalBiStream::new(&self.state, slot, send, recv, deadline);
        self.state.authorize(deadline).await?;
        ensure_deadline(deadline)?;
        Ok(stream)
    }
}

/// Stream bidirecional com crédito próprio e enquadramento u32 big-endian.
///
/// Se uma operação de I/O for cancelada parcialmente, descarte a stream.
/// Descartar preserva FINs confirmados localmente e cancela direções abertas.
pub struct OperationalBiStream {
    state: Arc<SessionState>,
    slot: Option<OwnedSemaphorePermit>,
    send: SendStream,
    recv: RecvStream,
    sent_fin: bool,
    received_fin: bool,
    deadline: Instant,
}

impl OperationalBiStream {
    fn new(
        state: &Arc<SessionState>,
        slot: OwnedSemaphorePermit,
        send: SendStream,
        recv: RecvStream,
        deadline: Instant,
    ) -> Self {
        Self {
            state: Arc::clone(state),
            slot: Some(slot),
            send,
            recv,
            sent_fin: false,
            received_fin: false,
            deadline,
        }
    }

    /// Prazo absoluto da tentativa, incluindo admissão e I/O.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    fn timed_out(&mut self) -> SessionError {
        self.abort();
        SessionError::Timeout
    }

    fn check_open(&mut self) -> Result<(), SessionError> {
        if self.slot.is_none() {
            return Err(SessionError::Closed);
        }
        self.state.check_ready()?;
        ensure_deadline(self.deadline).map_err(|_| self.timed_out())
    }

    fn incoming_error(&mut self, error: SessionError) -> SessionError {
        if error == SessionError::InvalidFrame {
            self.abort();
        }
        error
    }

    fn abort(&mut self) {
        if self.slot.is_some() {
            if !self.sent_fin {
                let _ = self.send.reset(STREAM_ABORTED.into());
            }
            if !self.received_fin {
                let _ = self.recv.stop(STREAM_ABORTED.into());
            }
            self.slot = None;
        }
    }

    /// Lê um corpo limitado. O FIN permanece para validação explícita.
    pub async fn read_frame(&mut self) -> Result<Vec<u8>, SessionError> {
        self.check_open()?;
        let mut header = [0; 4];
        before_deadline(self.deadline, self.recv.read_exact(&mut header))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(|error| self.incoming_error(exact_error(error)))?;
        self.check_open()?;
        let length = u32::from_be_bytes(header);
        if length == 0 || length > self.state.max_frame_bytes {
            return Err(self.incoming_error(SessionError::InvalidFrame));
        }
        self.check_open()?;
        let size = usize::try_from(length).map_err(|_| SessionError::InvalidFrame)?;
        let mut body = vec![0; size];
        before_deadline(self.deadline, self.recv.read_exact(&mut body))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(|error| self.incoming_error(exact_error(error)))?;
        self.check_open()?;
        Ok(body)
    }

    /// Escreve um corpo com prefixo, deixando o envio aberto.
    pub async fn write_frame(&mut self, body: &[u8]) -> Result<(), SessionError> {
        self.check_open()?;
        let length = u32::try_from(body.len()).map_err(|_| SessionError::InvalidFrame)?;
        if length == 0 || length > self.state.max_frame_bytes {
            return Err(SessionError::InvalidFrame);
        }
        before_deadline(self.deadline, self.send.write_all(&length.to_be_bytes()))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(write_error)?;
        self.check_open()?;
        before_deadline(self.deadline, self.send.write_all(body))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(write_error)?;
        self.check_open()
    }

    /// Encerra somente a direção de envio com FIN.
    pub fn finish_send(&mut self) -> Result<(), SessionError> {
        self.check_open()?;
        self.send.finish().map_err(|_| SessionError::Transport)?;
        self.sent_fin = true;
        Ok(())
    }

    /// Exige FIN na direção de recepção, sem aceitar bytes adicionais.
    pub async fn expect_fin(&mut self) -> Result<(), SessionError> {
        self.check_open()?;
        if before_deadline(self.deadline, self.recv.read(&mut [0]))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(read_error)?
            .is_some()
        {
            return Err(self.incoming_error(SessionError::InvalidFrame));
        }
        self.received_fin = true;
        self.check_open()
    }
}

impl Drop for OperationalBiStream {
    fn drop(&mut self) {
        self.abort();
    }
}

async fn before_deadline<T>(
    deadline: Instant,
    operation: impl Future<Output = T>,
) -> Result<T, SessionError> {
    let result = timeout_at(deadline, operation)
        .await
        .map_err(|_| SessionError::Timeout)?;
    ensure_deadline(deadline)?;
    Ok(result)
}

fn ensure_deadline(deadline: Instant) -> Result<(), SessionError> {
    (Instant::now() < deadline)
        .then_some(())
        .ok_or(SessionError::Timeout)
}

fn abort_stream(send: &mut SendStream, recv: &mut RecvStream, code: u32) {
    let _ = send.reset(code.into());
    let _ = recv.stop(code.into());
}

fn exact_error(error: ReadExactError) -> SessionError {
    match error {
        ReadExactError::FinishedEarly(_) => SessionError::InvalidFrame,
        ReadExactError::ReadError(error) => read_error(error),
    }
}

fn read_error(error: ReadError) -> SessionError {
    match error {
        ReadError::Reset(code) if code == VarInt::from(STREAM_BUSY) => SessionError::Busy,
        ReadError::ConnectionLost(_) => SessionError::Closed,
        _ => SessionError::Transport,
    }
}

fn write_error(error: WriteError) -> SessionError {
    match error {
        WriteError::Stopped(code) if code == VarInt::from(STREAM_BUSY) => SessionError::Busy,
        WriteError::ConnectionLost(_) => SessionError::Closed,
        _ => SessionError::Transport,
    }
}
