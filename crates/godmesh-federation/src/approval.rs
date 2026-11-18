//! Certificado de aprovação assinado por nó (ADR-071, emenda §2; fatia M1b).
//!
//! A emenda à ADR-071 diz que autoridade de ingresso **é de nó, não de token
//! solto**: um grant humano com `federation:admin` autoriza a *operação*, mas não
//! prova que quem aprovou é um nó autorizado da malha. Este módulo é a segunda
//! prova: um Ed25519 assinado pelo nó aprovador sobre o conteúdo canônico do
//! `join_certificate`, verificável por qualquer nó que conheça a chave do
//! aprovador — sem confiar em quem entregou o certificado.
//!
//! ## Serialização canônica
//!
//! Mesma regra do [`SignedManifest`](crate::manifest::SignedManifest): JSON não
//! é assinado. O domínio muda (`godmesh.federation.approval.v1`), o codec é o
//! mesmo — `push_field`/`normalise` de `manifest`. Conteúdo assinado, na ordem:
//!
//! ```text
//! "godmesh.federation.approval.v1"
//! "join_request_id" <u32 BE len><bytes>
//! "node_id"         <u32 BE len><bytes>   -- nó APROVADOR
//! "public_key"      <u32 BE len><bytes>   -- chave pública do aprovador
//! "cluster"         <u32 BE len><bytes>
//! "scopes"          <u32 BE n> (<u32 BE len><bytes>)*  -- ordenado, sem repetição
//! "not_before"      <i64 BE>
//! "expires_at"      <i64 BE>
//! "epoch"           <u64 BE>
//! ```
//!
//! `join_request_id` entra nos bytes assinados de propósito: sem isso, um
//! certificado emitido para um pedido seria replay contra outro.
//!
//! ## Verificação
//!
//! [`ApprovalCertificate::verify_with_key`] verifica contra a chave que o
//! **registro** diz ser do aprovador (peers autorizados ou genesis declarado), e
//! não contra a chave que o próprio certificado declara. Essa distinção é o
//! ponto da emenda: o certificado não é autoridade sobre a própria identidade.
//! `node_id` precisa ser o derivado daquela chave, sob pena de um nó assinar em
//! nome de outro.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::identity::{self, NodeIdentity};
use crate::manifest::{MAX_FIELD_LEN, MAX_LIST_ITEMS, normalise, push_field};

/// Domínio da assinatura do certificado de aprovação (versão 1).
pub const APPROVAL_DOMAIN: &[u8] = b"godmesh.federation.approval.v1";

/// Domínio exclusivo da assinatura de rejeição (versão 1).
pub const REJECTION_DOMAIN: &[u8] = b"godmesh.federation.rejection.v1";

/// Ação decidida pela rota, vinculada à assinatura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionAction {
    /// Autoriza o ingresso.
    Approve,
    /// Encerra o pedido.
    Reject,
}

impl DecisionAction {
    const fn domain(self) -> &'static [u8] {
        match self {
            Self::Approve => APPROVAL_DOMAIN,
            Self::Reject => REJECTION_DOMAIN,
        }
    }
}

/// Prefixo do identificador de pedido de ingresso (`fjr_` + 32 hex).
pub const JOIN_REQUEST_ID_PREFIX: &str = "fjr_";

/// Confere o formato de `join_request_id`: prefixo e 32 hex minúsculos.
#[must_use]
pub fn is_join_request_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix(JOIN_REQUEST_ID_PREFIX) else {
        return false;
    };
    rest.len() == 32
        && rest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Prova assinada de que **um nó autorizado** decidiu sobre um pedido.
///
/// `node_id` e `public_key` são do nó aprovador — o candidato aprovado é
/// identificado pelo `join_request_id`, nunca por este campo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ApprovalCertificate {
    /// Pedido a que esta aprovação se aplica (ligação anti-replay).
    pub join_request_id: String,
    /// `node_id` do nó aprovador (`gmn_` + hex de `sha256(pubkey)`).
    pub node_id: String,
    /// Chave pública Ed25519 do aprovador, em hex minúsculo.
    pub public_key: String,
    /// Cluster em nome do qual o aprovador age; o pedido precisa ser do mesmo.
    pub cluster: String,
    /// Escopos que a aprovação declara (conjunto; a ordem não é assinada).
    pub scopes: Vec<String>,
    /// Início de validade em segundos Unix (UTC).
    pub not_before: i64,
    /// Fim de validade em segundos Unix (UTC).
    pub expires_at: i64,
    /// Geração da identidade de federação do aprovador (anti-rollback).
    pub epoch: u64,
    /// Assinatura Ed25519 (hex minúsculo, 128 caracteres) sobre os bytes canônicos.
    pub signature: String,
}

