//! Servidor HTTP do GodMesh — fatia mínima do Programa A.
//!
//! A regra do endpoint de incidente é a mesma do núcleo: nada é aceito sem
//! credencial válida, sem grant verificado e sem política de retenção; a resposta
//! nunca promete mais do que a persistência local prova. Sujeito e predicado vêm
//! do grant verificado, nunca do corpo do pedido.

#![forbid(unsafe_code)]

pub mod admin;
pub mod api;
pub mod auth;
pub mod events;
pub mod http;
pub mod inbox;
pub mod incidents;
pub mod lifecycle;
pub mod metrics;
pub mod observability;
pub mod migrate;
pub mod publisher;
pub mod runtime;
