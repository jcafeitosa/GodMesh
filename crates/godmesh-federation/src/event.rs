//! Evento assinado pelo nó de origem (S-sec-2, regra dura de
//! `docs/04-seguranca/abuso-e-malha-maligna.md` §5).
//!
//! Um manifesto prova **identidade**; este envelope prova **autoria de evento**:
//! a posição canônica (ADR-057), o tenant, a classe de retenção e o instante de
//! emissão são assinados pela chave do nó que se declara origem. Sem isso, um nó
//! de terceiro comprometido injeta evento em nome de outro nó/tenant e o
//! histórico contaminado é pior que histórico ausente.
//!
//! ## Serialização canônica (mesmo padrão do manifesto, domínio próprio)
//!
//! ```text
//! "godmesh.federation.event.v1"       -- domínio: a mesma chave nunca assina
//!                                       -- um manifesto e um evento de forma
//!                                       -- intercambiável
//! "origin_node"      <u32 BE len><bytes>
//! "tenant_id"        <u32 BE len><bytes>
//! "stream_id"        <u32 BE len><bytes>
//! "partition"        <u32 BE len><4 bytes BE>
//! "authority_epoch"  <u32 BE len><8 bytes BE>
//! "seq"              <u32 BE len><8 bytes BE>
//! "retention_class"  <u32 BE len><bytes>
//! "issued_at"        <u32 BE len><8 bytes BE>
//! "node_id"          <u32 BE len><bytes>
//! ```
//!
//! `signature` **não** entra nos bytes assinados (é a assinatura deles). O
//! envelope não carrega `public_key`: a chave vem do registro de pares
//! (`federation_peer`), aprovado por ingresso — incluir chave no corpo abriria
//! um canal de substituição de identidade que a assinatura não fecha. A ligação
//! `node_id` ↔ chave é reconfirida no verificador com
//! [`crate::identity::verify_node_id`].
//!
//! Nenhuma criptografia própria: `ed25519_dalek` 3.0.0 verifica com
//! `verify_strict`, via [`crate::identity::verify_signature`].

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::identity::{self, NodeIdentity};
use crate::manifest::{MAX_FIELD_LEN, push_field};

/// Domínio da assinatura do evento (versão 1).
pub const EVENT_DOMAIN: &[u8] = b"godmesh.federation.event.v1";

/// Posição canônica declarada pelo nó (ADR-057), dentro do envelope assinado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EventPositionClaim {
    /// Nó que se declara origem do evento. **Tem** de ser o nó que assina.
    pub origin_node: String,
    /// Stream da posição.
    pub stream_id: String,
    /// Partição da posição.
    pub partition: u32,
    /// Epoch da autoridade.
    pub authority_epoch: u64,
    /// Sequência dentro de `(origem, tenant, stream, partição, epoch)`.
    pub seq: u64,
}

/// Evento com posição, tenancy e retenção assinadas pela chave do nó de origem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedEvent {
    /// Posição canônica reivindicada (origem incluída: origem é assinada).
    pub position: EventPositionClaim,
    /// Tenant a que o evento pertence — também assinado, para que um nó
    /// autorizado no tenant A não injete em nome de A uma stream de B.
    pub tenant_id: String,
    /// Classe de retenção pedida (o servidor ainda resolve política).
    pub retention_class: String,
    /// Instante de emissão em segundos Unix (UTC). Limita replay de envelope.
    pub issued_at: i64,
    /// Nó assinante: `gmn_` + hex de `sha256(public_key)`.
    pub node_id: String,
    /// Assinatura Ed25519 (hex minúsculo) sobre os bytes canônicos.
    pub signature: String,
}

