//! Streams de aplicação de uma conexão promovida a operacional.

#[cfg(feature = "relay-harness")]
mod relay;
#[cfg(feature = "relay-harness")]
pub(super) use relay::RelayValidity;
#[cfg(feature = "relay-harness")]
pub use relay::{RelayCancellation, RelayOpening, RelayRecv, RelaySend};

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

/// Limites da fixture com snapshot. O teto A2 permanece em `a2.max_frame_bytes`.
#[cfg(feature = "snapshot-harness")]
#[derive(Debug, Clone, Copy)]
pub struct SnapshotSessionLimits {
    pub a2: SessionLimits,
    pub max_session_frame_bytes: NonZeroU32,
    pub max_control_bytes: NonZeroU32,
    pub max_snapshot_bytes: NonZeroU32,
}

#[cfg(feature = "snapshot-harness")]
impl SnapshotSessionLimits {
    fn control_cap(self) -> u32 {
        self.max_control_bytes
            .min(self.max_session_frame_bytes)
            .get()
    }

    fn body_cap(self, negotiated_max: NonZeroU32) -> u32 {
        self.max_snapshot_bytes
            .min(self.max_session_frame_bytes)
            .min(negotiated_max)
            .get()
    }
}

/// Primeiro frame já lido pelo dispatcher após a promoção.
#[cfg(feature = "snapshot-harness")]
#[derive(Debug, PartialEq, Eq)]
pub enum StreamOpening {
    A2(Vec<u8>),
    Snapshot(Vec<u8>),
}

