//! Emergência — SOS como evento estruturado, não como mensagem (ADR-039/058/064).
//!
//! Este crate modela a máquina de estados do incidente e o que pode ser alegado em
//! cada etapa. Não envia, não persiste e não atende: garante que nenhuma camada
//! afirme mais do que a evidência permite.

#![forbid(unsafe_code)]

pub mod incident;
