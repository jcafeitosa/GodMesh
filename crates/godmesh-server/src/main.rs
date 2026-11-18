//! Binário do servidor HTTP do GodMesh.
//!
//! Configuração vem do ambiente, sem default silencioso:
//! - `DATABASE_URL` (obrigatório, sempre)
//! - `GODMESH_GRANTS` (obrigatório): registro de grants verificados em JSON;
//!   sem ele nenhuma credencial resolve e todo `/v1/incidents` responderia 401
//! - `BIND_ADDR` (obrigatório, exceto em `GODMESH_ENV=dev|development`, onde o
//!   default é `127.0.0.1:8080`)
//! - `GODMESH_ENV` (opcional; ausente é tratado como `production`)
//!
//! Ausência de configuração é erro explícito com mensagem e código de saída != 0.
//! Um default de loopback em produção deixava o serviço inalcançável de fora do
//! contêiner sem nenhum erro; por isso o ambiente controla se o default existe.
//!
//! Nenhum exemplo de configuração neste arquivo traz senha: o binário é publicado.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use godmesh_server::auth::GrantRegistry;
use sqlx::postgres::PgPoolOptions;

/// Endereço padrão aceito **apenas** em ambiente de desenvolvimento: só loopback.
/// Expor a interface exige configuração deliberada.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";

/// Valores de `GODMESH_ENV` que autorizam o default de loopback. Qualquer outro
/// valor — inclusive ausente — é tratado como produção (falha fechada).
const DEV_ENV_VALUES: [&str; 2] = ["dev", "development"];

/// Configuração lida do ambiente.
struct Config {
    database_url: String,
    grants: GrantRegistry,
    bind_addr: String,
}

impl Config {
    /// Lê o ambiente e devolve erro legível quando algo obrigatório falta.
    ///
    /// # Errors
    /// `DATABASE_URL` ausente/vazia; `GODMESH_GRANTS` ausente, vazia ou inválida;
    /// `BIND_ADDR` ausente fora de `GODMESH_ENV=dev|development` ou inválido.
    /// Nenhuma mensagem inclui token.
    fn from_env() -> Result<Self, String> {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(v) if !v.trim().is_empty() => v,
            Ok(_) => {
                return Err(
                    "DATABASE_URL está definida mas vazia; informe a URL do Postgres".to_owned(),
                );
            }
            Err(std::env::VarError::NotPresent) => {
                return Err(
                    "DATABASE_URL não definida; informe a URL do Postgres em DATABASE_URL (exemplo de formato: postgres://usuario:senha@host:porta/banco)"
                        .to_owned(),
                );
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err("DATABASE_URL contém bytes não-UTF8".to_owned());
            }
        };

        let bind_addr = resolve_bind_addr(
            std::env::var("BIND_ADDR").ok().as_deref(),
            std::env::var("GODMESH_ENV").ok().as_deref(),
        )?;

        let grants = GrantRegistry::from_env()
            .map_err(|e| format!("registro de grants indisponível: {e}"))?;

        Ok(Self {
            database_url,
            grants,
            bind_addr,
        })
    }
}

/// Decide o endereço de escuta a partir do ambiente.
///
/// Fora de `dev`/`development`, `BIND_ADDR` é obrigatório: um default de loopback
/// em produção faz o contêiner subir "saudável" e inalcançável.
///
/// # Errors
/// Devolve mensagem citando `BIND_ADDR` quando ela falta e o ambiente não é dev.
fn resolve_bind_addr(bind_addr: Option<&str>, env: Option<&str>) -> Result<String, String> {
    if let Some(v) = bind_addr.map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(v.to_owned());
    }

    let env_normalizado = env.map(str::trim).map(str::to_ascii_lowercase);
    if env_normalizado
        .as_deref()
        .is_some_and(|e| DEV_ENV_VALUES.contains(&e))
    {
        return Ok(DEFAULT_BIND_ADDR.to_owned());
    }

    Err(format!(
        "BIND_ADDR não definida; com GODMESH_ENV={} o endereço de escuta é obrigatório \
         (exemplo: BIND_ADDR=0.0.0.0:8080); o default {DEFAULT_BIND_ADDR} só vale em GODMESH_ENV=dev",
        env.unwrap_or("<ausente> = production")
    ))
}

fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("godmesh-server: configuração inválida: {msg}");
            return ExitCode::from(78); // EX_CONFIG
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
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
        .map_err(|e| {
            format!(
                "não foi possível escutar em BIND_ADDR={}: {e}",
                config.bind_addr
            )
        })?;

    let local = listener
        .local_addr()
        .map_err(|e| format!("listener sem endereço local: {e}"))?;
    println!(
        "godmesh-server: escutando em http://{local} ({} grant(s) configurado(s))",
        config.grants.len()
    );

    axum::serve(listener, godmesh_server::http::router(pool, config.grants))
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

// Módulo de testes no fim do arquivo: `clippy::items-after-test-module`
// (negado por `-D clippy::all`) exige que nenhum item venha depois dele.
#[cfg(test)]
mod tests {
    use super::{DEFAULT_BIND_ADDR, resolve_bind_addr};

    #[test]
    fn bind_addr_ausente_em_producao_falha_citando_a_variavel() {
        let erro = resolve_bind_addr(None, Some("production")).expect_err("deve falhar");
        assert!(
            erro.contains("BIND_ADDR"),
            "mensagem precisa citar BIND_ADDR: {erro}"
        );
    }

    #[test]
    fn bind_addr_ausente_sem_godmesh_env_e_tratada_como_producao() {
        assert!(resolve_bind_addr(None, None).is_err());
    }

    #[test]
    fn bind_addr_vazia_em_producao_falha() {
        assert!(resolve_bind_addr(Some("   "), Some("production")).is_err());
    }

    #[test]
    fn bind_addr_desconhecida_falha_fechada() {
        assert!(resolve_bind_addr(None, Some("staging")).is_err());
    }

    #[test]
    fn bind_addr_default_loopback_so_em_dev() {
        assert_eq!(
            resolve_bind_addr(None, Some("dev")).unwrap(),
            DEFAULT_BIND_ADDR
        );
        assert_eq!(
            resolve_bind_addr(None, Some("development")).unwrap(),
            DEFAULT_BIND_ADDR
        );
        assert_eq!(
            resolve_bind_addr(None, Some("DEV")).unwrap(),
            DEFAULT_BIND_ADDR
        );
    }

    #[test]
    fn bind_addr_explicita_vale_em_qualquer_ambiente() {
        assert_eq!(
            resolve_bind_addr(Some("0.0.0.0:8080"), Some("production")).unwrap(),
            "0.0.0.0:8080"
        );
    }
}
