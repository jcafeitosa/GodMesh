//! ADR-058 (ACK por fronteira), ADR-064 (negação sem raiz de confiança) e
//! ADR-065 (classe de retenção do incidente).
//!
//! Invariante central: o rótulo exibido ao usuário é derivado do estado, nunca da
//! intenção. Sem persistência não existe "acionado".

/// Estado do incidente SOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentState {
    /// Gatilho local aceito pelo app; nada persistido ainda.
    TriggeredLocally,
    /// Registro mínimo persistido localmente (ADR-065: classe `incidente_sos`).
    PersistedLocal,
    /// Evento replicado à autoridade.
    ReplicatedToAuthority,
    /// Nó confirmou tecnicamente o recebimento.
    AcknowledgedTechnical,
    /// Pessoa autorizada confirmou (exige prova).
    AcknowledgedHuman,
    /// Enviado sem confirmação; resultado desconhecido.
    OutcomeUnknown,
    /// Nada foi persistido; o pedido não existe em lugar algum.
    NotPersisted,
    /// Encerrado após ACK humano.
    Closed,
}

impl IncidentState {
    /// Nome da classe de ACK compatível com a ADR-058.
    #[must_use]
    pub const fn ack_class_name(self) -> &'static str {
        match self {
            Self::TriggeredLocally => "transport_accepted",
            Self::PersistedLocal => "persisted_local",
            Self::ReplicatedToAuthority | Self::OutcomeUnknown => "replicated_to_authority",
            Self::AcknowledgedTechnical => "materialized",
            Self::AcknowledgedHuman | Self::Closed => "acknowledged_human",
            Self::NotPersisted => "transport_accepted",
        }
    }

    /// Só estados com evidência de chegada podem alegar entrega.
    #[must_use]
    pub const fn claims_delivery(self) -> bool {
        matches!(self, Self::AcknowledgedTechnical | Self::AcknowledgedHuman | Self::Closed)
    }

    /// Sucesso de domínio exige reconhecimento humano ou encerramento.
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::AcknowledgedHuman | Self::Closed)
    }
}

/// Prova de que uma pessoa reconheceu o incidente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanProof {
    responder: Option<String>,
    at_unix: u64,
}

impl HumanProof {
    #[must_use]
    pub fn person(responder: &str, at_unix: u64) -> Self {
        Self { responder: Some(responder.to_owned()), at_unix }
    }

    /// Prova sem pessoa não é prova: `AcknowledgedHuman` não pode ser alegado.
    pub const fn anonymous() -> Self {
        Self { responder: None, at_unix: 0 }
    }

    #[must_use]
    pub fn responder(&self) -> Option<&str> {
        self.responder.as_deref()
    }

    #[must_use]
    pub const fn at_unix(&self) -> u64 {
        self.at_unix
    }
}

/// Registro mínimo aceito por etapa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acknowledgement {
    pub class: &'static str,
    pub source: String,
    pub at_unix: u64,
}

/// Incidente SOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incident {
    id: String,
    tenant: String,
    principal: String,
    state: IncidentState,
    acks: Vec<Acknowledgement>,
    reason: Option<String>,
}

impl Incident {
    #[must_use]
    pub fn triggered(id: &str, tenant: &str, principal: &str) -> Self {
        Self {
            id: id.to_owned(),
            tenant: tenant.to_owned(),
            principal: principal.to_owned(),
            state: IncidentState::TriggeredLocally,
            acks: Vec::new(),
            reason: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> IncidentState {
        self.state
    }

    #[must_use]
    pub fn ack_class(&self) -> &'static str {
        self.state.ack_class_name()
    }

    /// Texto que a UI pode mostrar. Nunca promete o que não aconteceu.
    #[must_use]
    pub fn ui_label(&self) -> &'static str {
        match self.state {
            IncidentState::TriggeredLocally => "tentativa local, não persistida",
            IncidentState::NotPersisted => "não salvo / não enfileirado",
            IncidentState::PersistedLocal => "registrado localmente, não enviado",
            IncidentState::ReplicatedToAuthority => "replicado à autoridade, sem confirmação",
            IncidentState::AcknowledgedTechnical => "recebido por um nó, sem pessoa",
            IncidentState::AcknowledgedHuman => "reconhecido por pessoa autorizada",
            IncidentState::OutcomeUnknown => "envio sem confirmação — resultado desconhecido",
            IncidentState::Closed => "encerrado",
        }
    }

