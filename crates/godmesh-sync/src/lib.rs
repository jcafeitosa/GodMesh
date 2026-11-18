//! Sync durável do GodMesh — ADR-057 e ADR-061.
//!
//! Contratos verificáveis implementados aqui, todos em memória:
//!
//! - `position`: identidade de posição, ordenação por epoch e comparabilidade;
//! - `inbox`: dedupe, lacuna sem salto e ACK máximo `persisted_local`;
//! - `outbox`: prefixo contíguo confirmado, publicação separada da confirmação e
//!   plano de replay que nunca reemite efeito irreversível.
//!
//! O que **não** está implementado: durabilidade (nenhum crate aqui escreve em
//! banco ou WAL), publisher, broker, consumidor, retry real e reconstrução de
//! projeção. `outbox` guarda sequência e natureza do efeito, não payload,
//! identidade de evento nem digest, então `replay_from` produz um plano de
//! contagem, não estado reconstruído. Persistência e projeção dependem de
//! SPEC-004/SPEC-021 (`docs/08-specs/`) e de decisão de infraestrutura.

#![forbid(unsafe_code)]

pub mod inbox;
pub mod outbox;
pub mod position;