#[cfg(feature = "snapshot-harness")]
#[derive(PartialEq, Eq)]
enum StreamProfile {
    Unselected,
    A2,
    Snapshot,
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
    #[cfg(feature = "snapshot-harness")]
    snapshot: Option<SnapshotSessionLimits>,
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
        #[cfg(feature = "snapshot-harness")] snapshot: Option<SnapshotSessionLimits>,
    ) -> Option<Self> {
        let id = NEXT_SESSION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .ok()?;
        let state = Arc::new(SessionState {
            id: SessionId(id),
            link,
            links,
            max_frame_bytes: limits.max_frame_bytes.get(),
            #[cfg(feature = "snapshot-harness")]
            snapshot,
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
        self.open_stream(deadline, OperationalBiStream::new).await
    }

    async fn open_stream<T>(
        &self,
        deadline: Instant,
        wrap: impl FnOnce(
            &Arc<SessionState>,
            OwnedSemaphorePermit,
            SendStream,
            RecvStream,
            Instant,
        ) -> T,
    ) -> Result<T, SessionError> {
        self.state.check_ready()?;
        ensure_deadline(deadline)?;
        let slot = self.state.slot()?;
        self.state.authorize(deadline).await?;
        ensure_deadline(deadline)?;
        let (send, recv) = timeout_at(deadline, self.state.connection().open_bi())
            .await
            .map_err(|_| SessionError::Timeout)?
            .map_err(|_| SessionError::Closed)?;
        let stream = wrap(&self.state, slot, send, recv, deadline);
        self.state.check_ready()?;
        ensure_deadline(deadline)?;
        Ok(stream)
    }

    /// Retira uma stream da fila pós-manifesto e confere autorização antes da
    /// entrega. Em saturação, encerra só a stream com o código de cota.
    pub async fn accept_bi(&self) -> Result<OperationalBiStream, SessionError> {
        self.accept_stream(self.state.attempt_timeout, OperationalBiStream::new)
            .await
    }

    async fn accept_stream<T>(
        &self,
        opening_timeout: Duration,
        wrap: impl FnOnce(
            &Arc<SessionState>,
            OwnedSemaphorePermit,
            SendStream,
            RecvStream,
            Instant,
        ) -> T,
    ) -> Result<T, SessionError> {
        let _accept = self.state.accept.lock().await;
        self.state.check_ready()?;
        let (mut send, mut recv) = self
            .state
            .connection()
            .accept_bi()
            .await
            .map_err(|_| SessionError::Closed)?;
        let Some(deadline) = Instant::now().checked_add(opening_timeout) else {
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
        let stream = wrap(&self.state, slot, send, recv, deadline);
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
    #[cfg(feature = "snapshot-harness")]
    profile: StreamProfile,
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
            #[cfg(feature = "snapshot-harness")]
            profile: StreamProfile::Unselected,
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

    fn select_a2(&mut self) -> Result<(), SessionError> {
        self.check_open()?;
        #[cfg(feature = "snapshot-harness")]
        {
            if self.profile == StreamProfile::Snapshot {
                return Err(SessionError::InvalidFrame);
            }
            self.profile = StreamProfile::A2;
        }
        Ok(())
    }

    /// Lê um frame A2 limitado. O FIN permanece para validação explícita.
    pub async fn read_frame(&mut self) -> Result<Vec<u8>, SessionError> {
        self.select_a2()?;
        self.read_bounded_frame(self.state.max_frame_bytes, None)
            .await
    }

    /// Escreve um frame A2 com prefixo, deixando o envio aberto.
    pub async fn write_frame(&mut self, body: &[u8]) -> Result<(), SessionError> {
        self.check_open()?;
        let length = checked_length(body, self.state.max_frame_bytes, None)?;
        self.select_a2()?;
        self.write_sized_frame(body, length).await
    }

    /// Consome a abertura uma vez e entrega o frame A2 ou o primeiro controle.
    #[cfg(feature = "snapshot-harness")]
    pub async fn read_opening(&mut self) -> Result<StreamOpening, SessionError> {
        self.check_open()?;
        if self.profile != StreamProfile::Unselected {
            return Err(self.incoming_error(SessionError::InvalidFrame));
        }
        // Mesmo que a leitura seja interrompida, o escape não pode ser lido de novo.
        self.profile = StreamProfile::A2;
        let length = self.read_prefix().await?;
        if length != 0 {
            return self
                .read_payload(length, self.state.max_frame_bytes, None)
                .await
                .map(StreamOpening::A2);
        }
        let Some(limits) = self.state.snapshot else {
            return Err(self.incoming_error(SessionError::InvalidFrame));
        };
        let control = self.read_bounded_frame(limits.control_cap(), None).await?;
        self.profile = StreamProfile::Snapshot;
        Ok(StreamOpening::Snapshot(control))
    }

    /// Escreve o escape zero e o primeiro controle, apenas em uma stream nova.
    #[cfg(feature = "snapshot-harness")]
    pub async fn write_snapshot_opening(&mut self, control: &[u8]) -> Result<(), SessionError> {
        self.check_open()?;
        if self.profile != StreamProfile::Unselected || self.sent_fin {
            return Err(SessionError::InvalidFrame);
        }
        let limits = self.state.snapshot.ok_or(SessionError::InvalidFrame)?;
        let length = checked_length(control, limits.control_cap(), None)?;
        self.profile = StreamProfile::Snapshot;
        self.write_bytes(&0u32.to_be_bytes()).await?;
        self.write_sized_frame(control, length).await
    }

    #[cfg(feature = "snapshot-harness")]
    fn snapshot_limits(&mut self) -> Result<SnapshotSessionLimits, SessionError> {
        self.check_open()?;
        if self.profile != StreamProfile::Snapshot {
            return Err(SessionError::InvalidFrame);
        }
        self.state.snapshot.ok_or(SessionError::InvalidFrame)
    }

    /// Lê um controle sob o teto de controle da sessão.
    #[cfg(feature = "snapshot-harness")]
    pub async fn read_snapshot_control(&mut self) -> Result<Vec<u8>, SessionError> {
        let cap = self.snapshot_limits()?.control_cap();
        self.read_bounded_frame(cap, None).await
    }

    /// Escreve um controle sob o teto de controle da sessão.
    #[cfg(feature = "snapshot-harness")]
    pub async fn write_snapshot_control(&mut self, control: &[u8]) -> Result<(), SessionError> {
        let cap = self.snapshot_limits()?.control_cap();
        let length = checked_length(control, cap, None)?;
        self.write_sized_frame(control, length).await
    }

    /// Lê os bytes integrais de B1, exigindo o comprimento declarado antes de alocar.
    #[cfg(feature = "snapshot-harness")]
    pub async fn read_snapshot_body(
        &mut self,
        expected_len: NonZeroU32,
        negotiated_max: NonZeroU32,
    ) -> Result<Vec<u8>, SessionError> {
        let cap = self.snapshot_limits()?.body_cap(negotiated_max);
        self.read_bounded_frame(cap, Some(expected_len.get())).await
    }

    /// Escreve os bytes integrais de B1 após conferir comprimento e tetos.
    #[cfg(feature = "snapshot-harness")]
    pub async fn write_snapshot_body(
        &mut self,
        body: &[u8],
        declared_len: NonZeroU32,
        negotiated_max: NonZeroU32,
    ) -> Result<(), SessionError> {
        let cap = self.snapshot_limits()?.body_cap(negotiated_max);
        let length = checked_length(body, cap, Some(declared_len.get()))?;
        self.write_sized_frame(body, length).await
    }

    async fn read_prefix(&mut self) -> Result<u32, SessionError> {
        self.check_open()?;
        let mut header = [0; 4];
        before_deadline(self.deadline, self.recv.read_exact(&mut header))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(|error| self.incoming_error(exact_error(error)))?;
        self.check_open()?;
        Ok(u32::from_be_bytes(header))
    }

    async fn read_bounded_frame(
        &mut self,
        cap: u32,
        expected: Option<u32>,
    ) -> Result<Vec<u8>, SessionError> {
        let length = self.read_prefix().await?;
        self.read_payload(length, cap, expected).await
    }

    async fn read_payload(
        &mut self,
        length: u32,
        cap: u32,
        expected: Option<u32>,
    ) -> Result<Vec<u8>, SessionError> {
        if !valid_length(length, cap, expected) {
            return Err(self.incoming_error(SessionError::InvalidFrame));
        }
        self.check_open()?;
        let size =
            usize::try_from(length).map_err(|_| self.incoming_error(SessionError::InvalidFrame))?;
        let mut body = vec![0; size];
        before_deadline(self.deadline, self.recv.read_exact(&mut body))
            .await
            .map_err(|_| self.timed_out())?
            .map_err(|error| self.incoming_error(exact_error(error)))?;
        self.check_open()?;
        Ok(body)
    }

    async fn write_sized_frame(&mut self, body: &[u8], length: u32) -> Result<(), SessionError> {
        self.write_bytes(&length.to_be_bytes()).await?;
        self.write_bytes(body).await
    }

    async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
        self.check_open()?;
        if let Err(error) = before_deadline(self.deadline, self.send.write_all(bytes))
            .await
            .map_err(|_| self.timed_out())?
        {
            self.abort();
            return Err(write_error(error));
        }
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

fn valid_length(length: u32, cap: u32, expected: Option<u32>) -> bool {
    length != 0 && length <= cap && expected.is_none_or(|expected| length == expected)
}

fn checked_length(body: &[u8], cap: u32, expected: Option<u32>) -> Result<u32, SessionError> {
    let length = u32::try_from(body.len()).map_err(|_| SessionError::InvalidFrame)?;
    valid_length(length, cap, expected)
        .then_some(length)
        .ok_or(SessionError::InvalidFrame)
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
