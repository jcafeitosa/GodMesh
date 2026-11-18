//! Núcleo de composição do GodMesh.
//!
//! Liga os contratos dos crates de domínio sem rede, banco ou transporte. Aqui a
//! regra deixa de ser "cada contrato isolado" e passa a ser "nenhuma operação
//! sensível atravessa fronteira sem autoridade e sem política".

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use godmesh_emergency::incident::{HumanProof, Incident};
use godmesh_identity::grant::{GrantDecision, GrantPredicate};
use godmesh_policy::high_water::{AuthorityMark, DeviceEvidence, HighWaterError};
use godmesh_policy::retention::{RetentionError, RetentionTable};
use godmesh_protocol::ack::AckClass;
use godmesh_sync::position::EventPosition;

/// Composição do núcleo.
#[derive(Debug)]
pub struct Core {
    retention: RetentionTable,
    incidents: BTreeMap<String, Incident>,
}

impl Core {
    #[must_use]
    pub fn new(retention: RetentionTable) -> Self {
        Self { retention, incidents: BTreeMap::new() }
    }

    #[must_use]
    pub fn incidents(&self) -> &BTreeMap<String, Incident> {
        &self.incidents
    }

    /// # Errors
    /// `GrantDenied` quando o predicado nega; nesse caso nada é criado.
    pub fn open_incident(
        &mut self,
        predicate: &GrantPredicate,
        id: &str,
        tenant: &str,
        principal: &str,
    ) -> Result<Incident, CoreError> {
        match predicate.evaluate() {
            GrantDecision::Allow => {}
            denied => return Err(CoreError::GrantDenied(denied)),
        }
        let incident = Incident::triggered(id, tenant, principal);
        self.incidents.insert(id.to_owned(), incident.clone());
        Ok(incident)
    }

    /// # Errors
    /// `NotFound`; `Retention` quando a classe do incidente não tem política.
    pub fn persist_incident(&mut self, id: &str, inbox_ref: &str) -> Result<(), CoreError> {
        let policy_ref = match self.retention.get("incidente_sos") {
            Ok(policy) => format!("policy:{}m", policy.months()),
            Err(e) => {
                if let Some(inc) = self.incidents.get_mut(id) {
                    inc.fail_persistence("retention_policy_missing");
                }
                return Err(CoreError::Retention(e));
            }
        };
        let inc = self.incidents.get_mut(id).ok_or(CoreError::NotFound)?;
        inc.persist(inbox_ref, &policy_ref).map_err(CoreError::Incident)
    }

    /// # Errors
    /// `NotFound`; `Incident` para transição ilegal.
    pub fn replicate_incident(&mut self, id: &str, node: &str, at_unix: u64) -> Result<(), CoreError> {
        self.incidents
            .get_mut(id)
            .ok_or(CoreError::NotFound)?
            .mark_replicated(node, at_unix)
            .map_err(CoreError::Incident)
    }

    /// # Errors
    /// `NotFound`; `Incident` para transição ilegal.
    pub fn ack_technical(&mut self, id: &str, node: &str, at_unix: u64) -> Result<(), CoreError> {
        self.incidents
            .get_mut(id)
            .ok_or(CoreError::NotFound)?
            .mark_technical_ack(node, at_unix)
            .map_err(CoreError::Incident)
    }

    /// # Errors
    /// `NotFound`; `Incident` para transição ilegal ou prova ausente.
    pub fn acknowledge_human(&mut self, id: &str, proof: HumanProof) -> Result<(), CoreError> {
        self.incidents
            .get_mut(id)
            .ok_or(CoreError::NotFound)?
            .mark_acknowledged_human(proof)
            .map_err(CoreError::Incident)
    }

    /// # Errors
    /// `NotFound`.
    pub fn incident(&self, id: &str) -> Result<&Incident, CoreError> {
        self.incidents.get(id).ok_or(CoreError::NotFound)
    }

    /// Classe de ACK do incidente, já tipada pelo contrato de protocolo.
    ///
    /// # Errors
    /// `NotFound`; `Ack` se o nome não pertencer ao conjunto fechado.
    pub fn incident_ack_class(&self, id: &str) -> Result<AckClass, CoreError> {
        let name = self.incident(id)?.ack_class();
        AckClass::parse(name).map_err(|_| CoreError::Ack)
    }

    /// Grava posição de evento apenas se a classe de retenção existir.
    ///
    /// # Errors
    /// `Retention` sem política; `Position` para posição inválida.
    pub fn record_event(&self, draft: EventDraft, retention_class: &str) -> Result<EventPosition, CoreError> {
        self.retention.get(retention_class)?;
        EventPosition::new(draft.origin, draft.tenant, draft.stream, draft.partition, draft.epoch, draft.seq)
            .map_err(CoreError::Position)
    }

    /// Exige que a evidência offline esteja estritamente à frente da alta marca.
    ///
    /// # Errors
    /// Propaga `HighWaterError` (restore, replay, clone, sem fonte).
    pub fn confirm_offline_freshness(
        &self,
        mark: &AuthorityMark,
        evidence: DeviceEvidence,
    ) -> Result<(), CoreError> {
        mark.compare(evidence).map_err(CoreError::HighWater)
    }
}

/// Proposta de evento antes da validação de retenção e posição.
#[derive(Debug, Clone, Copy)]
pub struct EventDraft<'a> {
    pub origin: &'a str,
    pub tenant: &'a str,
    pub stream: &'a str,
    pub partition: u32,
    pub epoch: u64,
    pub seq: u64,
}

/// Erros de composição.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    GrantDenied(GrantDecision),
    Retention(RetentionError),
    Incident(godmesh_emergency::incident::IncidentError),
    Position(godmesh_sync::position::PositionError),
    HighWater(HighWaterError),
    Ack,
    NotFound,
}

impl From<RetentionError> for CoreError {
    fn from(e: RetentionError) -> Self {
        Self::Retention(e)
    }
}
