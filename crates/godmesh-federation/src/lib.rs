//! Identidade, manifesto assinado e registro de pares da federação GodMesh.
//!
//! Esta é a primeira fatia da malha (ADR-071): identidade de nó, manifesto
//! assinado, registro de pares e ingresso por API. **Não há transporte aqui** —
//! QUIC/mTLS é a fatia seguinte (M2). O que existe é o que dá para afirmar sem
//! rede de pares:
//!
//! * [`identity`] — par Ed25519 persistido em arquivo `0600`, `node_id` derivado
//!   da chave pública, assinatura e verificação;
//! * [`manifest`] — [`manifest::SignedManifest`] com serialização canônica
//!   definida e testada;
//! * [`peer`] — [`peer::PeerRecord`] e a máquina de estados de confiança.
//!
//! Nada de criptografia própria: `ed25519-dalek` 3.0.0 assina e verifica. Nenhuma
//! chave privada aparece em `Debug`, log, resposta ou banco.

#![forbid(unsafe_code)]

pub mod identity;
pub mod manifest;
pub mod peer;

pub use identity::{IdentityError, NodeIdentity, node_id_for, verify_node_id};
pub use manifest::{DEFAULT_MAX_AGE_SECONDS, ManifestError, SignedManifest};
pub use peer::{PeerRecord, TrustState, TrustTransitionError};
