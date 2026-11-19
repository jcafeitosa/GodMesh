//! Elegibilidade local da alternativa M5 na fixture de laboratório.

use std::{
    fmt,
    net::SocketAddr,
    num::NonZeroU64,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use crate::{
    identity::node_id_for,
    node::{DialAttempt, DialError, DialTarget},
};

const EVIDENCE_TTL: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub enum FallbackError {
    InvalidTarget,
    PathNotEffective,
    GenerationChanged,
    BindingMismatch,
    OutsideWindow,
    AlreadyReserved,
    DirectPromoted,
    DirectNotCancelled,
    DirectFailed(DialError),
}

impl fmt::Display for FallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectFailed(error) => write!(f, "discagem direta falhou: {error}"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl std::error::Error for FallbackError {}

/// Estado do caminho direto, exclusivo do controlador local da fixture.
pub struct DirectPathController {
    target: DialTarget,
    id: Arc<()>,
    generation: Mutex<PathGeneration>,
}

#[derive(Default)]
struct PathGeneration {
    effective: Option<NonZeroU64>,
    latest: Option<NonZeroU64>,
}

impl DirectPathController {
    /// Vincula as observações locais a um endpoint e pin fixos.
    ///
    /// # Errors
    /// Recusa ID divergente da chave ou endpoint fora do perfil.
    pub fn new(mut target: DialTarget) -> Result<Self, FallbackError> {
        if node_id_for(&target.public_key) != target.node_id {
            return Err(FallbackError::InvalidTarget);
        }
        target.addr = normalize(target.addr).ok_or(FallbackError::InvalidTarget)?;
        Ok(Self {
            target,
            id: Arc::new(()),
            generation: Mutex::new(PathGeneration::default()),
        })
    }

    /// Serializa mudanças de caminho, confirmação e reserva da alternativa.
    ///
    /// # Panics
    /// Recusa continuar se o controlador tiver abandonado uma mudança por pânico.
    #[must_use]
    pub fn lock(&self) -> DirectPathGuard<'_> {
        DirectPathGuard {
            controller: self,
            generation: self
                .generation
                .lock()
                .expect("controlador de caminho interrompido"),
        }
    }
}

/// Proteção síncrona da geração efetiva e da reserva, sem operações de rede.
pub struct DirectPathGuard<'a> {
    controller: &'a DirectPathController,
    generation: MutexGuard<'a, PathGeneration>,
}

/// Resultado tipado de indisponibilidade, emitido pelo controlador local.
pub struct DirectPathUnavailable {
    controller: Arc<()>,
    attempt: Arc<()>,
    target: DialTarget,
    generation: NonZeroU64,
    confirmed_at: Instant,
}

/// Capacidade emitida somente após o cancelamento observado e o recolhimento.
pub struct ReapedDirect {
    attempt: Arc<()>,
    target: DialTarget,
    reserved: bool,
}

impl ReapedDirect {
    pub(crate) fn new(attempt: Arc<()>, target: DialTarget) -> Self {
        Self {
            attempt,
            target,
            reserved: false,
        }
    }
}

/// Reserva única. O chamador ainda precisa autorizar a rota antes da abertura.
pub struct FallbackReservation(DialTarget);

impl FallbackReservation {
    /// Vincula o endereço do shim local reservado ao mesmo ID e pin do destino.
    #[must_use]
    pub fn into_target(mut self, shim: SocketAddr) -> DialTarget {
        self.0.addr = shim;
        self.0
    }
}

impl DirectPathGuard<'_> {
    /// Publica a geração de bloqueio confirmada, ou desativa o bloqueio.
    ///
    /// O controlador mantém esta proteção também ao alterar o estado efetivo.
    ///
    /// # Errors
    /// Recusa regressão de geração ou reativação de geração já encerrada.
    pub fn set_effective(&mut self, effective: Option<NonZeroU64>) -> Result<(), FallbackError> {
        if effective == self.generation.effective {
            return Ok(());
        }
        if let Some(new) = effective {
            if self.generation.latest.is_some_and(|latest| new <= latest) {
                return Err(FallbackError::GenerationChanged);
            }
            self.generation.latest = Some(new);
        }
        self.generation.effective = effective;
        Ok(())
    }

    /// Registra a confirmação monotônica local para a tentativa pinada.
    ///
    /// # Errors
    /// Recusa caminho inativo ou tentativa de outro destino, pin ou endpoint.
    pub fn confirm_unavailable(
        &self,
        attempt: &DialAttempt,
        confirmed_at: Instant,
    ) -> Result<DirectPathUnavailable, FallbackError> {
        let generation = self
            .generation
            .effective
            .ok_or(FallbackError::PathNotEffective)?;
        let (id, target) = attempt.fallback_binding();
        if !same_target(&self.controller.target, target) {
            return Err(FallbackError::BindingMismatch);
        }
        Ok(DirectPathUnavailable {
            controller: Arc::clone(&self.controller.id),
            attempt: Arc::clone(id),
            target: self.controller.target.clone(),
            generation,
            confirmed_at,
        })
    }

    /// Consome a tentativa recolhida e reserva a alternativa na mesma decisão.
    ///
    /// O cadastro corrente e o instante devem ser obtidos sob esta proteção.
    ///
    /// # Errors
    /// Recusa prova já usada, vínculos divergentes, geração substituída ou TTL excedido.
    pub fn reserve(
        &mut self,
        evidence: &DirectPathUnavailable,
        direct: &mut ReapedDirect,
        current: &DialTarget,
        now: Instant,
    ) -> Result<FallbackReservation, FallbackError> {
        if direct.reserved {
            return Err(FallbackError::AlreadyReserved);
        }
        if !Arc::ptr_eq(&self.controller.id, &evidence.controller)
            || !Arc::ptr_eq(&direct.attempt, &evidence.attempt)
            || !same_target(&evidence.target, &direct.target)
            || !same_target(&evidence.target, current)
        {
            return Err(FallbackError::BindingMismatch);
        }
        if self.generation.effective != Some(evidence.generation) {
            return Err(FallbackError::GenerationChanged);
        }
        if !now
            .checked_duration_since(evidence.confirmed_at)
            .is_some_and(|age| age < EVIDENCE_TTL)
        {
            return Err(FallbackError::OutsideWindow);
        }
        direct.reserved = true;
        Ok(FallbackReservation(evidence.target.clone()))
    }
}

fn normalize(addr: SocketAddr) -> Option<SocketAddr> {
    let ip = match addr {
        SocketAddr::V6(v6) if v6.scope_id() != 0 => return None,
        SocketAddr::V6(v6) => v6.ip().to_ipv4_mapped().map_or(addr.ip(), Into::into),
        SocketAddr::V4(_) => addr.ip(),
    };
    if addr.port() == 0 || ip.is_unspecified() || ip.is_multicast() {
        return None;
    }
    Some(SocketAddr::new(ip, addr.port()))
}

fn same_target(expected: &DialTarget, actual: &DialTarget) -> bool {
    expected.node_id == actual.node_id
        && expected.public_key == actual.public_key
        && Some(expected.addr) == normalize(actual.addr)
}
