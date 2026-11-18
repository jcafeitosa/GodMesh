//! Loop de publicação do outbox durável (ADR-061), em tokio.
//!
//! O publicador é uma passagem periódica over grupos com pendência:
//! `publish_next` reivindica a próxima sequência contígua, o [`Sink`] executa o
//! efeito e só então `confirm` marca `confirmed_at`. Falha do efeito →
//! `record_failure` (incrementa `attempts`, mantém `confirmed_at` nulo).
//!
//! Configuração por ambiente, sem default silencioso de comportamento:
//!
//! - `GODMESH_OUTBOX`: `on` (default) liga; `off` desliga o publicador. Qualquer
//!   outro valor é erro de configuração — um typo não deve virar "ligado".
//! - `GODMESH_OUTBOX_INTERVAL_MS`: intervalo entre passagens em milissegundos,
//!   **default 1000 ms**; precisa ser inteiro > 0.
//!
//! O efeito desta fase é o sink de teste `outbox_sink`
//! ([`godmesh_sync::pg_outbox::PgSink`]). Não há broker, bridge, rádio,
//! pagamento ou notificação aqui.

use core::fmt;
use core::future::Future;
use core::time::Duration;

use godmesh_sync::pg_outbox::{OutboxError, PartitionKey, PublishOutcome, PgOutbox, Sink};

/// Variável que liga/desliga o publicador.
pub const ENV_SWITCH: &str = "GODMESH_OUTBOX";
/// Variável do intervalo entre passagens.
pub const ENV_INTERVAL_MS: &str = "GODMESH_OUTBOX_INTERVAL_MS";
/// Intervalo padrão entre passagens, documentado aqui e no README do crate.
pub const DEFAULT_INTERVAL_MS: u64 = 1_000;

/// Configuração do publicador resolvida do ambiente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxConfig {
    pub enabled: bool,
    pub interval: Duration,
}

impl OutboxConfig {
    /// Lê `GODMESH_OUTBOX` e `GODMESH_OUTBOX_INTERVAL_MS`.
    ///
    /// # Errors
    /// Valor desconhecido em `GODMESH_OUTBOX`; intervalo não numérico, ou zero.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::resolve(
            std::env::var(ENV_SWITCH).ok().as_deref(),
            std::env::var(ENV_INTERVAL_MS).ok().as_deref(),
        )
    }

    /// Versão pura de [`Self::from_env`], testável sem mexer no ambiente global.
    ///
    /// # Errors
    /// Mesmas recusas de [`Self::from_env`].
    pub fn resolve(switch: Option<&str>, interval_ms: Option<&str>) -> Result<Self, ConfigError> {
        let enabled = match switch.map(str::trim).filter(|v| !v.is_empty()) {
            None => true,
            Some(v) if matches_any(v, &["on", "1", "true", "enabled"]) => true,
            Some(v) if matches_any(v, &["off", "0", "false", "disabled"]) => false,
            Some(v) => {
                return Err(ConfigError::UnknownSwitch(v.to_owned()));
            }
        };

        let interval = match interval_ms.map(str::trim).filter(|v| !v.is_empty()) {
            None => Duration::from_millis(DEFAULT_INTERVAL_MS),
            Some(v) => {
                let ms: u64 = v
                    .parse()
                    .map_err(|_| ConfigError::InvalidInterval(v.to_owned()))?;
                if ms == 0 {
                    return Err(ConfigError::InvalidInterval(v.to_owned()));
                }
                Duration::from_millis(ms)
            }
        };

        Ok(Self { enabled, interval })
    }
}

/// Comparação sem caixa: `On`/`OFF` valem como `on`/`off`.
fn matches_any(value: &str, accepted: &[&str]) -> bool {
    accepted
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

/// Erro de configuração do publicador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// `GODMESH_OUTBOX` com valor fora de on/off.
    UnknownSwitch(String),
    /// `GODMESH_OUTBOX_INTERVAL_MS` não é inteiro > 0.
    InvalidInterval(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSwitch(v) => write!(
                f,
                "{ENV_SWITCH}={v} não é reconhecido; use on ou off"
            ),
            Self::InvalidInterval(v) => write!(
                f,
                "{ENV_INTERVAL_MS}={v} inválido; use inteiro de milissegundos > 0"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Erro de uma passagem de publicação.
#[derive(Debug)]
pub enum PublisherError {
    Storage(sqlx::Error),
    Outbox(OutboxError),
}

impl fmt::Display for PublisherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(e) => write!(f, "falha de banco no publicador: {e}"),
            Self::Outbox(e) => write!(f, "outbox recusou a operação: {e}"),
        }
    }
}

impl std::error::Error for PublisherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            Self::Outbox(e) => Some(e),
        }
    }
}

impl From<sqlx::Error> for PublisherError {
    fn from(e: sqlx::Error) -> Self {
        Self::Storage(e)
    }
}

impl From<OutboxError> for PublisherError {
    fn from(e: OutboxError) -> Self {
        Self::Outbox(e)
    }
}

