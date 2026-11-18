//! Portões de abuso aplicados **antes** dos handlers: payload, timeout, cotas e
//! admissão por classe. Nada aqui conhece regra de domínio — o módulo só recusa
//! pedidos que não passam pelo portão, e nunca escreve.
//!
//! Dois middlewares, cada um no ponto da cadeia onde tem as informações do seu
//! par (ver `super::build_router`):
//!
//! * [`http_gate`] — camada mais externa com estado: classifica pela URL, aplica
//!   o teto de corpo (413 antes de desserializar), a cota **por IP** para pedidos
//!   sem credencial (429) e o timeout de requisição (504).
//! * [`grant_gate`] — roda depois de `require_grant`: conhece o grant verificado,
//!   então aplica a cota por *(tenant, token, classe)* (429), a cota de eventos
//!   por escopo (429) e a recusa por pressão para classes baixas (503).
//!
//! Ordem resultante em uma rota `/v1`: `observe → http_gate → require_grant →
//! grant_gate → portão de escopo → handler`. Todas as recusas acontecem antes
//! de qualquer INSERT e devolvem o mesmo formato de erro do resto da API.

use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{self, HeaderValue};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::AppState;
use crate::admission::{CLASS_HEADER, RequestClass, RoutePolicy, inbox_event_cost, route_class};
use crate::auth::VerifiedGrant;

