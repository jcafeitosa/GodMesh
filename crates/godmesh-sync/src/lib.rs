//! Sync durável do GodMesh — ADR-057 e ADR-061.
//!
//! Contratos verificáveis implementados aqui:
//!
//! - `position`: identidade de posição, ordenação por epoch e comparabilidade;
//! - `inbox`: dedupe, lacuna sem salto e ACK máximo `persisted_local`;
//! - `outbox`: modelo em memória do prefixo contíguo confirmado, da publicação
//!   separada da confirmação e do plano de replay que nunca reemite efeito
//!   irreversível;
//! - `pg_outbox`: o **mesmo contrato sobre a tabela `outbox_entry`** (migrações
//!   0001 e 0003), com append transacional, reivindicação atômica de publicação,
//!   confirmação em ordem, contagem de tentativas e o sink de teste
//!   `outbox_sink`.
//!
//! O que **não** está implementado: broker, bridge, consumidor, retry ilimitado,
//! reconstrução de projeção e entrega a efeito externo real. `outbox` e
//! `outbox_entry` guardam sequência e natureza do efeito, não payload, identidade
//! de evento nem digest, então `replay_from` produz um plano de contagem, não
//! estado reconstruído. Projeção e integração externa dependem de
//! SPEC-004/SPEC-021 (`docs/08-specs/`) e de decisão de infraestrutura.

#![forbid(unsafe_code)]

pub mod inbox;
pub mod outbox;
pub mod pg_outbox;
pub mod position;