/// Falhas do envelope assinado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignedEventError {
    /// Campo textual obrigatório vazio.
    EmptyField(&'static str),
    /// Campo textual acima de [`MAX_FIELD_LEN`].
    FieldTooLong { field: &'static str, len: usize },
    /// `node_id` fora do formato `gmn_` + 64 hex.
    InvalidNodeId(String),
    /// Chave pública fora do formato esperado.
    InvalidPublicKey(String),
    /// Assinatura fora do formato esperado.
    InvalidSignatureEncoding(String),
    /// Assinatura não confere com os bytes canônicos e a chave do registro.
    SignatureInvalid,
    /// A origem declarada na posição não é o nó que assinou.
    OriginNotSigner {
        origin_node: String,
        node_id: String,
    },
    /// `issued_at` velho demais para a janela.
    Expired {
        issued_at: i64,
        now: i64,
        max_age_seconds: i64,
    },
    /// `issued_at` no futuro além da tolerância de relógio.
    NotYetValid { issued_at: i64, now: i64 },
}

impl fmt::Display for SignedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "campo obrigatório vazio: {field}"),
            Self::FieldTooLong { field, len } => {
                write!(f, "campo {field} com {len} bytes; máximo {}", MAX_FIELD_LEN)
            }
            Self::InvalidNodeId(detail) => write!(f, "node_id inválido: {detail}"),
            Self::InvalidPublicKey(detail) => write!(f, "chave pública inválida: {detail}"),
            Self::InvalidSignatureEncoding(detail) => {
                write!(f, "assinatura inválida: {detail}")
            }
            Self::SignatureInvalid => write!(f, "assinatura não confere com o evento"),
            Self::OriginNotSigner {
                origin_node,
                node_id,
            } => write!(
                f,
                "origem declarada ({origin_node}) não é o nó assinante ({node_id})"
            ),
            Self::Expired {
                issued_at,
                now,
                max_age_seconds,
            } => write!(
                f,
                "evento expirado: issued_at={issued_at}, now={now}, janela={max_age_seconds}s"
            ),
            Self::NotYetValid { issued_at, now } => {
                write!(
                    f,
                    "evento emitido no futuro: issued_at={issued_at}, now={now}"
                )
            }
        }
    }
}

impl std::error::Error for SignedEventError {}

impl From<identity::IdentityError> for SignedEventError {
    fn from(e: identity::IdentityError) -> Self {
        match e {
            identity::IdentityError::InvalidPublicKey(detail) => Self::InvalidPublicKey(detail),
            identity::IdentityError::InvalidSignatureEncoding(detail) => {
                Self::InvalidSignatureEncoding(detail)
            }
            identity::IdentityError::SignatureInvalid => Self::SignatureInvalid,
            other => Self::InvalidPublicKey(other.to_string()),
        }
    }
}

fn text_field(out: &mut Vec<u8>, tag: &str, value: &str) {
    push_field(out, tag, value.as_bytes());
}

impl SignedEvent {
    /// Bytes canônicos assinados (sem `signature`). Ver docs do módulo.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(EVENT_DOMAIN);
        text_field(&mut out, "origin_node", &self.position.origin_node);
        text_field(&mut out, "tenant_id", &self.tenant_id);
        text_field(&mut out, "stream_id", &self.position.stream_id);
        push_field(
            &mut out,
            "partition",
            &self.position.partition.to_be_bytes(),
        );
        push_field(
            &mut out,
            "authority_epoch",
            &self.position.authority_epoch.to_be_bytes(),
        );
        push_field(&mut out, "seq", &self.position.seq.to_be_bytes());
        text_field(&mut out, "retention_class", &self.retention_class);
        push_field(&mut out, "issued_at", &self.issued_at.to_be_bytes());
        text_field(&mut out, "node_id", &self.node_id);
        out
    }

    /// Valida formato dos campos assinados **antes** de qualquer criptografia:
    /// campo vazio ou fora do tipo não é "assinatura inválida".
    ///
    /// # Errors
    /// [`SignedEventError::EmptyField`], [`SignedEventError::FieldTooLong`] ou
    /// [`SignedEventError::InvalidNodeId`].
    pub fn validate_fields(&self) -> Result<(), SignedEventError> {
        for (name, value) in [
            ("origin_node", &self.position.origin_node),
            ("tenant_id", &self.tenant_id),
            ("stream_id", &self.position.stream_id),
            ("retention_class", &self.retention_class),
            ("node_id", &self.node_id),
            ("signature", &self.signature),
        ] {
            if value.is_empty() {
                return Err(SignedEventError::EmptyField(name));
            }
            if value.len() > MAX_FIELD_LEN {
                return Err(SignedEventError::FieldTooLong {
                    field: name,
                    len: value.len(),
                });
            }
        }
        if !identity::is_node_id(&self.node_id) {
            return Err(SignedEventError::InvalidNodeId(self.node_id.clone()));
        }
        Ok(())
    }

    /// A origem da posição tem de ser o próprio nó que assina.
    ///
    /// Esta é a regra que fecha o buraco da fatia: sem ela, um nó autorizado
    /// assina evento **em nome de outro nó** (a origem é só um campo livre no
    /// corpo). Relay com origem preservada ponta a ponta exige assinatura
    /// encadeada (M3/M4 da emenda ADR-072) — enquanto isso não existe, quem
    /// assina é a origem, e nada além disso é afirmado.
    ///
    /// # Errors
    /// [`SignedEventError::OriginNotSigner`].
    pub fn verify_origin_is_signer(&self) -> Result<(), SignedEventError> {
        if self.position.origin_node != self.node_id {
            return Err(SignedEventError::OriginNotSigner {
                origin_node: self.position.origin_node.clone(),
                node_id: self.node_id.clone(),
            });
        }
        Ok(())
    }

    /// Verifica a assinatura contra a chave pública **do registro de pares**.
    ///
    /// Reconfere antes a ligação `node_id` ↔ chave: um registro corrompido ou
    /// forjado (chave que não deriva do `node_id`) nega, e nega como problema de
    /// identidade, não como "assinatura ruim".
    ///
    /// # Errors
    /// [`SignedEventError::InvalidPublicKey`] para chave do registro fora do
    /// formato, [`SignedEventError::SignatureInvalid`] quando o `node_id` não é
    /// o derivado da chave ou quando a assinatura não confere.
    pub fn verify_signature_against(
        &self,
        registered_public_key_hex: &str,
    ) -> Result<(), SignedEventError> {
        identity::parse_public_key(registered_public_key_hex)?;
        if !identity::verify_node_id(&self.node_id, registered_public_key_hex) {
            // Mesma classe de falha de uma assinatura que não confere: a
            // identidade provada pelo corpo não é a identidade do registro.
            return Err(SignedEventError::SignatureInvalid);
        }
        identity::verify_signature(
            registered_public_key_hex,
            &self.canonical_bytes(),
            &self.signature,
        )?;
        Ok(())
    }

    /// Janela de frescor de `issued_at`, com a mesma regra do manifesto:
    /// idade acima de `max_age_seconds` é recusa, emissão no futuro além da
    /// mesma tolerância também é recusa (relógio adiantado não estica validade).
    ///
    /// # Errors
    /// [`SignedEventError::Expired`] ou [`SignedEventError::NotYetValid`].
    pub fn verify_fresh(&self, now: i64, max_age_seconds: i64) -> Result<(), SignedEventError> {
        let age = now.saturating_sub(self.issued_at);
        if age > max_age_seconds {
            return Err(SignedEventError::Expired {
                issued_at: self.issued_at,
                now,
                max_age_seconds,
            });
        }
        if self.issued_at.saturating_sub(now) > max_age_seconds {
            return Err(SignedEventError::NotYetValid {
                issued_at: self.issued_at,
                now,
            });
        }
        Ok(())
    }

    /// Assina o envelope com a identidade do nó: preenche `node_id` e
    /// `signature` a partir da chave. O chamador **não** escolhe o `node_id` —
    /// ele é sempre o derivado da chave que assina, o que elimina assinar em
    /// nome de outro nó por descuido.
    pub fn sign_with(&mut self, identity: &NodeIdentity) {
        self.node_id = identity.node_id().to_owned();
        self.signature = identity.sign_hex(&self.canonical_bytes());
    }
}

