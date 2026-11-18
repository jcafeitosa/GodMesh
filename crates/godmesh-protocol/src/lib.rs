//! Contratos do GodMesh Protocol (GMP) — ADR-056…058.
//!
//! Este crate implementa apenas invariantes de contrato verificáveis em teste de
//! unidade. Não é servidor, não é wire, não é transporte. Nenhuma variante aqui
//! declara entrega, atendimento ou sucesso de domínio.

#![forbid(unsafe_code)]

pub mod ack;
pub mod cursor;
pub mod version;