/// Falhas de validação do certificado de aprovação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    /// Campo textual obrigatório vazio.
    EmptyField(&'static str),
    /// Campo textual acima de `MAX_FIELD_LEN`.
    FieldTooLong { field: &'static str, len: usize },
    /// Lista acima de `MAX_LIST_ITEMS` ou com item vazio.
    TooManyItems { field: &'static str, len: usize },
    /// `join_request_id` fora do formato `fjr_` + 32 hex.
    InvalidJoinRequestId(String),
    /// `node_id` fora do formato `gmn_` + 64 hex.
    InvalidNodeId(String),
    /// Chave pública fora do formato esperado.
    InvalidPublicKey(String),
    /// Assinatura fora do formato esperado.
    InvalidSignatureEncoding(String),
    /// Assinatura não confere com os bytes canônicos.
    SignatureInvalid,
    /// `node_id` declarado não é o derivado da chave usada para verificar.
    NodeIdMismatch,
    /// `not_before` no futuro em relação a `now`.
    NotYetValid { not_before: i64, now: i64 },
    /// `expires_at` já passou em relação a `now`.
    Expired { expires_at: i64, now: i64 },
    /// Janela invertida (`expires_at <= not_before`): nada satisfaz.
    InvertedWindow { not_before: i64, expires_at: i64 },
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "campo obrigatório vazio: {field}"),
            Self::FieldTooLong { field, len } => {
                write!(f, "campo {field} com {len} bytes; máximo {MAX_FIELD_LEN}")
            }
            Self::TooManyItems { field, len } => {
                write!(f, "lista {field} com {len} itens; máximo {MAX_LIST_ITEMS}")
            }
            Self::InvalidJoinRequestId(value) => {
                write!(
                    f,
                    "join_request_id inválido: {value}; esperado fjr_ + 32 hex"
                )
            }
            Self::InvalidNodeId(value) => write!(f, "node_id inválido: {value}"),
            Self::InvalidPublicKey(detail) => write!(f, "chave pública inválida: {detail}"),
            Self::InvalidSignatureEncoding(detail) => {
                write!(f, "assinatura inválida: {detail}")
            }
            Self::SignatureInvalid => write!(f, "assinatura não confere com o certificado"),
            Self::NodeIdMismatch => {
                write!(f, "node_id não é o derivado da chave pública do aprovador")
            }
            Self::NotYetValid { not_before, now } => write!(
                f,
                "certificado ainda não válido: not_before={not_before}, now={now}"
            ),
            Self::Expired { expires_at, now } => write!(
                f,
                "certificado expirado: expires_at={expires_at}, now={now}"
            ),
            Self::InvertedWindow {
                not_before,
                expires_at,
            } => write!(
                f,
                "janela invertida: not_before={not_before} >= expires_at={expires_at}"
            ),
        }
    }
}

impl std::error::Error for ApprovalError {}

impl From<identity::IdentityError> for ApprovalError {
    fn from(e: identity::IdentityError) -> Self {
        use identity::IdentityError;
        match e {
            IdentityError::InvalidPublicKey(detail) => Self::InvalidPublicKey(detail),
            IdentityError::InvalidSignatureEncoding(detail) => {
                Self::InvalidSignatureEncoding(detail)
            }
            IdentityError::SignatureInvalid => Self::SignatureInvalid,
            other => Self::InvalidPublicKey(other.to_string()),
        }
    }
}

