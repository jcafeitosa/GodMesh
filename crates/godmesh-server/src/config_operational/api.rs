use std::{future::poll_fn, pin::Pin, sync::Arc};

use axum::{
    Router,
    body::{Body, HttpBody},
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use godmesh_federation::config_bundle::authenticate_bundle;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ConfigPlane, Peer};
use crate::{
    admission::RequestClass,
    auth::{AuthError, GrantRecord, Scope, bearer_token},
    config_bundle::{Published, Rejection, WriteError, WriteOutcome},
    portable_join::{ClusterGuard, PortableJoinError},
};

pub(super) const PATH: &str = "/v1/federation/config";
const CACHE: &str = "private, no-cache";
type Hash = [u8; 32];

#[derive(Clone, Copy)]
pub(super) enum ErrorCode {
    BadRequest,
    Grant,
    Forbidden,
    Large,
    BodyTimeout,
    Rate,
    Unavailable,
    ConflictRead,
    ConflictWrite,
    Rollback,
    Invalid,
    Software,
    Storage,
}

const ERRORS: &[ErrorCode] = &[
    ErrorCode::BadRequest,
    ErrorCode::Grant,
    ErrorCode::Forbidden,
    ErrorCode::Large,
    ErrorCode::BodyTimeout,
    ErrorCode::Rate,
    ErrorCode::Unavailable,
    ErrorCode::ConflictRead,
    ErrorCode::ConflictWrite,
    ErrorCode::Rollback,
    ErrorCode::Invalid,
    ErrorCode::Software,
    ErrorCode::Storage,
];

impl ErrorCode {
    fn contract(self) -> (u16, &'static str, &'static str, bool) {
        match self {
            Self::BadRequest => (
                400,
                "config_bad_request",
                "Confira escopo, query e ETag.",
                false,
            ),
            Self::Grant => (401, "unauthenticated", "Apresente um grant válido.", false),
            Self::Forbidden => (
                403,
                "config_access_denied",
                "Confira papel, autoridade M1C e permissões.",
                false,
            ),
            Self::Large => (
                413,
                "payload_too_large",
                "Respeite o teto de corpo do listener.",
                false,
            ),
            Self::BodyTimeout => (
                408,
                "request_timeout",
                "Envie o corpo dentro do prazo.",
                true,
            ),
            Self::Rate => (429, "rate_limited", "Aguarde Retry-After.", true),
            Self::Unavailable => (
                404,
                "config_unavailable",
                "Aguarde uma publicação vigente.",
                false,
            ),
            Self::ConflictRead => (
                503,
                "config_conflict",
                "Resolva o conflito pelo operador autorizado.",
                false,
            ),
            Self::ConflictWrite => (
                409,
                "config_conflict",
                "Resolva o conflito pelo operador autorizado.",
                false,
            ),
            Self::Rollback => (
                409,
                "config_rollback",
                "Use versão superior à observada.",
                false,
            ),
            Self::Invalid => (
                422,
                "config_bundle_invalid",
                "Confira assinatura, escopo, digest e janela.",
                false,
            ),
            Self::Software => (
                422,
                "config_incompatible_software",
                "Confira a versão mínima de software.",
                false,
            ),
            Self::Storage => (
                503,
                "storage_unavailable",
                "Repita após recuperar o PostgreSQL.",
                true,
            ),
        }
    }
}

struct Failure {
    code: ErrorCode,
    effect_unknown: bool,
    retry_after: Option<u64>,
}

impl From<ErrorCode> for Failure {
    fn from(code: ErrorCode) -> Self {
        Self {
            code,
            effect_unknown: false,
            retry_after: None,
        }
    }
}

impl From<sqlx::Error> for Failure {
    fn from(_: sqlx::Error) -> Self {
        ErrorCode::Storage.into()
    }
}

impl From<PortableJoinError> for Failure {
    fn from(error: PortableJoinError) -> Self {
        match error {
            PortableJoinError::Storage(_) | PortableJoinError::Encoding(_) => ErrorCode::Storage,
            _ => ErrorCode::Forbidden,
        }
        .into()
    }
}

impl From<WriteError> for Failure {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Transient(_) => ErrorCode::Storage,
            WriteError::Rejected(Rejection::WrongRole | Rejection::LocalAuthority) => {
                ErrorCode::Forbidden
            }
            WriteError::Rejected(Rejection::Rollback) => ErrorCode::Rollback,
            WriteError::Rejected(Rejection::ConflictOpen) => ErrorCode::ConflictWrite,
            WriteError::Rejected(_) => ErrorCode::Invalid,
        }
        .into()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ErrorBody {
    pub error: String,
    detail: String,
    hint: String,
    docs_url: String,
    pub retryable: bool,
    ack_class: String,
    #[serde(
        default,
        deserialize_with = "read_effect",
        skip_serializing_if = "Option::is_none"
    )]
    effect: Option<String>,
}

