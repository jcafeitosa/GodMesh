//! Binário do servidor HTTP do GodMesh.
//!
//! Configuração vem do ambiente, sem default silencioso para o banco:
//! - `DATABASE_URL` (obrigatório)
//! - `BIND_ADDR` (opcional, default `127.0.0.1:8080`)
//!
//! Ausência de configuração é erro explícito com mensagem e código de saída != 0.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use sqlx::postgres::PgPoolOptions;

/// Endereço padrão: só loopback. Expor a interface exige configuração deliberada.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";

/// Configuração lida do ambiente.
struct Config {
    database_url: String,
    bind_addr: String,
}

impl Config {
    /// Lê o ambiente e devolve erro legível quando algo obrigatório falta.
    ///
    /// # Errors
    /// `DATABASE_URL` ausente ou vazio; `BIND_ADDR` presente mas inválido.
    fn from_env() -> Result<Self, String> {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(v) if !v.trim().is_empty() => v,
            Ok(_) => {
                return Err("DATABASE_URL está definida mas vazia; informe a URL do Postgres".to_owned());
            }
            Err(std::env::VarError::NotPresent) => {
                return Err(
                    "DATABASE_URL não definida; exemplo: DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh"
                        .to_owned(),
                );
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err("DATABASE_URL contém bytes não-UTF8".to_owned());
            }
        };

        let bind_addr = match std::env::var("BIND_ADDR") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => DEFAULT_BIND_ADDR.to_owned(),
        };

        Ok(Self {
            database_url,
            bind_addr,
        })
    }
}

fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("godmesh-server: configuração inválida: {msg}");
            return ExitCode::from(78); // EX_CONFIG
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("godmesh-server: falha ao criar runtime tokio: {e}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("godmesh-server: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// Sobe o pool, liga o listener e serve até receber sinal de término.
async fn run(config: Config) -> Result<(), String> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await
        .map_err(|e| format!("não foi possível conectar ao Postgres em DATABASE_URL: {e}"))?;

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .map_err(|e| format!("não foi possível escutar em BIND_ADDR={}: {e}", config.bind_addr))?;

    let local = listener
        .local_addr()
        .map_err(|e| format!("listener sem endereço local: {e}"))?;
    println!("godmesh-server: escutando em http://{local}");

    axum::serve(listener, godmesh_server::http::router(pool))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("servidor encerrou com erro: {e}"))?;

    println!("godmesh-server: encerrado");
    Ok(())
}

/// Resolve em Ctrl-C ou SIGTERM; sem isso o processo só morre por SIGKILL.
async fn shutdown_signal() {
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