    /// # Errors
    /// `IllegalTransition` fora do estado permitido; `EmptyField` sem referência.
    pub fn persist(&mut self, inbox_ref: &str, retention_policy: &str) -> Result<(), IncidentError> {
        if self.state != IncidentState::TriggeredLocally {
            return Err(IncidentError::IllegalTransition);
        }
        if inbox_ref.is_empty() || retention_policy.is_empty() {
            return Err(IncidentError::EmptyField);
        }
        self.state = IncidentState::PersistedLocal;
        self.acks.push(Acknowledgement {
            class: "persisted_local",
            source: inbox_ref.to_owned(),
            at_unix: 0,
        });
        Ok(())
    }

    /// Registra a falha de persistência; o pedido passa a não existir.
    pub fn fail_persistence(&mut self, reason: &str) {
        self.state = IncidentState::NotPersisted;
        self.reason = Some(reason.to_owned());
    }

    /// # Errors
    /// `IllegalTransition` fora de `PersistedLocal`.
    pub fn mark_replicated(&mut self, node: &str, at_unix: u64) -> Result<(), IncidentError> {
        if self.state != IncidentState::PersistedLocal {
            return Err(IncidentError::IllegalTransition);
        }
        self.state = IncidentState::ReplicatedToAuthority;
        self.acks.push(Acknowledgement {
            class: "replicated_to_authority",
            source: node.to_owned(),
            at_unix,
        });
        Ok(())
    }

    /// # Errors
    /// `IllegalTransition` fora de `ReplicatedToAuthority`.
    pub fn mark_technical_ack(&mut self, node: &str, at_unix: u64) -> Result<(), IncidentError> {
        if self.state != IncidentState::ReplicatedToAuthority {
            return Err(IncidentError::IllegalTransition);
        }
        self.state = IncidentState::AcknowledgedTechnical;
        self.acks.push(Acknowledgement {
            class: "materialized",
            source: node.to_owned(),
            at_unix,
        });
        Ok(())
    }

    /// # Errors
    /// `IllegalTransition` fora de `AcknowledgedTechnical`; `MissingHumanProof` sem pessoa.
    pub fn mark_acknowledged_human(&mut self, proof: HumanProof) -> Result<(), IncidentError> {
        if self.state != IncidentState::AcknowledgedTechnical {
            return Err(IncidentError::IllegalTransition);
        }
        let Some(responder) = proof.responder() else {
            return Err(IncidentError::MissingHumanProof);
        };
        self.state = IncidentState::AcknowledgedHuman;
        self.acks.push(Acknowledgement {
            class: "acknowledged_human",
            source: responder.to_owned(),
            at_unix: proof.at_unix(),
        });
        Ok(())
    }

    /// Envio sem confirmação. Não promove a classe de ACK.
    pub fn mark_outcome_unknown(&mut self, reason: &str) {
        self.state = IncidentState::OutcomeUnknown;
        self.reason = Some(reason.to_owned());
    }

    /// # Errors
    /// `IllegalTransition` se não houver ACK humano.
    pub fn close(&mut self, reason: &str) -> Result<(), IncidentError> {
        if self.state != IncidentState::AcknowledgedHuman {
            return Err(IncidentError::IllegalTransition);
        }
        self.state = IncidentState::Closed;
        self.reason = Some(reason.to_owned());
        Ok(())
    }

    #[must_use]
    pub fn acks(&self) -> &[Acknowledgement] {
        &self.acks
    }

    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    #[must_use]
    pub const fn retention_class(&self) -> &'static str {
        "incidente_sos"
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    #[must_use]
    pub fn principal(&self) -> &str {
        &self.principal
    }

    /// Entrega a dispositivo nunca é promovida por este contrato.
    ///
    /// # Errors
    /// Sempre `IllegalTransition`: quem prova entrega é o destinatário, não o emissor.
    pub fn mark_delivered(&mut self, _device: &str) -> Result<(), IncidentError> {
        // Entrega a dispositivo não é etapa deste contrato: quem prova é o destinatário.
        Err(IncidentError::IllegalTransition)
    }
}

/// Erros do contrato de incidente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentError {
    IllegalTransition,
    MissingHumanProof,
    EmptyField,
}
