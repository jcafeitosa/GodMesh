//! Políticas transversais do GodMesh — retenção (ADR-062/065) e prova de
//! anti-rollback do grant offline (ADR-064).
//!
//! Ambas as políticas falham fechadas: ausência de política ou de prova nega.

#![forbid(unsafe_code)]

pub mod high_water;
pub mod retention;
