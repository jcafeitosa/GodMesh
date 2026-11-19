//! Fronteira HTTP de ingresso portátil. O ingressador recebe apenas envelopes
//! tipados e a cadeia segue, em texto, diretamente ao verificador da M1C-A.

use std::fmt;
use std::marker::PhantomData;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use godmesh_federation::SignedManifest;
use godmesh_federation::join_authority::{
    JoinRevocationReceiptV1, JoinRevocationV1, JoinValidityV1,
};
use serde::de::{MapAccess, Visitor, value::MapAccessDeserializer};
use serde::{Deserialize, Deserializer, Serialize};

use super::contract::{Access, ParamSpec, Registry, RouteDoc, Schema};
use super::{ApiErrorResponse, AppState, created_status};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::portable_join::{
    ChainEnvelope, ChainImportEnvelope, JoinSignatureRequest, PortableJoin, PortableJoinError,
    RevocationEnvelope, RevocationImportEnvelope,
};

const JOIN_ID: ParamSpec = ParamSpec {
    name: "join_request_id",
    in_path: true,
    required: true,
    ty: "string",
    desc: "Identificador do pedido de ingresso",
};
const MANIFEST: Schema = Schema::Ref("PortableManifestEnvelope");
const CHAIN: Schema = Schema::Ref("ChainEnvelope");
const VALIDITY: Schema = Schema::Ref("JoinValidityEnvelope");
const RECEIPT: Schema = Schema::Ref("JoinRevocationEnvelope");

pub(super) fn register(reg: &mut Registry) {
    let admin = Access::Scope(Scope::FederationAdmin);
    reg.get(
        "/v1/federation/local-manifest",
        admin,
        RouteDoc {
            summary: "Publica o manifesto portátil do nó",
            request: None,
            success: &[(200, MANIFEST)],
            errors: &[
                "federation_governance_disabled",
                "join_proof_invalid",
                "storage_unavailable",
            ],
            params: &[],
        },
        manifest,
    );
    reg.post(
        "/v1/federation/genesis-imports",
        admin,
        RouteDoc {
            summary: "Importa o manifesto de um genesis pinado",
            request: Some("PortableManifestEnvelope"),
            success: &[(201, MANIFEST), (200, MANIFEST)],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_issuer_not_authorized",
                "join_chain_replacement_conflict",
                "storage_unavailable",
            ],
            params: &[],
        },
        genesis,
    );
    reg.post(
        "/v1/federation/join-signatures",
        admin,
        RouteDoc {
            summary: "Assina localmente uma oferta de ingresso",
            request: Some("JoinSignatureRequest"),
            success: &[(200, CHAIN)],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_issuer_not_authorized",
                "join_validity_invalid",
                "join_proof_revoked",
                "join_envelope_capacity_exceeded",
                "storage_unavailable",
            ],
            params: &[],
        },
        sign,
    );
    reg.post(
        "/v1/federation/join-requests/{join_request_id}/approve-v2",
        admin,
        RouteDoc {
            summary: "Confirma a prova portátil na sua raiz",
            request: Some("ChainEnvelope"),
            success: &[(201, CHAIN), (200, CHAIN)],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_issuer_not_authorized",
                "join_proof_revoked",
                "join_envelope_capacity_exceeded",
                "approval_epoch_regressed",
                "join_request_not_found",
                "join_request_state_conflict",
                "storage_unavailable",
            ],
            params: &[JOIN_ID],
        },
        approve,
    );
    reg.get(
        "/v1/federation/join-requests/{join_request_id}/proof",
        admin,
        RouteDoc {
            summary: "Recupera historicamente uma prova confirmada na raiz",
            request: None,
            success: &[(200, CHAIN)],
            errors: &[
                "federation_governance_disabled",
                "join_proof_not_found",
                "storage_unavailable",
            ],
            params: &[JOIN_ID],
        },
        proof,
    );
    reg.post(
        "/v1/federation/join-validity",
        admin,
        RouteDoc {
            summary: "Emite vigência na raiz para uma prova confirmada",
            request: Some("ChainEnvelope"),
            success: &[(200, VALIDITY)],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_issuer_not_authorized",
                "join_proof_revoked",
                "join_validity_invalid",
                "storage_unavailable",
            ],
            params: &[],
        },
        validity,
    );
    reg.post(
        "/v1/federation/join-imports",
        admin,
        RouteDoc {
            summary: "Importa ou renova uma prova ativa própria ou de um par",
            request: Some("ChainImportEnvelope"),
            success: &[
                (201, Schema::Ref("PortableJoinImportResult")),
                (200, Schema::Ref("PortableJoinImportResult")),
            ],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_issuer_not_authorized",
                "join_proof_revoked",
                "join_validity_invalid",
                "join_chain_replacement_conflict",
                "storage_unavailable",
            ],
            params: &[],
        },
        import,
    );
    reg.post(
        "/v1/federation/join-revocations",
        admin,
        RouteDoc {
            summary: "Confirma revogação de um elo na raiz e emite o recibo",
            request: Some("RevocationEnvelope"),
            success: &[(201, RECEIPT), (200, RECEIPT)],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_issuer_not_authorized",
                "join_proof_revoked",
                "join_revocation_invalid",
                "join_revocation_conflict",
                "approval_epoch_regressed",
                "storage_unavailable",
            ],
            params: &[],
        },
        revoke,
    );
    reg.post(
        "/v1/federation/join-revocation-imports",
        admin,
        RouteDoc {
            summary: "Aplica bloqueio local por recibo da raiz",
            request: Some("RevocationImportEnvelope"),
            success: &[(201, RECEIPT), (200, RECEIPT)],
            errors: &[
                "invalid_request",
                "federation_governance_disabled",
                "join_proof_invalid",
                "join_revocation_invalid",
                "join_revocation_conflict",
                "storage_unavailable",
            ],
            params: &[],
        },
        import_revocation,
    );
}

