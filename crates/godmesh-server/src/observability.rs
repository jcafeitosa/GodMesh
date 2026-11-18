//! Observação de cada requisição: uma linha de log estruturada e as métricas.
//!
//! A linha é emitida **uma vez por requisição**, depois do handler, e carrega
//! exatamente: `request_id`, rota (padrão registrado, não o caminho com id),
//! método, status, duração, `token_id` e tenant. Nunca o token, nunca o corpo,
//! nunca o caminho cru — um caminho com identificador de incidente é dado de
//! tenant e não pertence a log operacional.
//!
//! `token_id` e tenant só existem **depois** de `require_grant` autenticar; por
//! isso o middleware externo instala um [`RequestContext`] compartilhado na
//! extensão do pedido e o portão de autenticação o preenche. Sem credencial a
//! linha sai com `token_id=-` e `tenant=-`: ausência de identidade é registrada
//! como ausência, não inventada.
//!
//! Formato (uma linha, `chave=valor` separados por espaço):
//!
//! ```text
//! event=http_request request_id=req-1a2b-... method=GET route=/v1/incidents status=200 duration_ms=3 token_id=inc-read tenant=tenant-br
//! ```
//!
//! Valores são sanitizados para `[A-Za-z0-9._:@-]`: nada de espaço, quebra de
//! linha ou controle vindo de configuração pode forjar uma linha nova.

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::AppState;
use crate::metrics::UNMATCHED_ROUTE;

/// Destino das linhas de log de requisição.
pub trait RequestLog: Send + Sync + std::fmt::Debug {
    /// Registra uma linha já formatada. Implementações não podem falhar de forma
    /// que afete a resposta.
    fn record(&self, line: &str);
}

/// Log de produção: uma linha em stderr.
#[derive(Debug, Clone, Copy, Default)]
pub struct StderrRequestLog;

impl RequestLog for StderrRequestLog {
    fn record(&self, line: &str) {
        eprintln!("{line}");
    }
}

/// Log em memória, para provar em teste o que foi registrado e o que não foi.
#[derive(Debug, Default)]
pub struct CapturedLog {
    lines: std::sync::Mutex<Vec<String>>,
}

impl CapturedLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Linhas registradas até agora.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.lines.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl RequestLog for CapturedLog {
    fn record(&self, line: &str) {
        if let Ok(mut guard) = self.lines.lock() {
            guard.push(line.to_owned());
        }
    }
}

/// Identidade resolvida da requisição, preenchida pelo portão de autenticação.
///
/// Fica em `Arc` na extensão do pedido: o middleware externo guarda o mesmo
/// `Arc` e lê o que o middleware interno escreveu depois de `next.run`.
#[derive(Debug, Default)]
pub struct RequestContext {
    identity: OnceLock<Identity>,
}

/// o que o log pode afirmar sobre quem chamou.
#[derive(Debug, Clone)]
struct Identity {
    token_id: String,
    tenant_id: String,
}

impl RequestContext {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra a identidade verificada. Chamadas seguintes não sobrescrevem: a
    /// primeira identidade é a que autenticou o pedido.
    pub fn set_identity(&self, token_id: &str, tenant_id: &str) {
        let _ = self.identity.set(Identity {
            token_id: token_id.to_owned(),
            tenant_id: tenant_id.to_owned(),
        });
    }

    fn token_id(&self) -> &str {
        self.identity.get().map_or("-", |i| i.token_id.as_str())
    }

    fn tenant_id(&self) -> &str {
        self.identity.get().map_or("-", |i| i.tenant_id.as_str())
    }
}

/// Sequência de requisições do processo, só para tornar o `request_id` único.
static REQUEST_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `request_id` de correlação efêmera: processo + instante + sequência.
///
/// Não é identificador durável nem chave de busca histórica — é correlação de
/// uma requisição e morre com ela.
fn request_id() -> String {
    let seq = REQUEST_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("req-{:x}-{nanos:x}-{seq:x}", std::process::id())
}

/// Middleware único: conta, mede e registra a requisição, inclusive 401/403/404
/// e rotas não casadas (que são justamente as que mais importam em diagnóstico).
pub async fn observe(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let started = Instant::now();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or(UNMATCHED_ROUTE, MatchedPath::as_str)
        .to_owned();
    let method = request.method().clone();
    let context = Arc::new(RequestContext::new());
    let id = request_id();
    request.extensions_mut().insert(Arc::clone(&context));

    let response = next.run(request).await;

    let status = response.status().as_u16();
    state
        .metrics
        .observe(&route, status, started.elapsed().as_secs_f64());
    state.log.record(&line(
        &id,
        &method,
        &route,
        status,
        started.elapsed(),
        &context,
    ));
    response
}

/// Monta a linha de log, sanitizando todo valor que não é número.
fn line(
    request_id: &str,
    method: &axum::http::Method,
    route: &str,
    status: u16,
    duration: std::time::Duration,
    context: &RequestContext,
) -> String {
    format!(
        "event=http_request request_id={} method={} route={} status={status} duration_ms={} token_id={} tenant={}",
        sanitize(request_id),
        sanitize(method.as_str()),
        sanitize(route),
        duration.as_millis(),
        sanitize(context.token_id()),
        sanitize(context.tenant_id()),
    )
}

/// Reduz um valor ao alfabeto seguro de log (`[A-Za-z0-9._:@-]`). Qualquer outro
/// caractere vira `_`: nenhum valor configurado forja linha ou campo novo.
fn sanitize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '@' | '-' | '/' | '{' | '}')
        {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('-');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{RequestContext, sanitize};

    #[test]
    fn log_sem_identidade_registra_ausencia() {
        let context = RequestContext::new();
        assert_eq!(context.token_id(), "-");
        assert_eq!(context.tenant_id(), "-");
    }

    #[test]
    fn identidade_e_imutavel_apos_a_primeira_autenticacao() {
        let context = RequestContext::new();
        context.set_identity("primeiro", "tenant-a");
        context.set_identity("segundo", "tenant-b");
        assert_eq!(context.token_id(), "primeiro");
        assert_eq!(context.tenant_id(), "tenant-a");
    }

    #[test]
    fn sanitize_impede_forjar_campos_ou_linhas() {
        assert_eq!(sanitize("a b\nc=d"), "a_b_c_d");
        assert_eq!(sanitize(""), "-");
        assert_eq!(sanitize("/v1/incidents/{id}"), "/v1/incidents/{id}");
    }
}
