//! Ciclo de vida do processo: escuta, sinal, drenagem e fechamento do pool.
//!
//! O `axum::serve` com `with_graceful_shutdown` para de aceitar conexões e espera
//! as requisições em voo terminarem. O que ele **não** informa é quantas eram:
//! por isso o medidor de requisições em voo aqui. A contagem é lida no instante
//! do sinal, que é exatamente o conjunto que a drenagem precisa terminar.
//!
//! Ordem: sinal → para de aceitar → requisições em voo terminam → `pool.close()`
//! (nenhuma conexão nova, conexões devolvidas fecham) → processo sai com 0.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use sqlx::PgPool;
use tokio::net::TcpListener;

/// Medidor de requisições em voo, compartilhado com o middleware.
#[derive(Clone, Debug, Default)]
pub struct InFlight {
    count: Arc<AtomicUsize>,
}

impl InFlight {
    /// Medidor zerado.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Quantas requisições estão em voo agora.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

/// Decrementa ao sair, inclusive se o handler entrar em pânico.
struct Counted(InFlight);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.count.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Middleware de contagem: envolve a requisição inteira, não só a extração.
async fn track(State(gauge): State<InFlight>, request: Request, next: Next) -> Response {
    gauge.count.fetch_add(1, Ordering::SeqCst);
    let _guard = Counted(gauge);
    next.run(request).await
}

/// Resultado de uma execução do servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServeReport {
    /// Requisições que estavam em voo quando o sinal chegou e foram drenadas.
    pub drained: usize,
}

/// Serve até `shutdown` resolver, drena e fecha o pool.
///
/// O `Router` recebido não precisa do middleware de contagem: ele é aplicado
/// aqui, para que nenhuma rota nova nasça fora da contagem.
///
/// # Errors
/// Erro de I/O do `axum::serve` (ex.: listener quebrado). O pool é fechado nos
/// dois caminhos.
pub async fn serve<F>(
    listener: TcpListener,
    app: Router,
    pool: PgPool,
    shutdown: F,
) -> Result<ServeReport, String>
where
    F: Future<Output = ()> + Send + 'static,
{
    let gauge = InFlight::new();
    let app = app.layer(middleware::from_fn_with_state(gauge.clone(), track));

    let drained = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&drained);
    let observed = gauge;
    let shutdown = async move {
        shutdown.await;
        // Requisições em voo neste instante são as que precisam terminar.
        counter.store(observed.count(), Ordering::SeqCst);
    };

    let served = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;
    let drained = drained.load(Ordering::SeqCst);
    pool.close().await;
    served.map_err(|e| format!("servidor encerrou com erro: {e}"))?;
    Ok(ServeReport { drained })
}

/// Resolve em Ctrl-C ou SIGTERM; sem isso o processo só morre por SIGKILL.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("godmesh-server: falha ao escutar Ctrl-C: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                eprintln!("godmesh-server: falha ao escutar SIGTERM: {e}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use super::InFlight;
    use std::sync::atomic::Ordering;

    #[test]
    fn medidor_comeca_zerado() {
        assert_eq!(InFlight::new().count(), 0);
    }

    #[test]
    fn medidor_conta_incremento_e_decremento() {
        let gauge = InFlight::new();
        gauge.count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(gauge.count(), 1);
        gauge.count.fetch_sub(1, Ordering::SeqCst);
        assert_eq!(gauge.count(), 0);
    }
}
