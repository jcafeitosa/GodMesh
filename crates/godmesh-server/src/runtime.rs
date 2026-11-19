//! Ciclo de vida do processo: escuta, sinal, drenagem e fechamento do pool.
//!
//! O `axum::serve` com `with_graceful_shutdown` para de aceitar conexões e espera
//! as requisições em voo terminarem. O que ele **não** informa é quantas eram:
//! por isso o medidor de requisições em voo aqui. A contagem é lida no instante
//! do sinal, que é exatamente o conjunto que a drenagem precisa terminar.
//!
//! O binário aguarda o HTTP e os serviços auxiliares antes de fechar o pool.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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
    let result = serve_with_shared_pool(listener, app, shutdown).await;
    pool.close().await;
    result
}

/// Drena o listener. O supervisor fecha o pool depois dos outros serviços.
pub async fn serve_with_shared_pool<F>(
    listener: TcpListener,
    app: Router,
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

    // `ConnectInfo` habilita a cota por IP do portão de abuso: sem o par,
    // requisições não autenticadas não têm chave por origem no processo.
    let served = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await;
    let drained = drained.load(Ordering::SeqCst);
    served.map_err(|e| format!("servidor encerrou com erro: {e}"))?;
    Ok(ServeReport { drained })
}

const POOL_CLOSE_GRACE: Duration = Duration::from_secs(2);

/// Cancela e aguarda os serviços auxiliares antes de fechar o pool.
///
/// Uma consulta cancelada no cliente pode continuar bloqueada no PostgreSQL.
/// Se a devolução da conexão ainda estiver esperando pelo banco, o prazo
/// permite concluir a saída do processo e derrubar os sockets restantes.
pub async fn finish_after_http(
    http: Result<ServeReport, String>,
    shutdown: &tokio::sync::watch::Sender<bool>,
    pool: &PgPool,
    cleanup: impl Future<Output = Result<(), String>>,
) -> Result<ServeReport, String> {
    let _ = shutdown.send(true);
    let auxiliary = cleanup.await;
    if tokio::time::timeout(POOL_CLOSE_GRACE, pool.close())
        .await
        .is_err()
    {
        eprintln!("godmesh-server: prazo esgotado ao aguardar o fechamento do pool PostgreSQL");
    }
    http.and_then(|report| auxiliary.map(|()| report))
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
    use std::time::Duration;

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

    #[tokio::test]
    async fn pool_compartilhado_fecha_apos_cancelar_tarefas_tambem_quando_http_falha() {
        let Ok(url) = std::env::var("GODMESH_TEST_DATABASE_URL") else {
            return;
        };
        for failing in [false, true] {
            let pool = sqlx::PgPool::connect(&url).await.unwrap();
            let mut connection = pool.acquire().await.unwrap();
            let (cancel, mut stopped) = tokio::sync::watch::channel(false);
            let observed_pool = pool.clone();
            let auxiliary = tokio::spawn(async move {
                stopped.changed().await.unwrap();
                assert!(!observed_pool.is_closed());
                let value: i32 = sqlx::query_scalar("SELECT 1")
                    .fetch_one(&mut *connection)
                    .await
                    .unwrap();
                assert_eq!(value, 1);
            });
            let result = if failing {
                Err("falha do listener de teste".to_owned())
            } else {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    super::serve_with_shared_pool(listener, axum::Router::new(), async {}),
                )
                .await
                .unwrap()
            };
            assert_eq!(result.is_err(), failing);
            let ended = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                super::finish_after_http(result, &cancel, &pool, async {
                    auxiliary.await.unwrap();
                    Ok(())
                }),
            )
            .await
            .unwrap();
            assert_eq!(ended.is_err(), failing);
            assert!(pool.is_closed());
        }
    }

    #[tokio::test]
    async fn shutdown_termina_com_consulta_cancelada_ainda_bloqueada_no_postgres() {
        let Ok(url) = std::env::var("GODMESH_TEST_DATABASE_URL") else {
            return;
        };
        let observer = sqlx::PgPool::connect(&url).await.unwrap();
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        let mut blocker = observer.begin().await.unwrap();
        let lock: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(113, $1)")
            .bind(lock)
            .execute(&mut *blocker)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        let (shutdown, mut stopped) = tokio::sync::watch::channel(false);
        let reader = tokio::spawn(async move {
            tokio::select! {
                _ = stopped.changed() => {}
                result = sqlx::query("SELECT pg_advisory_xact_lock(113, $1)")
                    .bind(lock)
                    .execute(&mut *tx) => panic!("consulta terminou antes do cancelamento: {result:?}"),
            }
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let blocked: bool = sqlx::query_scalar("SELECT $1 = ANY(pg_blocking_pids($2))")
                    .bind(lock)
                    .bind(pid)
                    .fetch_one(&observer)
                    .await
                    .unwrap();
                if blocked {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("consulta aguardando o lock externo");

        let result = tokio::time::timeout(
            Duration::from_secs(4),
            super::finish_after_http(
                Ok(super::ServeReport { drained: 0 }),
                &shutdown,
                &pool,
                async {
                    reader.await.unwrap();
                    Ok(())
                },
            ),
        )
        .await;
        blocker.rollback().await.unwrap();
        pool.close().await;
        assert!(
            result
                .expect("shutdown antes de liberar o lock externo")
                .is_ok()
        );
    }
}
