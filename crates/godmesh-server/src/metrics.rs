//! Métricas mínimas no formato de texto do Prometheus, sem dependência nova.
//!
//! O que existe aqui é deliberadamente pouco e honesto:
//!  * **contador** por `(rota, status)` — rotas são os padrões registrados no
//!    router (`MatchedPath`), nunca o caminho cru, para não explodir a
//!    cardinalidade (um id por série seria um `map` sem teto);
//!  * **histograma** de latência por rota, com faixas fixas e cumulativas;
//!  * **gauges** do pool de conexões e do outbox pendente.
//!
//! Não há SLO, alerta, janela ou alvo: esses parâmetros não estão aprovados
//! (`docs/05-operacao/observabilidade.md`). O que esta fatia publica é a
//! **fonte** dos sinais `C13-SIG-002` (API), `C13-SIG-007` (pool) e
//! `C13-SIG-008` (backlog de outbox), sem afirmar saúde a partir deles.
//!
//! Nenhum label carrega tenant, principal, `token_id`, rota crua com id ou
//! qualquer identificador estável: métrica agregada não é canal de dado pessoal.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

/// Faixas (em segundos) do histograma de latência, cumulativas, como no
/// Prometheus clássico. `+Inf` é implícito e sempre igual ao `_count`.
pub const LATENCY_BUCKETS: [f64; 10] = [
    0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000, 2.500, 5.000,
];

/// Rótulo de rota usado quando a requisição não casou com nenhuma rota.
pub const UNMATCHED_ROUTE: &str = "unmatched";

/// Histograma acumulado de uma rota.
#[derive(Debug, Default)]
struct Histogram {
    /// Contagem por faixa, **acumulada** (`le`), na ordem de [`LATENCY_BUCKETS`].
    buckets: [u64; LATENCY_BUCKETS.len()],
    sum_seconds: f64,
    count: u64,
}

impl Histogram {
    fn observe(&mut self, seconds: f64) {
        for (slot, bound) in self.buckets.iter_mut().zip(LATENCY_BUCKETS) {
            if seconds <= bound {
                *slot += 1;
            }
        }
        self.sum_seconds += seconds;
        self.count += 1;
    }
}

#[derive(Debug, Default)]
struct Inner {
    requests: BTreeMap<(String, u16), u64>,
    durations: BTreeMap<String, Histogram>,
}

/// Registro de métricas, compartilhado por middleware e handler de `/metrics`.
///
/// `Mutex` (e não átomos por série) porque o conjunto de séries é pequeno e
/// dinâmico; a seção crítica é uma atualização de contador.
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    inner: Arc<Mutex<Inner>>,
}

impl Metrics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra uma requisição concluída. `route` é o padrão da rota, não o
    /// caminho com id.
    pub fn observe(&self, route: &str, status: u16, seconds: f64) {
        // Envenenamento do lock (pânico de outro handler) não pode derrubar a
        // observação: as métricas são diagnóstico, não regra de negócio.
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        *guard
            .requests
            .entry((route.to_owned(), status))
            .or_insert(0) += 1;
        guard
            .durations
            .entry(route.to_owned())
            .or_default()
            .observe(seconds);
    }

    /// Quantas requisições já foram observadas com `(rota, status)`. Usado
    /// somente por teste.
    #[must_use]
    pub fn requests(&self, route: &str, status: u16) -> u64 {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.requests.get(&(route.to_owned(), status)).copied())
            .unwrap_or(0)
    }
}

/// Valores lidos no instante do scrape. `None` significa **desconhecido** e a
/// série correspondente não é emitida: ausência de leitura não é zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gauges {
    pub pool_size: u32,
    pub pool_idle: usize,
    pub outbox_pending: Option<i64>,
}