fn read_effect<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Option<String>, D::Error> {
    String::deserialize(input).map(Some)
}

pub(super) fn parse_remote_error(headers: &HeaderMap, body: &[u8]) -> Option<ErrorBody> {
    if !json_media(headers)
        || !body
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .is_some_and(|b| *b == b'{')
    {
        return None;
    }
    let error: ErrorBody = serde_json::from_slice(body).ok()?;
    let (_, _, _, retryable) = match error.error.as_str() {
        "config_conflict" => ErrorCode::ConflictRead,
        "storage_unavailable" => ErrorCode::Storage,
        _ => return None,
    }
    .contract();
    if error.detail.trim().is_empty()
        || error.hint.trim().is_empty()
        || error.docs_url != "/v1/meta"
        || error.ack_class != "transport_accepted"
        || error.retryable != retryable
        || error
            .effect
            .as_deref()
            .is_some_and(|effect| effect != "unknown" || !retryable)
    {
        return None;
    }
    Some(error)
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let (status, error, hint, retryable) = self.code.contract();
        let status = StatusCode::from_u16(status).expect("status do catálogo");
        let mut response = json_response(
            status,
            &ErrorBody {
                error: error.into(),
                detail: hint.into(),
                hint: hint.into(),
                docs_url: "/v1/meta".into(),
                retryable,
                ack_class: "transport_accepted".into(),
                effect: self.effect_unknown.then(|| "unknown".into()),
            },
        );
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        if let Some(after) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(after));
        }
        response
    }
}

pub(crate) fn transport_refusal(error: crate::config_https::HttpsError) -> Response {
    use crate::config_https::HttpsError;
    let code = match error {
        HttpsError::BodyTooLarge => ErrorCode::Large,
        HttpsError::Timeout => ErrorCode::BodyTimeout,
        _ => ErrorCode::BadRequest,
    };
    Failure::from(code).into_response()
}

fn json_response(status: StatusCode, body: &impl Serialize) -> Response {
    bytes_response(
        status,
        serde_json::to_vec(body).expect("UI de configuração serializável"),
    )
}

fn bytes_response(status: StatusCode, body: Vec<u8>) -> Response {
    let length = body.len();
    let mut response = (status, Body::from(body)).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    response
}

pub(super) fn etag(hash: &Hash) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{}\"", hex::encode(hash))).expect("ETag hex")
}

fn lower_hash(raw: &str) -> Option<Hash> {
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut hash = [0; 32];
    hex::decode_to_slice(raw, &mut hash).ok()?;
    Some(hash)
}

pub(super) fn read_etag(headers: &HeaderMap, name: HeaderName) -> Result<Option<Hash>, ()> {
    let mut raw = headers.get_all(name).iter();
    let Some(value) = raw.next() else {
        return Ok(None);
    };
    if raw.next().is_some() {
        return Err(());
    }
    let value = value.to_str().map_err(|_| ())?;
    let digest = value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .and_then(lower_hash);
    digest.map(Some).ok_or(())
}

pub(super) fn json_media(headers: &HeaderMap) -> bool {
    let mut content_types = headers.get_all(header::CONTENT_TYPE).iter();
    content_types
        .next()
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"application/json"))
        && content_types.next().is_none()
        && !headers.contains_key(header::CONTENT_ENCODING)
}

fn query_field(value: &str) -> Option<String> {
    let mut result = Vec::with_capacity(value.len());
    let mut bytes = value.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => result.push(b' '),
            b'%' => {
                let a = (bytes.next()? as char).to_digit(16)?;
                let b = (bytes.next()? as char).to_digit(16)?;
                result.push((a * 16 + b) as u8);
            }
            other => result.push(other),
        }
    }
    String::from_utf8(result).ok()
}

fn query_pairs(query: &str) -> Result<std::collections::BTreeMap<String, String>, Failure> {
    let mut values = std::collections::BTreeMap::new();
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').ok_or(ErrorCode::BadRequest)?;
        let key = query_field(key).ok_or(ErrorCode::BadRequest)?;
        let value = query_field(value).ok_or(ErrorCode::BadRequest)?;
        if values.insert(key, value).is_some() {
            return Err(ErrorCode::BadRequest.into());
        }
    }
    Ok(values)
}

