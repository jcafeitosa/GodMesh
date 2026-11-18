//! Servidor HTTP do GodMesh — fatia mínima do Programa A.
//!
//! A regra do endpoint de incidente é a mesma do núcleo: nada é aceito sem grant
//! válido e sem política de retenção; a resposta nunca promete mais do que a
//! persistência local prova.

#![forbid(unsafe_code)]

pub mod api;
pub mod http;
