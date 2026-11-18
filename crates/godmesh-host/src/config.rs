//! Configuração da coleta, do ambiente, sem default silencioso indevido.
//!
//! Só dois parâmetros existem nesta fatia, porque só duas decisões foram
//! tomadas: com que frequência observar, e de onde ler. Limiar de pressão,
//! histerese e ação **não** estão aqui — são as camadas 2 e 3 de ADR-073 e
//! ainda não têm parâmetro aprovado.

use std::path::PathBuf;
use std::time::Duration;

/// Intervalo padrão de coleta: 5 s, pedido na tarefa e coerente com o scrape
/// típico de 15 s (três amostras por scrape).
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// Snapshots mais velhos que `STALE_FACTOR × intervalo` são servidos como
/// indisponíveis. Um número velho sem aviso seria pior que nenhum número.
pub const STALE_FACTOR: u32 = 3;

/// Nome exato das variáveis de ambiente.
pub const ENV_INTERVAL_MS: &str = "GODMESH_HOST_METRICS_INTERVAL_MS";
pub const ENV_PROC_ROOT: &str = "GODMESH_HOST_PROC_ROOT";

/// Parâmetros da coleta de host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Intervalo entre coletas em background.
    pub interval: Duration,
    /// Raiz dos caminhos de `/proc`/`/sys`. `/` em produção.
    ///
    /// Configurável porque a evidência precisa rodar a árvore real do nó
    /// dentro de um diretório copiado (`--proc-fs`), e o teste precisa de
    /// fixture. Não é um convite a ler `/proc` de outro usuário: é o mesmo
    /// usuário, os mesmos privilégios, um caminho diferente.
    pub proc_root: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: DEFAULT_INTERVAL,
            proc_root: PathBuf::from("/"),
        }
    }
}

impl Config {
    /// Lê o ambiente.
    ///
    /// # Errors
    /// Ver [`Self::resolve`].
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::resolve(
            std::env::var(ENV_INTERVAL_MS).ok().as_deref(),
            std::env::var(ENV_PROC_ROOT).ok().as_deref(),
        )
    }

    /// Versão pura de [`Self::from_env`], testável sem mexer no ambiente global.
    ///
    /// # Errors
    /// [`ConfigError::InvalidInterval`] para valor não inteiro ou `0`; um
    /// intervalo `0` significaria "ler `/proc` num laço apertado", que é
    /// justamente o que a tarefa proíbe.
    /// [`ConfigError::InvalidRoot`] para caminho relativo ou vazio.
    pub fn resolve(interval_ms: Option<&str>, proc_root: Option<&str>) -> Result<Self, ConfigError> {
        let interval = match interval_ms.map(str::trim).filter(|value| !value.is_empty()) {
            None => DEFAULT_INTERVAL,
            Some(value) => {
                let ms: u64 = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidInterval(value.to_owned()))?;
                if ms == 0 {
                    return Err(ConfigError::InvalidInterval(value.to_owned()));
                }
                Duration::from_millis(ms)
            }
        };

        let proc_root = match proc_root.map(str::trim).filter(|value| !value.is_empty()) {
            None => PathBuf::from("/"),
            Some(value) => {
                let path = PathBuf::from(value);
                if !path.is_absolute() {
                    return Err(ConfigError::InvalidRoot(value.to_owned()));
                }
                path
            }
        };

        Ok(Self {
            interval,
            proc_root,
        })
    }

    /// Idade máxima tolerada de um snapshot servido.
    #[must_use]
    pub fn stale_after(&self) -> Duration {
        self.interval.mul_f32(STALE_FACTOR as f32)
    }
}

/// Erro de configuração de host telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// `GODMESH_HOST_METRICS_INTERVAL_MS` não é inteiro > 0.
    InvalidInterval(String),
    /// `GODMESH_HOST_PROC_ROOT` não é um caminho absoluto.
    InvalidRoot(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInterval(value) => write!(
                out,
                "{ENV_INTERVAL_MS}={value} não é um número de milissegundos > 0 \
                 (0 significaria ler /proc em laço apertado, o que é proibido)"
            ),
            Self::InvalidRoot(value) => write!(
                out,
                "{ENV_PROC_ROOT}={value} precisa ser um caminho absoluto \
                 (ex.: {ENV_PROC_ROOT}=/ ou {ENV_PROC_ROOT}=/var/lib/godmesh/proc-fs)"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError, DEFAULT_INTERVAL, ENV_INTERVAL_MS, ENV_PROC_ROOT};
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn sem_environment_e_cinco_segundos_na_raiz() {
        let config = Config::resolve(None, None).expect("default");
        assert_eq!(config.interval, DEFAULT_INTERVAL);
        assert_eq!(config.interval, Duration::from_secs(5));
        assert_eq!(config.proc_root, PathBuf::from("/"));
    }

    #[test]
    fn intervalo_e_raiz_vem_da_configuracao() {
        let config = Config::resolve(Some("1500"), Some("/var/lib/godmesh/proc-fs")).expect("valores");
        assert_eq!(config.interval, Duration::from_millis(1_500));
        assert_eq!(config.proc_root, PathBuf::from("/var/lib/godmesh/proc-fs"));
    }

    #[test]
    fn zero_nao_e_intervalmo_valido() {
        for value in ["0", "abc", "-5", "1.5", "5s"] {
            let error = Config::resolve(Some(value), None)
                .err()
                .unwrap_or_else(|| panic!("`{value}` deveria ser recusado"));
            assert_eq!(error, ConfigError::InvalidInterval(value.trim().to_owned()));
        }
    }

    #[test]
    fn raiz_relativa_e_recusada() {
        let error = Config::resolve(None, Some("proc-fs")).expect_err("relativa");
        assert_eq!(error, ConfigError::InvalidRoot("proc-fs".to_owned()));
    }

    #[test]
    fn erro_cita_a_variavel_exata() {
        let error = Config::resolve(Some("0"), None).expect_err("inválida");
        assert!(error.to_string().contains(ENV_INTERVAL_MS), "{error}");
        let error = Config::resolve(None, Some("relativa")).expect_err("inválida");
        assert!(error.to_string().contains(ENV_PROC_ROOT), "{error}");
    }

    #[test]
    fn janela_de_estale_e_multiplo_do_intervalo() {
        let config = Config::resolve(Some("5000"), None).expect("config");
        assert_eq!(config.stale_after(), Duration::from_secs(15));
    }
}