/// Constrói o envelope assinado com a identidade do nó (porta do emissor).
///
/// Usado pelo nó local (smoke/admin, `GODMESH_NODE_KEY`) e pelos testes de fio.
/// A posição é validada aqui só em formato; a admissão contra o prefixo contíguo
/// continua sendo regra do servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDraft<'a> {
    pub tenant_id: &'a str,
    pub stream_id: &'a str,
    pub partition: u32,
    pub authority_epoch: u64,
    pub seq: u64,
    pub retention_class: &'a str,
    pub issued_at: i64,
}

#[must_use]
pub fn signed_event(identity: &NodeIdentity, draft: EventDraft<'_>) -> SignedEvent {
    let mut event = SignedEvent {
        position: EventPositionClaim {
            origin_node: identity.node_id().to_owned(),
            stream_id: draft.stream_id.to_owned(),
            partition: draft.partition,
            authority_epoch: draft.authority_epoch,
            seq: draft.seq,
        },
        tenant_id: draft.tenant_id.to_owned(),
        retention_class: draft.retention_class.to_owned(),
        issued_at: draft.issued_at,
        node_id: String::new(),
        signature: String::new(),
    };
    event.sign_with(identity);
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> NodeIdentity {
        NodeIdentity::generate_in_memory().expect("identidade em memória")
    }

    #[test]
    fn bytes_canonicos_sao_deterministicos_e_assinatura_confere() {
        let id = identity();
        let first = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        let second = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        first
            .verify_signature_against(&id.public_key_hex())
            .expect("assinatura válida");
        assert!(first.canonical_bytes().starts_with(EVENT_DOMAIN));
        // O domínio está **dentro** dos bytes: outro domínio não reusa a assinatura.
        assert!(
            !first
                .canonical_bytes()
                .starts_with(b"godmesh.federation.manifest")
        );
    }

    #[test]
    fn alterar_qualquer_campo_invalida_a_assinatura() {
        let id = identity();
        let mut event = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        event.position.seq = 2;
        assert!(matches!(
            event.verify_signature_against(&id.public_key_hex()),
            Err(SignedEventError::SignatureInvalid)
        ));

        let mut other_tenant = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        other_tenant.tenant_id = "tenant-b".to_owned();
        assert!(matches!(
            other_tenant.verify_signature_against(&id.public_key_hex()),
            Err(SignedEventError::SignatureInvalid)
        ));
    }

    #[test]
    fn origem_diferente_do_assinante_e_recusada() {
        let id = identity();
        let victim = identity();
        let mut event = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        // Nó comprometido tenta reivindicar outro nó como origem da posição.
        event.position.origin_node = victim.node_id().to_owned();
        event.node_id = id.node_id().to_owned();
        event.signature = id.sign_hex(&event.canonical_bytes());
        assert!(matches!(
            event.verify_origin_is_signer(),
            Err(SignedEventError::OriginNotSigner { .. })
        ));
        // E o caminho normal passa.
        let clean = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        clean
            .verify_origin_is_signer()
            .expect("origem é o assinante");
    }

    #[test]
    fn chave_que_nao_deriva_do_node_id_negam_sem_confundir_com_erro_de_formato() {
        let id = identity();
        let other = identity();
        let event = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        // Registro com a chave de outro nó: a ligação node_id↔chave falha.
        assert!(matches!(
            event.verify_signature_against(&other.public_key_hex()),
            Err(SignedEventError::SignatureInvalid)
        ));
        // Chave fora do formato é outro erro (diagnóstico, não negação).
        assert!(matches!(
            event.verify_signature_against("zz"),
            Err(SignedEventError::InvalidPublicKey(_))
        ));
    }

    #[test]
    fn janela_de_frescor_recusa_velho_e_futuro_alem_da_tolerancia() {
        let id = identity();
        let now = 1_700_000_000;
        let old = SignedEvent {
            issued_at: now - 301,
            ..signed_event(
                &id,
                EventDraft {
                    tenant_id: "tenant-a",
                    stream_id: "stream-1",
                    partition: 1,
                    authority_epoch: 7,
                    seq: 1,
                    retention_class: "mensagem",
                    issued_at: now - 301,
                },
            )
        };
        assert!(matches!(
            old.verify_fresh(now, 300),
            Err(SignedEventError::Expired { .. })
        ));
        let future = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: now + 301,
            },
        );
        assert!(matches!(
            future.verify_fresh(now, 300),
            Err(SignedEventError::NotYetValid { .. })
        ));
        // No limite exato da janela, aceita (mesma regra do manifesto).
        let edge = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: now - 300,
            },
        );
        edge.verify_fresh(now, 300).expect("dentro da janela");
    }

    #[test]
    fn campos_vazios_e_fora_do_formato_sao_recusados_antes_da_criptografia() {
        let id = identity();
        let event = SignedEvent {
            node_id: "gmn_curto".to_owned(),
            ..signed_event(
                &id,
                EventDraft {
                    tenant_id: "tenant-a",
                    stream_id: "stream-1",
                    partition: 1,
                    authority_epoch: 7,
                    seq: 1,
                    retention_class: "mensagem",
                    issued_at: 1_700_000_000,
                },
            )
        };
        assert!(matches!(
            event.validate_fields(),
            Err(SignedEventError::InvalidNodeId(_))
        ));
        let empty = SignedEvent {
            tenant_id: String::new(),
            ..signed_event(
                &id,
                EventDraft {
                    tenant_id: "tenant-a",
                    stream_id: "stream-1",
                    partition: 1,
                    authority_epoch: 7,
                    seq: 1,
                    retention_class: "mensagem",
                    issued_at: 1_700_000_000,
                },
            )
        };
        assert!(matches!(
            empty.validate_fields(),
            Err(SignedEventError::EmptyField("tenant_id"))
        ));
    }

    #[test]
    fn envelope_serde_recusa_campos_desconhecidos_e_reparsa_assinatura() {
        let id = identity();
        let event = signed_event(
            &id,
            EventDraft {
                tenant_id: "tenant-a",
                stream_id: "stream-1",
                partition: 1,
                authority_epoch: 7,
                seq: 1,
                retention_class: "mensagem",
                issued_at: 1_700_000_000,
            },
        );
        let json = serde_json::to_string(&event).expect("serializa");
        let parsed: SignedEvent = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(parsed, event);
        assert!(matches!(
            parsed.verify_signature_against(&id.public_key_hex()),
            Ok(())
        ));
        let with_extra = json.replace("\"seq\":1", "\"seq\":1,\"tenant_region\":\"BR\"");
        assert!(serde_json::from_str::<SignedEvent>(&with_extra).is_err());
    }
}