/// Formato estável de recusa do portão — os mesmos campos de todo erro da API
/// (`error`, `detail`, `ack_class`), mais `Retry-After` quando o cliente tem
/// o que esperar. `ack_class` é o piso honesto: nada foi confirmado.
fn refusal(status: StatusCode, code: &str, detail: &str, retry_after: Option<u64>) -> Response {
    let body = json!({
        "error": code,
        "detail": detail,
        "ack_class": "transport_accepted",
    });
    let mut response = (status, axum::Json(body)).into_response();
    if let Some(secs) = retry_after
        && let Ok(value) = HeaderValue::from_str(&secs.to_string())
    {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

fn payload_too_large(limit: usize) -> Response {
    refusal(
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
        &format!("corpo acima do teto declarado de {limit} bytes; nada foi lido ou gravado"),
        None,
    )
}

fn rate_limited(retry_after: u64) -> Response {
    refusal(
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "cota do grant excedida; nenhuma escrita foi tentada",
        Some(retry_after),
    )
}

fn admission_shed(retry_after: u64) -> Response {
    refusal(
        StatusCode::SERVICE_UNAVAILABLE,
        "admission_control",
        "servidor sob pressão; esta classe de tráfego foi recusada agora",
        Some(retry_after),
    )
}

fn request_timeout() -> Response {
    refusal(
        StatusCode::GATEWAY_TIMEOUT,
        "request_timeout",
        "a requisição excedeu o timeout declarado; nada foi confirmado — reenvie \
         com a mesma idempotência",
        None,
    )
}

fn invalid_class(raw: &str) -> Response {
    refusal(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_request",
        &format!(
            "classe de tráfego {raw:?} desconhecida; valores: \
             sos, ptt, incident, event, sync, metrics"
        ),
        None,
    )
}

/// Classe efetiva do pedido: header declarado se presente, senão a tabela de
/// rotas. Header com valor desconhecido é recusado (422), nunca reinterpretado.
fn effective_class(request: &Request, table: RoutePolicy) -> Result<RequestClass, Box<Response>> {
    match request.headers().get(CLASS_HEADER) {
        None => Ok(table.class),
        Some(raw) => match raw.to_str() {
            Ok(text) => RequestClass::parse(text)
                .ok_or_else(|| Box::new(invalid_class(text))),
            Err(_) => Err(Box::new(invalid_class("<bytes não-UTF8>"))),
        },
    }
}

fn method_may_carry_body(method: &Method) -> bool {
    matches!(*method, Method::POST | Method::PUT | Method::PATCH)
}

/// Camada externa do portão: payload, cota pré-autenticação por IP e timeout.
pub async fn http_gate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let table = match route_class(request.method().as_str(), request.uri().path()) {
        None => return next.run(request).await, // /healthz, 404: sem política
        Some(t) => t,
    };
    let config = state.admission.config();

    // Classe inválida é recusada cedo, antes de qualquer leitura de corpo.
    let class = match effective_class(&request, table) {
        Ok(class) => class,
        Err(response) => return *response,
    };

    // 1) Payload: Content-Length primeiro — recusa barata antes de ler qualquer
    //    byte. Sem Content-Length (chunked) o corpo é lido com teto: o estouro
    //    estoura aqui, **antes** da desserialização do handler.
    let limit = table.body_limit(config);
    let mut request = request;
    if method_may_carry_body(request.method()) && limit > 0 {
        match request.headers().get(header::CONTENT_LENGTH) {
            Some(raw) => {
                if let Ok(len) = raw.to_str().unwrap_or_default().parse::<u64>()
                    && usize::try_from(len).is_ok_and(|len| len > limit)
                {
                    return payload_too_large(limit);
                }
                // Content-Length ilegível: hyper recusa antes de nós; seguimos.
            }
            None => {
                let (parts, body) = request.into_parts();
                match to_bytes(body, limit).await {
                    Ok(bytes) => request = Request::from_parts(parts, Body::from(bytes)),
                    Err(_) => return payload_too_large(limit),
                }
            }
        }
    }

    // 2) Cota por IP para pedidos **sem** credencial. Requer `ConnectInfo`
    //    (produção via `runtime::serve`); sem ele o teto do IP é do edge —
    //    declarado, não escondido.
    if !request.headers().contains_key(header::AUTHORIZATION)
        && let Some(ConnectInfo(peer)) =
            request.extensions().get::<ConnectInfo<std::net::SocketAddr>>()
    {
        let ip = peer.ip().to_string();
        if let Err(retry) = state.admission.check_anonymous(&ip, class) {
            return rate_limited(retry);
        }
    }

    // 3) Timeout por rota. O futuro do handler é cancelado no estouro; a
    //    resposta não promete ausência de efeito — só 504 com corpo estável.
    match tokio::time::timeout(table.timeout(config), next.run(request)).await {
        Ok(response) => response,
        Err(_) => request_timeout(),
    }
}

/// Portão interno (pós-autenticação): cota por grant, cota de eventos e
/// admissão por classe. Só existe para rotas sob `require_grant`.
pub async fn grant_gate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let table = match route_class(request.method().as_str(), request.uri().path()) {
        None => return next.run(request).await,
        Some(t) => t,
    };
    // `require_grant` roda antes deste portão; sem grant verificado nada aqui.
    let Some(grant) = request.extensions().get::<VerifiedGrant>().cloned() else {
        return next.run(request).await;
    };
    let Ok(class) = effective_class(&request, table) else {
        // Já validado no portão externo; défensivo.
        return next.run(request).await;
    };

    // Admissão por classe: sob pressão caem `event`/`sync`/`metrics`; SOS, PTT,
    // incidente e admin nunca são recusados por pressão — invariante §4-2.
    if class.sheds_under_pressure() && state.admission.under_pressure() {
        return admission_shed(1);
    }

    // Cota de requisições do grant, **por classe**: o bucket separado é o que
    // garante que flood de `event` não consome a cota de `sos`.
    if let Err(retry) =
        state.admission.check_request(&grant.subject.tenant_id, &grant.token_id, class)
    {
        return rate_limited(retry);
    }

    // Cota de eventos por escopo. O custo é declarado pelo pedido:
    // `POST /v1/events` = 1; lote de inbox = nº de posições, lido do corpo já
    // enquadrado no teto, antes da desserialização do handler.
    let (request, event_cost) = match charge_events(&state, request, table).await {
        Ok(pair) => pair,
        Err(response) => return *response,
    };
    if event_cost > 0
        && let Err(retry) =
            state.admission.check_events(&grant.subject.tenant_id, &grant.token_id, class, event_cost)
    {
        return rate_limited(retry);
    }

    // Só então o pedido conta como "em voo" para a pressão de admissão.
    let _flight = state.admission.enter();
    next.run(request).await
}

/// Lê o corpo apenas para contar eventos (inbox), devolvendo o mesmo corpo ao
/// handler. `POST /v1/events` custa 1 sem tocar no corpo; o resto custa 0.
async fn charge_events(
    state: &AppState,
    request: Request,
    table: RoutePolicy,
) -> Result<(Request, u64), Box<Response>> {
    if request.method() != Method::POST || table.body_kind != crate::admission::BodyKind::Inbox {
        // Toda escrita de evento que não é inbox custa 1 por evento declarado;
        // `/v1/events` é exatamente uma posição por pedido.
        let cost = if request.method() == Method::POST
            && request.uri().path() == "/v1/events"
        {
            1
        } else {
            0
        };
        return Ok((request, cost));
    }
    let limit = table.body_limit(state.admission.config()).max(1);
    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, limit).await {
        Ok(bytes) => bytes,
        // O portão externo já mediu; aqui é cinto-e-bretelle para corpo que
        // apareceu maior do que o permitido no caminho sem Content-Length.
        Err(_) => return Err(Box::new(payload_too_large(limit))),
    };
    let cost = inbox_event_cost(&bytes);
    Ok((Request::from_parts(parts, Body::from(bytes)), cost))
}