impl Metrics {
    /// Renderiza o exposition format `text/plain; version=0.0.4`.
    #[must_use]
    pub fn render(&self, gauges: &Gauges) -> String {
        let Ok(guard) = self.inner.lock() else {
            // Sem lock não há números; não inventar zero é a regra.
            return String::from("# métricas indisponíveis: lock envenenado\n");
        };

        let mut out = String::with_capacity(2048);

        out.push_str(
            "# HELP godmesh_http_requests_total Requisições HTTP atendidas por rota e status.\n",
        );
        out.push_str("# TYPE godmesh_http_requests_total counter\n");
        for ((route, status), count) in &guard.requests {
            let _ = writeln!(
                out,
                "godmesh_http_requests_total{{route=\"{}\",status=\"{status}\"}} {count}",
                escape_label(route)
            );
        }

        out.push_str(
            "# HELP godmesh_http_request_duration_seconds Duração das requisições HTTP por rota (faixas cumulativas).\n",
        );
        out.push_str("# TYPE godmesh_http_request_duration_seconds histogram\n");
        for (route, histogram) in &guard.durations {
            let route = escape_label(route);
            for (bound, count) in LATENCY_BUCKETS.iter().zip(histogram.buckets) {
                let _ = writeln!(
                    out,
                    "godmesh_http_request_duration_seconds_bucket{{route=\"{route}\",le=\"{bound:.3}\"}} {count}"
                );
            }
            let _ = writeln!(
                out,
                "godmesh_http_request_duration_seconds_bucket{{route=\"{route}\",le=\"+Inf\"}} {}",
                histogram.count
            );
            let _ = writeln!(
                out,
                "godmesh_http_request_duration_seconds_sum{{route=\"{route}\"}} {:.6}",
                histogram.sum_seconds
            );
            let _ = writeln!(
                out,
                "godmesh_http_request_duration_seconds_count{{route=\"{route}\"}} {}",
                histogram.count
            );
        }

        out.push_str("# HELP godmesh_pool_connections Conexões do pool do Postgres por estado.\n");
        out.push_str("# TYPE godmesh_pool_connections gauge\n");
        let _ = writeln!(
            out,
            "godmesh_pool_connections{{state=\"size\"}} {}",
            gauges.pool_size
        );
        let _ = writeln!(
            out,
            "godmesh_pool_connections{{state=\"idle\"}} {}",
            gauges.pool_idle
        );

        out.push_str(
            "# HELP godmesh_outbox_pending Entradas do outbox ainda não confirmadas (ADR-061).\n",
        );
        out.push_str("# TYPE godmesh_outbox_pending gauge\n");
        if let Some(pending) = gauges.outbox_pending {
            let _ = writeln!(out, "godmesh_outbox_pending {pending}");
        }

        out.push_str(
            "# HELP godmesh_metrics_scrape_dependency_up 1 quando a leitura de dependência para as métricas funcionou.\n",
        );
        out.push_str("# TYPE godmesh_metrics_scrape_dependency_up gauge\n");
        let _ = writeln!(
            out,
            "godmesh_metrics_scrape_dependency_up{{dependency=\"postgres\"}} {}",
            if gauges.outbox_pending.is_some() {
                1
            } else {
                0
            }
        );

        out
    }
}

/// Escapa um valor de label conforme o exposition format: `\`, `"` e quebra de
/// linha. Rotas são padrões conhecidos, mas o escape não pode depender disso.
fn escape_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Gauges, Metrics, UNMATCHED_ROUTE};

    fn gauges() -> Gauges {
        Gauges {
            pool_size: 8,
            pool_idle: 3,
            outbox_pending: Some(2),
        }
    }

    #[test]
    fn contador_e_histograma_aparecem_no_texto() {
        let metrics = Metrics::new();
        metrics.observe("/v1/incidents", 200, 0.004);
        metrics.observe("/v1/incidents", 200, 0.030);
        metrics.observe(UNMATCHED_ROUTE, 404, 0.001);

        let text = metrics.render(&gauges());
        assert!(
            text.contains("godmesh_http_requests_total{route=\"/v1/incidents\",status=\"200\"} 2"),
            "{text}"
        );
        assert!(
            text.contains("godmesh_http_request_duration_seconds_count{route=\"/v1/incidents\"} 2"),
            "{text}"
        );
        // Faixas cumulativas: 0.004 cabe em 0.005 e em todas as maiores; 0.030
        // não cabe em 0.025 e cabe em 0.050.
        assert!(
            text.contains("godmesh_http_request_duration_seconds_bucket{route=\"/v1/incidents\",le=\"0.005\"} 1"),
            "{text}"
        );
        assert!(
            text.contains("godmesh_http_request_duration_seconds_bucket{route=\"/v1/incidents\",le=\"0.025\"} 1"),
            "{text}"
        );
        assert!(
            text.contains("godmesh_http_request_duration_seconds_bucket{route=\"/v1/incidents\",le=\"0.050\"} 2"),
            "{text}"
        );
        assert!(
            text.contains("godmesh_http_request_duration_seconds_bucket{route=\"/v1/incidents\",le=\"+Inf\"} 2"),
            "{text}"
        );
    }

    #[test]
    fn gauges_de_pool_e_outbox_sao_publicados() {
        let text = Metrics::new().render(&gauges());
        assert!(
            text.contains("godmesh_pool_connections{state=\"size\"} 8"),
            "{text}"
        );
        assert!(
            text.contains("godmesh_pool_connections{state=\"idle\"} 3"),
            "{text}"
        );
        assert!(text.contains("godmesh_outbox_pending 2"), "{text}");
        assert!(
            text.contains("godmesh_metrics_scrape_dependency_up{dependency=\"postgres\"} 1"),
            "{text}"
        );
    }

    #[test]
    fn outbox_desconhecido_nao_vira_zero() {
        let text = Metrics::new().render(&Gauges {
            pool_size: 8,
            pool_idle: 3,
            outbox_pending: None,
        });
        assert!(
            !text
                .lines()
                .any(|l| l.starts_with("godmesh_outbox_pending ")),
            "série ausente, não zero: {text}"
        );
        assert!(
            text.contains("godmesh_metrics_scrape_dependency_up{dependency=\"postgres\"} 0"),
            "{text}"
        );
    }

    #[test]
    fn label_com_aspas_e_escapado() {
        let metrics = Metrics::new();
        metrics.observe("rota\"x", 500, 0.001);
        let text = metrics.render(&gauges());
        assert!(text.contains("route=\"rota\\\"x\""), "{text}");
    }
}