impl ApprovalCertificate {
    /// Bytes canônicos assinados (sem `signature`).
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.canonical_bytes_for(DecisionAction::Approve)
    }

    /// Bytes canônicos para a ação assinada, sem o campo `signature`.
    #[must_use]
    pub fn canonical_bytes_for(&self, action: DecisionAction) -> Vec<u8> {
        let mut out = Vec::with_capacity(192);
        out.extend_from_slice(action.domain());
        push_field(&mut out, "join_request_id", self.join_request_id.as_bytes());
        push_field(&mut out, "node_id", self.node_id.as_bytes());
        push_field(&mut out, "public_key", self.public_key.as_bytes());
        push_field(&mut out, "cluster", self.cluster.as_bytes());
        let scopes = normalise(&self.scopes);
        push_field(&mut out, "scopes", &(scopes.len() as u32).to_be_bytes());
        for item in scopes {
            push_field(&mut out, "scope", item.as_bytes());
        }
        push_field(&mut out, "not_before", &self.not_before.to_be_bytes());
        push_field(&mut out, "expires_at", &self.expires_at.to_be_bytes());
        push_field(&mut out, "epoch", &self.epoch.to_be_bytes());
        out
    }

    /// Assina o certificado com a identidade do nó aprovador.
    ///
    /// Preenche `node_id`, `public_key` e `signature`: quem chama não escolhe o
    /// `node_id` — ele é sempre o derivado da chave que assina, o que elimina
    /// assinar em nome de outro nó (mesma regra de `SignedManifest::sign`).
    pub fn sign(&mut self, identity: &NodeIdentity) {
        self.sign_for(identity, DecisionAction::Approve);
    }

    /// Assina a ação escolhida com a identidade do nó decisor.
    pub fn sign_for(&mut self, identity: &NodeIdentity, action: DecisionAction) {
        self.node_id = identity.node_id().to_owned();
        self.public_key = identity.public_key_hex();
        self.signature = identity.sign_hex(&self.canonical_bytes_for(action));
    }

    /// Verifica o certificado contra a chave **do registro** do aprovador.
    ///
    /// É a porta que o servidor usa: `trusted_public_key_hex` vem de
    /// `federation_peer` (estado `authorized`) ou do genesis declarado pelo
    /// operador — nunca do corpo do pedido. A chave embutida no certificado é
    /// conferida contra ela, então um certificado que se apresenta com outra
    /// chave falha aqui, mesmo com assinatura internamente consistente.
    ///
    /// # Errors
    /// [`ApprovalError`] para campo inválido, `node_id` não derivado da chave,
    /// chave divergente da registrada ou assinatura que não confere.
    pub fn verify_with_key(&self, trusted_public_key_hex: &str) -> Result<(), ApprovalError> {
        self.verify_for(trusted_public_key_hex, DecisionAction::Approve)
    }

    /// Verifica a prova contra a chave confiável e o domínio da ação.
    ///
    /// # Errors
    /// Os mesmos erros de [Self::verify_with_key], incluindo domínio divergente.
    pub fn verify_for(
        &self,
        trusted_public_key_hex: &str,
        action: DecisionAction,
    ) -> Result<(), ApprovalError> {
        self.validate_fields()?;
        let key = identity::parse_public_key(trusted_public_key_hex)?;
        // A chave registrada e a declarada têm de ser a mesma: o certificado não
        // é autoridade sobre a própria identidade.
        if key.to_bytes() != identity::parse_public_key(&self.public_key)?.to_bytes() {
            return Err(ApprovalError::NodeIdMismatch);
        }
        if identity::node_id_for(&key.to_bytes()) != self.node_id {
            return Err(ApprovalError::NodeIdMismatch);
        }
        Ok(identity::verify_signature(
            trusted_public_key_hex,
            &self.canonical_bytes_for(action),
            &self.signature,
        )?)
    }

    /// Verificação autossuficiente: usa a chave pública declarada no próprio
    /// certificado. Serve a teste e a leitura offline de um certificado já
    /// associado a um aprovador cuja chave o leitor **já** conhece por outro
    /// canal; quem decide ingresso deve usar [`Self::verify_with_key`].
    ///
    /// # Errors
    /// [`ApprovalError::SignatureInvalid`] se a assinatura não confere com os
    /// bytes canônicos; demais erros de campo/identidade conforme o caso.
    pub fn verify(&self) -> Result<(), ApprovalError> {
        self.verify_with_key(&self.public_key.clone())
    }

    /// Janela de validade: `not_before <= now < expires_at`.
    ///
    /// # Errors
    /// [`ApprovalError::InvertedWindow`] para janela impossível,
    /// [`ApprovalError::NotYetValid`] para `now` antes de `not_before` e
    /// [`ApprovalError::Expired`] para `now` em ou depois de `expires_at`.
    pub fn check_validity(&self, now: i64) -> Result<(), ApprovalError> {
        if self.expires_at <= self.not_before {
            return Err(ApprovalError::InvertedWindow {
                not_before: self.not_before,
                expires_at: self.expires_at,
            });
        }
        if now < self.not_before {
            return Err(ApprovalError::NotYetValid {
                not_before: self.not_before,
                now,
            });
        }
        if now >= self.expires_at {
            return Err(ApprovalError::Expired {
                expires_at: self.expires_at,
                now,
            });
        }
        Ok(())
    }

    /// Assinatura **e** janela de validade: o par que a rota de aprovação exige.
    ///
    /// # Errors
    /// Erros de [`Self::verify_with_key`] e de [`Self::check_validity`].
    pub fn verify_at(&self, trusted_public_key_hex: &str, now: i64) -> Result<(), ApprovalError> {
        self.verify_with_key(trusted_public_key_hex)?;
        self.check_validity(now)
    }

    fn validate_fields(&self) -> Result<(), ApprovalError> {
        for (field, value) in [
            ("cluster", &self.cluster),
            ("public_key", &self.public_key),
            ("node_id", &self.node_id),
            ("signature", &self.signature),
        ] {
            if value.trim().is_empty() {
                return Err(ApprovalError::EmptyField(field));
            }
            if value.len() > MAX_FIELD_LEN {
                return Err(ApprovalError::FieldTooLong {
                    field,
                    len: value.len(),
                });
            }
        }
        if !is_join_request_id(&self.join_request_id) {
            return Err(ApprovalError::InvalidJoinRequestId(
                self.join_request_id.clone(),
            ));
        }
        if !identity::is_node_id(&self.node_id) {
            return Err(ApprovalError::InvalidNodeId(self.node_id.clone()));
        }
        if self.scopes.is_empty() {
            return Err(ApprovalError::EmptyField("scopes"));
        }
        if self.scopes.len() > MAX_LIST_ITEMS {
            return Err(ApprovalError::TooManyItems {
                field: "scopes",
                len: self.scopes.len(),
            });
        }
        for item in &self.scopes {
            if item.trim().is_empty() {
                return Err(ApprovalError::EmptyField("scopes"));
            }
            if item.len() > MAX_FIELD_LEN {
                return Err(ApprovalError::FieldTooLong {
                    field: "scopes",
                    len: item.len(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn certificate(identity: &NodeIdentity, now: i64) -> ApprovalCertificate {
        let mut cert = ApprovalCertificate {
            join_request_id: "fjr_0123456789abcdef0123456789abcdef".to_owned(),
            node_id: String::new(),
            public_key: String::new(),
            cluster: "cluster-br-sul".to_owned(),
            scopes: vec!["federation:approve".to_owned(), "sync:read".to_owned()],
            not_before: now - 60,
            expires_at: now + 3_600,
            epoch: 4,
            signature: String::new(),
        };
        cert.sign(identity);
        cert
    }

    #[test]
    fn assinatura_valida_verifica() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let cert = certificate(&identity, 1_700_000_000);
        cert.verify().expect("assinatura válida");
        cert.verify_with_key(&identity.public_key_hex())
            .expect("contra a chave registrada");
        assert_eq!(
            cert.check_validity(1_700_000_000),
            Ok(()),
            "dentro da janela"
        );

        // Mesmos campos → mesmos bytes → mesma assinatura, qualquer que seja a
        // ordem de `scopes` (é conjunto, não lista).
        let mut reordered = cert.clone();
        reordered.scopes = vec!["sync:read".to_owned(), "federation:approve".to_owned()];
        assert_eq!(cert.canonical_bytes(), reordered.canonical_bytes());
        reordered.verify().expect("ordem de conjunto não importa");
    }

    #[test]
    fn um_byte_alterado_invalida_a_assinatura() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let original = certificate(&identity, 1_700_000_000);

        for campo in ["epoch_alterado", "cluster_alterado", "instante_alterado"] {
            let mut tampered = original.clone();
            match campo {
                "epoch_alterado" => tampered.epoch += 1,
                "cluster_alterado" => tampered.cluster = "cluster-outro".to_owned(),
                _ => tampered.not_before += 1,
            }
            assert_eq!(
                tampered.verify(),
                Err(ApprovalError::SignatureInvalid),
                "{campo}"
            );
        }

        // O identificador do pedido também é assinado: trocar o alvo do
        // certificado quebra a assinatura (anti-replay entre pedidos).
        let mut outro_pedido = original.clone();
        outro_pedido.join_request_id = "fjr_ffffffffffffffffffffffffffffffff".to_owned();
        assert_eq!(outro_pedido.verify(), Err(ApprovalError::SignatureInvalid));
    }

    #[test]
    fn node_id_divergente_da_chave_e_recusado() {
        let signer = NodeIdentity::generate_in_memory().expect("identidade");
        let outro = NodeIdentity::generate_in_memory().expect("outra identidade");
        let forged = certificate(&signer, 1_700_000_000);

        // `node_id` do assinante com a chave de OUTRO nó: o par identidade↔chave
        // não fecha, e é isso que se verifica antes da assinatura.
        let mut chave_alheia = forged.clone();
        chave_alheia.public_key = outro.public_key_hex();
        assert_eq!(chave_alheia.verify(), Err(ApprovalError::NodeIdMismatch));

        // E o inverso: assinatura legítima da chave do assinante declarando o
        // `node_id` de outro nó. Reassinar não salva a fraude: a ligação
        // `node_id`↔chave continua sem fechar.
        let mut impersonando = forged.clone();
        impersonando.node_id = outro.node_id().to_owned();
        impersonando.signature = signer.sign_hex(&impersonando.canonical_bytes());
        assert_eq!(impersonando.verify(), Err(ApprovalError::NodeIdMismatch));

        // E contra a chave registrada de OUTRO nó o certificado do assinante não
        // vale: é isso que verify_with_key existe para impedir.
        assert_eq!(
            forged.verify_with_key(&outro.public_key_hex()),
            Err(ApprovalError::NodeIdMismatch)
        );
    }

    #[test]
    fn certificado_expirado_falha_e_janela_invertida_falha() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let cert = certificate(&identity, 1_700_000_000);

        // Replay de certificado velho: exatamente um segundo depois de expira.
        assert_eq!(
            cert.check_validity(cert.expires_at),
            Err(ApprovalError::Expired {
                expires_at: cert.expires_at,
                now: cert.expires_at,
            })
        );
        assert_eq!(
            cert.verify_at(&identity.public_key_hex(), cert.expires_at + 1),
            Err(ApprovalError::Expired {
                expires_at: cert.expires_at,
                now: cert.expires_at + 1,
            }),
            "a assinatura continua válida; o que recusa é a janela"
        );
        assert_eq!(
            cert.check_validity(cert.not_before - 1),
            Err(ApprovalError::NotYetValid {
                not_before: cert.not_before,
                now: cert.not_before - 1,
            })
        );

        let mut invertido = cert.clone();
        invertido.expires_at = invertido.not_before;
        invertido.sign(&identity);
        assert!(matches!(
            invertido.check_validity(1_700_000_000),
            Err(ApprovalError::InvertedWindow { .. })
        ));
    }

    #[test]
    fn campos_obrigatorios_e_formatos_sao_validados_na_fronteira() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let mut cert = certificate(&identity, 1_700_000_000);

        cert.join_request_id = "fjr_curto".to_owned();
        assert_eq!(
            cert.verify(),
            Err(ApprovalError::InvalidJoinRequestId("fjr_curto".to_owned()))
        );

        let mut cert = certificate(&identity, 1_700_000_000);
        cert.scopes = vec![];
        assert_eq!(cert.verify(), Err(ApprovalError::EmptyField("scopes")));

        let mut cert = certificate(&identity, 1_700_000_000);
        cert.cluster = "   ".to_owned();
        assert_eq!(cert.verify(), Err(ApprovalError::EmptyField("cluster")));
    }
}
