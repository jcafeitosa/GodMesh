//! Sync durável do GodMesh — ADR-057 e ADR-061.
//!
//! Somente contratos verificáveis: identidade de posição, ordenação por epoch,
//! detecção de lacuna. Publicação, Inbox e projeção dependem de SPEC-004/SPEC-021
//! e de decisão de infraestrutura; não são implementados aqui.

#![forbid(unsafe_code)]

pub mod inbox;
pub mod outbox;
pub mod position;
