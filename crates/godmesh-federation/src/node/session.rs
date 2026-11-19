//! Streams de aplicação de uma conexão promovida a operacional.

use std::fmt;
use std::future::Future;
use std::num::{NonZeroU32, NonZeroUsize};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use quinn::{Connection, ReadError, ReadExactError, RecvStream, SendStream, VarInt, WriteError};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use super::{LinkIdentity, OperationalLink, close_with_rejection};
use crate::link::PeerLinks;

// Faixa de controle de streams, separada dos motivos de fechamento da conexão.
const STREAM_BUSY: u32 = 0x474D_0101;
const STREAM_ABORTED: u32 = 0x474D_0102;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Limites locais do dispatcher pós-manifesto.
#[derive(Debug, Clone, Copy)]
pub struct SessionLimits {
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

    async fn authorize(&self) -> Result<(), SessionError> {
        self.check_ready()?;
        let connection = self.connection();
        let result = tokio::select! {
            biased;
            _ = connection.closed() => return Err(SessionError::Closed),
            result = self.link.authorize() => result,
        };
        let _permit = result.map_err(|rejection| {
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

    /// Abre uma stream depois de adquirir cota e conferir autorização corrente.
    pub async fn open_bi(&self) -> Result<OperationalBiStream, SessionError> {
        self.state.check_ready()?;
        let slot = self.state.slot()?;
        self.state.authorize().await?;
        let (send, recv) = self
            .state
            .connection()
            .open_bi()
            .await
            .map_err(|_| SessionError::Closed)?;
        let stream = OperationalBiStream::new(&self.state, slot, send, recv);
        self.state.check_ready()?;
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
        let slot = match self.state.slot() {
            Ok(slot) => slot,
            Err(error) => {
                abort_stream(&mut send, &mut recv, STREAM_BUSY);
                return Err(error);
            }
        };
        let stream = OperationalBiStream::new(&self.state, slot, send, recv);
        self.state.authorize().await?;
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
}

impl OperationalBiStream {
    fn new(
        state: &Arc<SessionState>,
        slot: OwnedSemaphorePermit,
        send: SendStream,
        recv: RecvStream,
    ) -> Self {
        Self {
            state: Arc::clone(state),
            slot: Some(slot),
            send,
            recv,
            sent_fin: false,
            received_fin: false,
        }
    }

    fn check_open(&self) -> Result<(), SessionError> {
        if self.slot.is_none() {
            return Err(SessionError::Closed);
        }
        self.state.check_ready()
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
        self.recv
            .read_exact(&mut header)
            .await
            .map_err(|error| self.incoming_error(exact_error(error)))?;
        let length = u32::from_be_bytes(header);
        if length == 0 || length > self.state.max_frame_bytes {
            return Err(self.incoming_error(SessionError::InvalidFrame));
        }
        self.check_open()?;
        let size = usize::try_from(length).map_err(|_| SessionError::InvalidFrame)?;
        let mut body = vec![0; size];
        self.recv
            .read_exact(&mut body)
            .await
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
        self.send
            .write_all(&length.to_be_bytes())
            .await
            .map_err(write_error)?;
        self.send.write_all(body).await.map_err(write_error)?;
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
        if self
            .recv
            .read(&mut [0])
            .await
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
