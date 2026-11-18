//! # godmesh-host — telemetria de recursos do nó (ADR-073, camada 1)
//!
//! O servidor passa a **conhecer** o host. Esta crate é só a camada
//! "observar" de ADR-073: leitura `read-only` de CPU, memória, pressão,
//! disco, descritores, conexões, conntrack e interfaces. Não há firewall,
//! rota, `ulimit`, `kill` ou qualquer outra ação aqui — e não vai haver sem
//! a camada 3 aprovada, com escopo `host:admin`, `dry-run` obrigatório e
//! auditoria de antes/depois.
//!
//! ## Onde está a fronteira
//!
//!  * [`proc`] — parsers puros de texto. Nada toca disco aqui, então o
//!    formato é coberto por fixture e não pelo host do CI.
//!  * [`read`] — raiz configurável (`/` em produção, fixture no teste). O
//!    **mesmo** caminho de código serve os dois, o que é o que torna a
//!    evidência honesta.
//!  * [`collect`] — monta um [`snapshot::Snapshot`], decidindo o que é
//!    "lido" e o que é "ausente".
//!  * [`expose`] — texto Prometheus com prefixo `godmesh_host_`.
//!  * [`volumes`] — `statfs`, a única coisa que `/proc` não sabe.
//!
//! ## As três regras que sobrevivem a qualquer refactor
//!
//! 1. **Métrica ausente não é zero.** Grupo não lido não tem série; a
//!    afirmação da falha é `godmesh_host_metrics_available` /
//!    `godmesh_host_group_available`. Um `0` de leitura proibida leria
//!    "conntrack vazio" para um operador que tem 200 mil entradas.
//! 2. **Nada de `/proc` no caminho da requisição.** A coleta roda em
//!    background no intervalo configurado e o handler serve o último
//!    snapshot ([`collect::SharedSnapshot`]). O custo é declarado em
//!    `godmesh_host_collection_duration_seconds` e medido por teste de
//!    fumaça ([`budget`]).
//! 3. **Em contêiner o número é do namespace.** CPU/memória de cgroup são o
//!    limite do contêiner, não o hipervisor físico; `source="cgroup"` e a
//!    `HELP` de cada família dizem isso. `/proc/pressure` é a visão do
//!    kernel deste namespace — por isso as duas coexistem no painel, com
//!    rótulo `scope`.
//!
//! ## O que esta crate **não** prova
//!
//! Não prova limiar de pressão nem readiness (camada 2, parâmetros não
//! aprovados), não prova custo em carga de produção, não cobre inodes
//! (`sysinfo` 0.40.4 não expõe `f_files`/`f_ffree`), e não usa netlink —
//! conntrack é lido por arquivo, que é a interface estável.
//!
//! ## Dependência
//!
//! `sysinfo` 0.40.4, feature `disk` apenas (ADR-073; crates.io consultado em
//! 18/11/2026). É o menor conjunto possível: um syscall de filesystem que
//! `/proc` não expõe e que sem `unsafe` não se chama à mão. `procfs` 0.18.0 e
//! `if-addrs` **não** entraram: os arquivos de `/proc` desta fatia são
//! interfaces documentadas e estáveis, e o valor de abstração é negativo com
//! os parsers já escritos e testados por fixture. Ver a evidência
//! `docs/09-equipe/evidencias/2026-11-18-host-telemetria.md` § dependências.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod collect;
pub mod config;
pub mod expose;
pub mod proc;
pub mod read;
pub mod snapshot;
pub mod tree;
pub mod volumes;

pub use collect::{Collector, SharedSnapshot, SysinfoVolumes, VolumeSource};
pub use config::Config;
pub use snapshot::{Capacity, Group, PressureScope, Snapshot, Source, Unavailable};

/// Orçamento declarado da coleta: fração do intervalo de coleta que a coleta
/// pode ocupar. Com 5 s de intervalo, 250 ms.
///
/// É uma escolha desta fatia, não um parâmetro aprovado: a próxima revisão
/// pode discordar com um número, e é por isso que ele está aqui e não solto
/// num teste.
pub const BUDGET_FRACTION: u32 = 20;

/// Máximo de tempo que uma coleta pode custar dado um intervalo.
#[must_use]
pub fn budget(interval: std::time::Duration) -> std::time::Duration {
    // O intervalo é o teto do orçamento e nunca entra em overflow: `as_nanos`
    // de um `Duration` de escala humana cabe em u128 com folga.
    let nanos = interval.as_nanos().max(1_000);
    let share = nanos / u128::from(BUDGET_FRACTION);
    std::time::Duration::from_nanos(u64::try_from(share).unwrap_or(1).max(1))
}

#[cfg(test)]
mod tests {
    use super::budget;
    use std::time::Duration;

    #[test]
    fn orcamento_e_cinco_por_cento_do_intervalo() {
        assert_eq!(budget(Duration::from_secs(5)), Duration::from_millis(250));
        assert_eq!(budget(Duration::from_secs(1)), Duration::from_millis(50));
        // 500 µs de intervalo dão 25 µs de orçamento — o piso de 1 ns só vale
        // para um intervalo sub-microsegundo, que a config já recusa.
        assert_eq!(budget(Duration::from_micros(500)), Duration::from_micros(25));
        assert_eq!(budget(Duration::from_nanos(500)), Duration::from_nanos(50));
    }
}