/// Contagem de uma passagem. Efêmera: não é prova de entrega, é o que aconteceu
/// nesta passagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// Entregas aceitas pelo sink.
    pub published: u64,
    /// Entradas confirmadas (`confirmed_at` gravado depois do efeito).
    pub confirmed: u64,
    /// Entregas recusadas pelo sink.
    pub failed: u64,
}

impl Stats {
    /// Total de entradas tocadas nesta passagem.
    #[must_use]
    pub const fn touched(&self) -> u64 {
        self.published + self.failed
    }
}

/// Publicador do outbox.
#[derive(Debug, Clone)]
pub struct Publisher<S> {
    outbox: PgOutbox,
    sink: S,
}

impl<S: Sink + Send + Sync> Publisher<S> {
    #[must_use]
    pub const fn new(outbox: PgOutbox, sink: S) -> Self {
        Self { outbox, sink }
    }

    /// Uma passagem: para cada grupo com pendência, publica e confirma em ordem
    /// até parar.
    ///
    /// Cada pendência é tentada no máximo uma vez por passagem — depois de uma
    /// falha a passagem segue para o próximo grupo, e a retentativa acontece na
    /// passagem seguinte. Sem esse corte, um efeito que falha sempre viraria um
    /// laço quente dentro de um único tick.
    ///
    /// # Errors
    /// Falha de banco na leitura de grupos, na reivindicação ou na confirmação.
    pub async fn run_once(&self) -> Result<Stats, PublisherError> {
        let mut stats = Stats::default();
        for key in self.outbox.active_partitions().await? {
            self.publish_partition(&key, &mut stats).await?;
        }
        Ok(stats)
    }

    /// Uma passagem limitada a **um** grupo.
    ///
    /// Existe para operação dirigida e para teste: `run_once` varre todos os
    /// grupos com pendência do banco, então um teste que só quer medir o próprio
    /// grupo precisa deste escopo.
    ///
    /// # Errors
    /// Mesmas recusas de [`Self::run_once`].
    pub async fn run_once_for(&self, key: &PartitionKey) -> Result<Stats, PublisherError> {
        let mut stats = Stats::default();
        self.publish_partition(key, &mut stats).await?;
        Ok(stats)
    }

    /// Percorre uma partição até não haver mais o que publicar ou até algo parar
    /// a fila (lacuna, disputa, irreversível pendente, falha do sink).
    async fn publish_partition(
        &self,
        key: &PartitionKey,
        stats: &mut Stats,
    ) -> Result<(), PublisherError> {
        loop {
            match self.outbox.publish_next(key).await? {
                PublishOutcome::Published(entry) => match self.sink.deliver(key, &entry).await {
                    Ok(()) => {
                        self.outbox.confirm(key, entry.seq).await?;
                        stats.published += 1;
                        stats.confirmed += 1;
                    }
                    Err(e) => {
                        let record = self.outbox.record_failure(key, entry.seq).await?;
                        stats.failed += 1;
                        eprintln!(
                            "godmesh-server: outbox {}/{}/{} partição {} seq {} falhou \
                             (tentativa {}): {e}{}",
                            key.origin_node(),
                            key.tenant_id(),
                            key.stream_id(),
                            key.partition(),
                            entry.seq,
                            record.attempts,
                            if record.retry_scheduled {
                                "; reenfileirada"
                            } else {
                                "; irreversível, não será reenviada"
                            }
                        );
                        return Ok(());
                    }
                },
                PublishOutcome::NothingToPublish => return Ok(()),
                PublishOutcome::ClaimedByAnother(entry) => {
                    // Outra instância tem a vez; nada a fazer nesta passagem.
                    eprintln!(
                        "godmesh-server: outbox {}/{}/{} partição {} seq {} em publicação por outra instância",
                        key.origin_node(),
                        key.tenant_id(),
                        key.stream_id(),
                        key.partition(),
                        entry.seq
                    );
                    return Ok(());
                }
                PublishOutcome::BlockedByGap { missing } => {
                    eprintln!(
                        "godmesh-server: outbox {}/{}/{} partição {} parado na lacuna seq {missing}; \
                         nada depois da lacuna é publicado",
                        key.origin_node(),
                        key.tenant_id(),
                        key.stream_id(),
                        key.partition()
                    );
                    return Ok(());
                }
                PublishOutcome::AwaitingReconciliation(entry) => {
                    eprintln!(
                        "godmesh-server: outbox {}/{}/{} partição {} seq {} é efeito irreversível \
                         já publicado e não confirmado (tentativas {}); não será reenviado e o \
                         prefixo para aqui",
                        key.origin_node(),
                        key.tenant_id(),
                        key.stream_id(),
                        key.partition(),
                        entry.seq,
                        entry.attempts
                    );
                    return Ok(());
                }
            }
        }
    }