fn get_scope(query: Option<&str>, cluster: &str) -> Result<(), Failure> {
    let pairs = query_pairs(query.ok_or(ErrorCode::BadRequest)?)?;
    if pairs.len() != 3
        || pairs.get("cluster").map(String::as_str) != Some(cluster)
        || pairs.get("namespace").map(String::as_str) != Some("federation")
        || pairs.get("scope_id").map(String::as_str) != Some("")
    {
        return Err(ErrorCode::BadRequest.into());
    }
    Ok(())
}

fn resolve_hash(query: Option<&str>) -> Result<Option<Hash>, Failure> {
    let Some(query) = query else {
        return Ok(None);
    };
    let pairs = query_pairs(query)?;
    if pairs.len() != 2 || pairs.get("resolve_conflict").map(String::as_str) != Some("true") {
        return Err(ErrorCode::BadRequest.into());
    }
    pairs
        .get("expected_hash")
        .and_then(|s| lower_hash(s))
        .map(Some)
        .ok_or_else(|| ErrorCode::BadRequest.into())
}

async fn read_body(mut body: Body, limit: usize) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
        let frame = frame.map_err(|_| ErrorCode::BadRequest)?;
        if let Ok(data) = frame.into_data() {
            if data.len() > limit - bytes.len() {
                return Err(ErrorCode::Large.into());
            }
            bytes.extend_from_slice(&data);
        }
    }
    Ok(bytes)
}

fn peer(request: &Request) -> Result<Peer, Failure> {
    request
        .extensions()
        .get::<Peer>()
        .cloned()
        .ok_or_else(|| ErrorCode::Forbidden.into())
}

fn rate(result: crate::admission::RateDecision) -> Result<(), Failure> {
    result.map_err(|after| Failure {
        retry_after: Some(after),
        ..ErrorCode::Rate.into()
    })
}

impl ConfigPlane {
    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(PATH, get(read_config).merge(put(write_config)))
            .route("/v1/meta", get(meta))
            .route("/openapi.json", get(openapi))
            .with_state(self.clone())
    }
}

async fn read_config(
    State(plane): State<Arc<ConfigPlane>>,
    request: Request,
) -> Result<Response, Failure> {
    let peer = peer(&request)?;
    let _flight = plane.admission.enter();
    rate(
        plane
            .admission
            .check_anonymous(&peer.remote_addr.ip().to_string(), RequestClass::Incident),
    )?;
    get_scope(request.uri().query(), plane.portable.cluster())?;
    let conditional =
        read_etag(request.headers(), header::IF_NONE_MATCH).map_err(|()| ErrorCode::BadRequest)?;
    let mut tx = plane.begin().await?;
    let chosen = async {
        let mut guard = plane.portable.lock_cluster_guard(&mut tx).await?;
        let now = plane.clock.now_unix();
        plane.authorize_peer(&mut guard, &peer, now).await?;
        if plane.config.readers.get(&peer.node_id) != Some(&peer.public_key) {
            return Err(Failure::from(ErrorCode::Forbidden));
        }
        Ok(plane
            .bundle
            .select_published_in_tx(&mut guard, conditional.as_ref(), now)
            .await?)
    }
    .await;
    match chosen {
        Ok(chosen) => {
            let response = match chosen {
                Published::Frame { frame, hash, .. } => {
                    cached(bytes_response(StatusCode::OK, frame), &hash)
                }
                Published::NotModified { hash, .. } => {
                    cached(StatusCode::NOT_MODIFIED.into_response(), &hash)
                }
                Published::Unavailable => Failure::from(ErrorCode::Unavailable).into_response(),
                Published::BlockedConflict => {
                    Failure::from(ErrorCode::ConflictRead).into_response()
                }
            };
            tx.commit().await?;
            Ok(response)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

fn cached(mut response: Response, hash: &Hash) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::ETAG, etag(hash));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE));
    response
}

async fn config_grant(plane: &ConfigPlane, headers: &HeaderMap) -> Result<GrantRecord, Failure> {
    let mut credentials = headers.get_all(header::AUTHORIZATION).iter();
    if credentials.next().is_none() || credentials.next().is_some() {
        return Err(ErrorCode::Grant.into());
    }
    let token = bearer_token(headers).ok_or(ErrorCode::Grant)?;
    plane
        .authorization
        .resolve_config_grant(token)
        .await
        .map_err(|error| {
            match error {
                AuthError::IssuanceUnavailable | AuthError::RevocationUnavailable => {
                    ErrorCode::Storage
                }
                _ => ErrorCode::Grant,
            }
            .into()
        })
}

