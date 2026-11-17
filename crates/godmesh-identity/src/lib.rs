//! Identidade e autorização do GodMesh — ADR-059 e ADR-064.
//!
//! O predicado do grant offline é a peça que impede replay e ação sem autoridade.
//! Falhar aqui é negar; não existe caminho de "assume válido".

#![forbid(unsafe_code)]

pub mod grant;
