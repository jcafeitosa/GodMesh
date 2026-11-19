//! Identidade, manifesto assinado, registro de pares e transporte da federação.
//!
//! Fatias reconciliadas neste crate (ADR-071 + emenda, ADR-072 §2/§5 e emenda):
//!
//! * **M1 — ingresso**: [`identity`] (par Ed25519 persistido em arquivo `0600`,
//!   `node_id` derivado da chave pública), [`manifest`]
//!   ([`manifest::SignedManifest`] com serialização canônica definida e testada)
//!   e [`peer`] ([`peer::PeerRecord`], [`peer::TrustState`] e o registro em
//!   memória [`peer::PeerRegistry`]).
//! * **M1b — aprovação assinada**: [`approval`] ([`approval::ApprovalCertificate`],
//!   a prova de que quem aprovou é um **nó**, não um token solto) e [`genesis`]
//!   ([`genesis::Genesis`], bootstrap declarado pelo operador via
//!   `GODMESH_FEDERATION_GENESIS`). A emenda à ADR-071 §1/§5 é a norma: a
//!   autoridade de ingresso é de nó, e o bootstrap não é *trust on first use*.
//! * **M2 — transporte**: [`transport`] (QUIC + TLS 1.3 com mTLS obrigatório e
//!   validação por chave registrada, ALPN fixo `godmesh/1`), [`handshake`]
//!   (manifesto assinado nos dois sentidos, códigos de fechamento `0x474D00xx`),
//!   [`link`] (estado de enlace com `unknown` de primeira classe) e [`node`]
//!   (endpoint, aceitação, discagem e heartbeat 5 s × 3).
//! * **S-sec-2 — evento assinado**: [`event`] ([`event::SignedEvent`], mesmo
//!   padrão canônico do manifesto, domínio próprio `godmesh.federation.event.v1`)
//!   — a regra dura de `docs/04-seguranca/abuso-e-malha-maligna.md` §5: evento
//!   sem assinatura de nó autorizado não entra no log durável.
//!
//! Existe **uma** definição de `NodeIdentity`, `SignedManifest` e `PeerRecord`: a
//! de M1, que é a que o banco (`migration 0008` e `0009`) e as rotas
//! `/v1/federation/**` consomem. O transporte consome esses tipos; a única
//! extensão feita para M2 foi o campo assinado `alpn` do manifesto.
//!
//! Nada de criptografia própria: `ed25519-dalek` 3.0.0 assina e verifica,
//! `rustls`/`ring` fazem o handshake TLS, `rcgen` emite o certificado de nó.
//! Nenhuma chave privada aparece em `Debug`, log, resposta ou banco.

#![forbid(unsafe_code)]

pub mod event;
pub mod approval;
pub mod genesis;
pub mod handshake;
pub mod identity;
pub mod join_authority;
pub mod join_chain;
pub mod link;
pub mod manifest;
pub mod node;
pub mod peer;
pub mod transport;

pub use approval::{ApprovalCertificate, ApprovalError, DecisionAction};
pub use event::{EventPositionClaim, SignedEvent, SignedEventError, signed_event};
pub use genesis::{Genesis, GenesisError, GenesisNode, GENESIS_ENV};
pub use identity::{IdentityError, NodeIdentity, node_id_for, verify_node_id};
pub use manifest::{DEFAULT_MAX_AGE_SECONDS, ManifestError, SignedManifest};
pub use peer::{PeerRecord, PeerRegistry, TrustState, TrustTransitionError};

/// Instante Unix em segundos, usado em `issued_at` e `last_seen_at`.
///
/// Relógio não é autenticidade (ADR-071 §9): a janela de `issued_at` limita
/// replay, e a decisão de autorização nunca depende dele.
#[must_use]
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}