async fn check_grant(
    guard: &mut ClusterGuard<'_, '_>,
    grant: &GrantRecord,
    now: i64,
) -> Result<(), Failure> {
    if grant.revoked_at.is_some()
        || now < grant.not_before
        || now >= grant.expires_at
        || grant.verified().validate().is_err()
    {
        return Err(ErrorCode::Grant.into());
    }
    // A administração legada grava com ROW EXCLUSIVE. Este lock mantém a
    // decisão sobre uma revogação, inclusive a primeira, até o commit do PUT.
    sqlx::query("LOCK TABLE grant_revocation IN SHARE MODE")
        .execute(guard.connection())
        .await?;
    let revoked: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM grant_revocation WHERE token_id = $1)")
            .bind(&grant.token_id)
            .fetch_one(guard.connection())
            .await?;
    if revoked {
        return Err(ErrorCode::Grant.into());
    }
    if !grant.has_scope(Scope::FederationAdmin) {
        return Err(ErrorCode::Forbidden.into());
    }
    Ok(())
}

async fn write_config(
    State(plane): State<Arc<ConfigPlane>>,
    request: Request,
) -> Result<Response, Failure> {
    let peer = peer(&request)?;
    let _flight = plane.admission.enter();
    rate(
        plane
            .admission
            .check_anonymous(&peer.remote_addr.ip().to_string(), RequestClass::Incident),
    )?;
    let expected = resolve_hash(request.uri().query())?;
    let grant = config_grant(&plane, request.headers()).await?;
    rate(plane.admission.check_request(
        &grant.subject.tenant_id,
        &grant.token_id,
        RequestClass::Incident,
    ))?;
    if !json_media(request.headers()) {
        return Err(ErrorCode::Invalid.into());
    }
    let limit = plane.config.max_body_bytes.get();
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|size| size > limit as u64)
    {
        return Err(ErrorCode::Large.into());
    }
    let raw = tokio::time::timeout(
        plane.config.body_timeout,
        read_body(request.into_body(), limit),
    )
    .await
    .map_err(|_| ErrorCode::BodyTimeout)??;
    let authenticated =
        authenticate_bundle(&raw, &plane.config.policy.bundle).map_err(|_| ErrorCode::Invalid)?;
    if expected.is_some_and(|digest| &digest != authenticated.hash()) {
        return Err(ErrorCode::Invalid.into());
    }
    let mut tx = plane.begin().await?;
    let written = async {
        let mut guard = plane.portable.lock_cluster_guard(&mut tx).await?;
        let now = plane.clock.now_unix();
        plane.authorize_peer(&mut guard, &peer, now).await?;
        let permitted = if expected.is_some() {
            &plane.config.resolvers
        } else {
            &plane.config.submitters
        };
        if permitted.get(&peer.node_id) != Some(&peer.public_key) {
            return Err(ErrorCode::Forbidden.into());
        }
        check_grant(&mut guard, &grant, now).await?;
        Ok(if expected.is_some() {
            plane
                .bundle
                .resolve_conflict_in_tx(
                    &mut guard,
                    &plane.portable,
                    &raw,
                    plane.config.policy.role,
                    &grant.subject.principal,
                    now,
                )
                .await?
        } else {
            plane
                .bundle
                .publish_in_tx(&mut guard, &plane.portable, &raw, now)
                .await?
        })
    }
    .await;
    let outcome = match written {
        Ok(outcome) => outcome,
        Err(error) => {
            tx.rollback().await?;
            return Err(error);
        }
    };
    let result = match outcome {
        WriteOutcome::Accepted | WriteOutcome::Replay => json_response(
            if outcome == WriteOutcome::Accepted {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            &json!({"version":authenticated.body().version,"hash":hex::encode(authenticated.hash())}),
        ),
        WriteOutcome::ConflictRecorded | WriteOutcome::ConflictKnown => {
            Failure::from(ErrorCode::ConflictWrite).into_response()
        }
        WriteOutcome::IncompatibleRecorded | WriteOutcome::IncompatibleKnown => {
            Failure::from(ErrorCode::Software).into_response()
        }
    };
    plane.commit_write(tx, outcome).await.map_err(|_| Failure {
        effect_unknown: true,
        ..ErrorCode::Storage.into()
    })?;
    Ok(result)
}