    /// Roda passagens a cada `interval` até `shutdown` resolver.
    ///
    /// Devolve o total acumulado. Erros de uma passagem são relatados e a rotina
    /// continua: um banco momentaneamente indisponível não deve matar o laço.
    pub async fn run<F>(self, interval: Duration, shutdown: F) -> Stats
    where
        F: Future<Output = ()>,
    {
        let mut shutdown = std::pin::pin!(shutdown);
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut total = Stats::default();
        loop {
            tokio::select! {
                () = &mut shutdown => {
                    println!(
                        "godmesh-server: outbox encerrado (publicados {}, confirmados {}, falhas {})",
                        total.published, total.confirmed, total.failed
                    );
                    return total;
                }
                _ = ticker.tick() => {
                    match self.run_once().await {
                        Ok(stats) => {
                            if stats.touched() > 0 {
                                println!(
                                    "godmesh-server: outbox passagem: publicados {}, confirmados {}, falhas {}",
                                    stats.published, stats.confirmed, stats.failed
                                );
                            }
                            total.published += stats.published;
                            total.confirmed += stats.confirmed;
                            total.failed += stats.failed;
                        }
                        Err(e) => {
                            eprintln!("godmesh-server: outbox passagem falhou: {e}");
                        }
                    }
                }
            }
        }
    }
}

impl<S: Sink + Send + Sync> Publisher<S> {
    /// Como [`Publisher::run`], mas limitado a um grupo.
    ///
    /// É o mesmo laço de [`Publisher::run_once_for`]: existe para operação
    /// dirigida e para que um teste meça o próprio grupo sem publicar os grupos
    /// de outros testes do banco.
    pub async fn run_for<F>(self, key: &PartitionKey, interval: Duration, shutdown: F) -> Stats
    where
        F: Future<Output = ()>,
    {
        let mut shutdown = std::pin::pin!(shutdown);
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut total = Stats::default();
        loop {
            tokio::select! {
                () = &mut shutdown => {
                    println!(
                        "godmesh-server: outbox encerrado (publicados {}, confirmados {}, falhas {})",
                        total.published, total.confirmed, total.failed
                    );
                    return total;
                }
                _ = ticker.tick() => {
                    match self.run_once_for(key).await {
                        Ok(stats) => {
                            if stats.touched() > 0 {
                                println!(
                                    "godmesh-server: outbox passagem: publicados {}, confirmados {}, falhas {}",
                                    stats.published, stats.confirmed, stats.failed
                                );
                            }
                            total.published += stats.published;
                            total.confirmed += stats.confirmed;
                            total.failed += stats.failed;
                        }
                        Err(e) => {
                            eprintln!("godmesh-server: outbox passagem falhou: {e}");
                        }
                    }
                }
            }
        }
    }
}

// `items_after_test_module` (negado por `-D clippy::all`) exige que o módulo de
// testes seja o último item do arquivo.
#[cfg(test)]
mod tests {
    use super::{ConfigError, DEFAULT_INTERVAL_MS, OutboxConfig};
    use core::time::Duration;

    #[test]
    fn ausente_de_ambiente_liga_com_intervalo_padrao() {
        let cfg = OutboxConfig::resolve(None, None).expect("default válido");
        assert!(cfg.enabled);
        assert_eq!(cfg.interval, Duration::from_millis(DEFAULT_INTERVAL_MS));
        assert_eq!(DEFAULT_INTERVAL_MS, 1_000, "default documentado é 1000 ms");
    }

    #[test]
    fn off_desliga_e_variantes_aceitas() {
        for valor in ["off", "OFF", "0", "false", "disabled"] {
            let cfg = OutboxConfig::resolve(Some(valor), None).expect("valor aceito");
            assert!(!cfg.enabled, "{valor} precisa desligar o publicador");
        }
        for valor in ["on", "ON", "1", "true", "enabled"] {
            let cfg = OutboxConfig::resolve(Some(valor), None).expect("valor aceito");
            assert!(cfg.enabled, "{valor} precisa ligar o publicador");
        }
    }

    #[test]
    fn valor_desconhecido_falha_fechada() {
        assert_eq!(
            OutboxConfig::resolve(Some("talvez"), None),
            Err(ConfigError::UnknownSwitch("talvez".to_owned()))
        );
    }

    #[test]
    fn intervalo_zero_ou_invalido_e_recusado() {
        assert!(matches!(
            OutboxConfig::resolve(None, Some("0")),
            Err(ConfigError::InvalidInterval(_))
        ));
        assert!(matches!(
            OutboxConfig::resolve(None, Some("mil")),
            Err(ConfigError::InvalidInterval(_))
        ));
        assert!(matches!(
            OutboxConfig::resolve(None, Some("-5")),
            Err(ConfigError::InvalidInterval(_))
        ));
    }

    #[test]
    fn intervalo_explicito_vale() {
        let cfg = OutboxConfig::resolve(None, Some("250")).expect("intervalo válido");
        assert_eq!(cfg.interval, Duration::from_millis(250));
    }
}