impl From<PortableJoinError> for ApiErrorResponse {
    fn from(error: PortableJoinError) -> Self {
        ApiError::from(error).into()
    }
}

fn service(state: &AppState) -> Result<&PortableJoin, ApiErrorResponse> {
    state
        .portable
        .as_deref()
        .ok_or_else(|| PortableJoinError::Disabled.into())
}

struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("um envelope JSON em forma de objeto")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

type Input<T> = Result<Json<Object<T>>, JsonRejection>;

fn envelope<T>(body: Input<T>) -> Result<T, ApiErrorResponse> {
    body.map(|Json(Object(inner))| inner)
        .map_err(|error| ApiError::InvalidRequest(error.body_text()).into())
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PortableManifestEnvelope {
    manifest: SignedManifest,
}

#[derive(Serialize)]
struct JoinValidityEnvelope {
    validity: JoinValidityV1,
}

#[derive(Serialize)]
struct JoinRevocationEnvelope {
    revocation: JoinRevocationV1,
    receipt: JoinRevocationReceiptV1,
}

#[derive(Serialize, utoipa::ToSchema)]
struct PortableJoinImportResult {
    imported: bool,
}

async fn manifest(
    State(state): State<AppState>,
) -> Result<Json<PortableManifestEnvelope>, ApiErrorResponse> {
    Ok(Json(PortableManifestEnvelope {
        manifest: service(&state)?.local_manifest()?,
    }))
}

async fn genesis(
    State(state): State<AppState>,
    body: Input<PortableManifestEnvelope>,
) -> Result<(StatusCode, Json<PortableManifestEnvelope>), ApiErrorResponse> {
    let value = envelope(body)?;
    let created = service(&state)?
        .import_genesis(&value.manifest, state.join_window)
        .await?;
    Ok((created_status(created), Json(value)))
}

async fn sign(
    State(state): State<AppState>,
    body: Input<JoinSignatureRequest>,
) -> Result<Json<ChainEnvelope>, ApiErrorResponse> {
    let request = envelope(body)?;
    let chain_json = service(&state)?
        .sign_offer(
            request,
            state.join_window,
            state.admission.config().max_body_bytes,
        )
        .await?;
    Ok(Json(ChainEnvelope { chain_json }))
}

async fn approve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(grant): Extension<VerifiedGrant>,
    body: Input<ChainEnvelope>,
) -> Result<(StatusCode, Json<ChainEnvelope>), ApiErrorResponse> {
    let request = envelope(body)?;
    let (created, chain_json) = service(&state)?
        .approve(
            &id,
            &request.chain_json,
            &grant.subject.principal,
            state.join_window,
            state.admission.config().max_body_bytes,
        )
        .await
        .map_err(|error| match error {
            PortableJoinError::NotFound => ApiError::JoinRequestNotFound,
            other => other.into(),
        })?;
    Ok((created_status(created), Json(ChainEnvelope { chain_json })))
}

async fn proof(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ChainEnvelope>, ApiErrorResponse> {
    let chain_json = service(&state)?.proof(&id).await?;
    Ok(Json(ChainEnvelope { chain_json }))
}

async fn validity(
    State(state): State<AppState>,
    body: Input<ChainEnvelope>,
) -> Result<Response, ApiErrorResponse> {
    let request = envelope(body)?;
    let validity = service(&state)?.issue_validity(&request.chain_json).await?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(JoinValidityEnvelope { validity }),
    )
        .into_response())
}

async fn import(
    State(state): State<AppState>,
    body: Input<ChainImportEnvelope>,
) -> Result<(StatusCode, Json<PortableJoinImportResult>), ApiErrorResponse> {
    let request = envelope(body)?;
    let created = service(&state)?
        .import_chain(
            &request.expected_node_id,
            &request.expected_public_key,
            &request.chain_json,
            &request.validity,
        )
        .await?;
    Ok((
        created_status(created),
        Json(PortableJoinImportResult { imported: true }),
    ))
}

async fn revoke(
    State(state): State<AppState>,
    body: Input<RevocationEnvelope>,
) -> Result<(StatusCode, Json<JoinRevocationEnvelope>), ApiErrorResponse> {
    let request = envelope(body)?;
    let (created, receipt) = service(&state)?
        .revoke(&request.target_chain_json, &request.revocation)
        .await?;
    Ok((
        created_status(created),
        Json(JoinRevocationEnvelope {
            revocation: request.revocation,
            receipt,
        }),
    ))
}

async fn import_revocation(
    State(state): State<AppState>,
    body: Input<RevocationImportEnvelope>,
) -> Result<(StatusCode, Json<JoinRevocationEnvelope>), ApiErrorResponse> {
    let value = envelope(body)?;
    let created = service(&state)?
        .import_revocation(&value.target_chain_json, &value.revocation, &value.receipt)
        .await?;
    Ok((
        created_status(created),
        Json(JoinRevocationEnvelope {
            revocation: value.revocation,
            receipt: value.receipt,
        }),
    ))
}

pub(super) fn schemas(
    builder: utoipa::openapi::ComponentsBuilder,
) -> utoipa::openapi::ComponentsBuilder {
    builder
        .schema_from::<PortableManifestEnvelope>()
        .schema_from::<PortableJoinImportResult>()
        .schema_from::<JoinSignatureRequest>()
        .schema_from::<ChainEnvelope>()
}