async fn meta(State(plane): State<Arc<ConfigPlane>>) -> Response {
    let mut errors = Vec::new();
    for code in ERRORS {
        let (status, error, hint, retryable) = code.contract();
        errors.push(json!({"http_status":status,"error":error,"hint":hint,"retryable":retryable}));
    }
    json_response(
        StatusCode::OK,
        &json!({
            "routes":[
                {"path":PATH,"method":"GET","authentication":"mTLS","permission":"reader"},
                {"path":PATH,"method":"PUT","authentication":"mTLS","scope":"federation:admin",
                 "permission":"submitter or resolver","max_body_bytes":plane.config.max_body_bytes.get()}
            ],
            "errors":errors
        }),
    )
}

fn documented_responses(errors: &[ErrorCode], successes: &[(u16, &str)]) -> Value {
    let mut responses = serde_json::Map::new();
    for (status, description) in successes {
        responses.insert(status.to_string(), json!({"description":description}));
    }
    for code in errors {
        let (status, error, hint, _) = code.contract();
        let entry = responses.entry(status.to_string()).or_insert_with(|| {
            json!({
                "description":"",
                "content":{"application/json":{"schema":{"$ref":"#/components/schemas/Error"}}}
            })
        });
        let existing = entry["description"].as_str().unwrap_or_default();
        entry["description"] = json!(format!("{existing}{error} {hint} "));
    }
    Value::Object(responses)
}

async fn openapi(State(plane): State<Arc<ConfigPlane>>) -> Response {
    let mut get = documented_responses(
        &[
            ErrorCode::BadRequest,
            ErrorCode::Forbidden,
            ErrorCode::Unavailable,
            ErrorCode::ConflictRead,
            ErrorCode::Storage,
            ErrorCode::Rate,
        ],
        &[(200, "Frame M3-A"), (304, "Sem corpo")],
    );
    get["200"]["content"] = json!({"application/json":{"schema":{"type":"object"}}});
    for code in ["200", "304"] {
        get[code]["headers"] = json!({
            "ETag":{"schema":{"type":"string"}},
            "Cache-Control":{"schema":{"const":CACHE}}
        });
    }
    let mut put = documented_responses(
        &[
            ErrorCode::BadRequest,
            ErrorCode::Forbidden,
            ErrorCode::Grant,
            ErrorCode::ConflictWrite,
            ErrorCode::Rollback,
            ErrorCode::Invalid,
            ErrorCode::Software,
            ErrorCode::Storage,
            ErrorCode::Large,
            ErrorCode::Rate,
            ErrorCode::BodyTimeout,
        ],
        &[(200, "Replay confirmado"), (201, "Bundle confirmado")],
    );
    for code in ["200", "201"] {
        put[code]["content"] = json!({"application/json":{"schema":{
            "type":"object","required":["version","hash"],"additionalProperties":false,
            "properties":{"version":{"type":"integer","format":"int64","minimum":1},
                "hash":{"type":"string","pattern":"^[0-9a-f]{64}$"}}
        }}});
    }
    json_response(
        StatusCode::OK,
        &json!({
            "openapi":"3.1.0","info":{"title":"GodMesh Config HTTPS","version":"1"},
            "components":{"securitySchemes":{"peer":{"type":"mutualTLS"},"grant":{"type":"http","scheme":"bearer"}},
                "schemas":{"Error":{"type":"object","additionalProperties":false,
                    "required":["error","detail","hint","docs_url","retryable","ack_class"],
                    "properties":{"error":{"type":"string"},"detail":{"type":"string"},"hint":{"type":"string"},
                        "docs_url":{"type":"string"},"retryable":{"type":"boolean"},
                        "ack_class":{"const":"transport_accepted"},"effect":{"const":"unknown"}}}}},
            "paths":{PATH:{
                "get":{"security":[{"peer":[]}],"responses":get,"parameters":[
                    {"name":"cluster","in":"query","required":true,"schema":{"type":"string"}},
                    {"name":"namespace","in":"query","required":true,"schema":{"const":"federation"}},
                    {"name":"scope_id","in":"query","required":true,"allowEmptyValue":true,"schema":{"const":""}},
                    {"name":"If-None-Match","in":"header","schema":{"type":"string"}}
                ]},
                "put":{"security":[{"peer":[],"grant":[]}],"responses":put,
                    "x-max-body-bytes":plane.config.max_body_bytes.get(),
                    "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},
                    "parameters":[
                        {"name":"resolve_conflict","in":"query","schema":{"const":"true"}},
                        {"name":"expected_hash","in":"query","schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}}
                    ]}
            }}
        }),
    )
}
