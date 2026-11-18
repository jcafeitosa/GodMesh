//! Coletor: monta um [`Snapshot`] a partir dos arquivos de host.
//!
//! Duas coisas estruturam este arquivo:
//!
//! 1. **nenhum caminho de requisição lê `/proc`.** A coleta acontece num
//!    intervalo próprio ([`crate::config`]) e o handler serve o último
//!    snapshot. Um scrape que abrisse 12 arquivos do kernel colocaria o custo
//!    da observabilidade no meio do caminho de quem faz o scrape — e num
//!    incidente é exatamente aí que a telemetria é mais procurada.
//! 2. **cada grupo declara sua fonte ou sua ausência.** `Err(Unavailable)`
//!    nunca vira valor padrão: o grupo sai marcado como indisponível e as
//!    séries dele simplesmente não aparecem ([`crate::expose`] regra 2).
//!
//! Volumes entram por [`VolumeSource`] porque `statfs` não tem fixture: o
//! teste fornece a lista e a produção chama `sysinfo`. Todo o resto é lido do
//! [`Read`], cuja raiz é um diretório de fixture no teste e `/` na produção —
//! ou seja, o mesmo código que roda no nó é o que o teste exercita.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::proc;
use crate::read::Read;
use crate::snapshot::{
    Cgroup, Conntrack, CpuTimes, Descriptors, Group, GroupStatus, PressureScope, Snapshot,
    Source, TcpStates, Unavailable, Volume,
};
use crate::volumes::{self, MAX_INTERFACES, MAX_VOLUMES};

/// Onde buscar os volumes montados.
///
/// Objeto de teste: `statfs` é uma chamada de sistema, não um arquivo — sem
/// esta costura o teste dependeria do disco do CI, e a tarefa proíbe isso.
pub trait VolumeSource: Send + Sync + std::fmt::Debug {
    fn volumes(&self) -> Vec<Volume>;
}

/// Fonte de produção: `statfs` via `sysinfo`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SysinfoVolumes;

impl VolumeSource for SysinfoVolumes {
    fn volumes(&self) -> Vec<Volume> {
        volumes::volumes()
    }
}

/// Fonte fixa, para teste e para `--proc-fs` de evidência.
#[derive(Debug, Clone)]
pub struct StaticVolumes(pub Vec<Volume>);

impl VolumeSource for StaticVolumes {
    fn volumes(&self) -> Vec<Volume> {
        self.0.clone()
    }
}

/// Estado entre coletas: o instante anterior é o que transforma contadores em
/// taxas.
#[derive(Debug, Default)]
struct Previous {
    cpu: Option<(CpuTimes, Instant)>,
}

/// Coletor de telemetria do host. `Send` mas não `Sync`: um dono por vez, e o
/// snapshot publicado vai para [`SharedSnapshot`].
#[derive(Debug, Clone)]
pub struct Collector {
    read: Read,
    volume_source: Arc<dyn VolumeSource>,
    previous: Arc<Mutex<Previous>>,
}

impl Collector {
    /// Coletor de produção: lê `/` e usa `statfs`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_read(Read::default(), Arc::new(SysinfoVolumes))
    }

    /// Coletor enraizado num diretório (fixture ou `--proc-fs`), com fonte de
    /// volumes explícita.
    #[must_use]
    pub fn with_read(read: Read, volume_source: Arc<dyn VolumeSource>) -> Self {
        Self {
            read,
            volume_source,
            previous: Arc::new(Mutex::new(Previous::default())),
        }
    }

    /// Raiz de leitura efetiva, para o `--dump` dizer de onde vieram os números.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.read.root()
    }

    /// Faz uma coleta. Nunca falha: o que não foi lido é marcado e publicado
    /// como ausência, porque a observabilidade não pode derrubar o nó.
    #[must_use]
    pub fn collect(&self) -> Snapshot {
        let started = Instant::now();
        let mut snapshot = Snapshot::empty();
        snapshot.taken_at_millis = unix_millis();

        let cgroup_info = self.cgroup_info();
        snapshot.container = cgroup_info.is_some();

        self.collect_cpu(&mut snapshot);
        self.collect_memory(&mut snapshot);
        self.collect_pressure(&mut snapshot, cgroup_info.as_ref());
        self.collect_descriptors(&mut snapshot);
        self.collect_sockets(&mut snapshot);
        self.collect_tcp(&mut snapshot);
        self.collect_conntrack(&mut snapshot);
        self.collect_interfaces(&mut snapshot);
        self.collect_volumes(&mut snapshot);
        if let Some(info) = &cgroup_info {
            self.collect_cgroup(&mut snapshot, info);
        }

        snapshot.source = dominant_source(&snapshot);
        snapshot.collection_seconds = started.elapsed().as_secs_f64();
        snapshot
    }

    // --- grupos ------------------------------------------------------------

    fn collect_cpu(&self, snapshot: &mut Snapshot) {
        // Contadores absolutos primeiro; a taxa só existe com uma amostra
        // anterior, e a primeira coleta do processo declara isso com a série
        // ausente, não com uma taxa inventada.
        match self.read.text("proc/stat") {
            Ok(text) => match proc::cpu_stat(&text) {
                Some(times) => {
                    let window = {
                        let mut previous = lock(&self.previous);
                        let window = previous.cpu.as_ref().and_then(|(before, instant)| {
                            proc::cpu_window(before, &times, instant.elapsed().as_secs_f64())
                        });
                        previous.cpu = Some((times.clone(), Instant::now()));
                        window
                    };
                    snapshot.cpu_times = Ok(times);
                    snapshot.cpu_window = window;
                }
                None => {
                    snapshot.cpu_times = Err(Unavailable::Malformed);
                    snapshot.cpu_window = None;
                }
            },
            Err(e) => {
                snapshot.cpu_times = Err(e);
                snapshot.cpu_window = None;
            }
        }

        snapshot.load = match self.read.text("proc/loadavg") {
            Ok(text) => proc::loadavg(&text).ok_or(Unavailable::Malformed),
            Err(e) => Err(e),
        };

        // Threads deste processo é leitura própria do `/proc/self/status`; a
        // ausência dela não apaga o grupo de CPU, que tem outras duas fontes.
        snapshot.process_threads = match self.read.text("proc/self/status") {
            Ok(text) => proc::status_threads(&text).ok_or(Unavailable::Malformed),
            Err(e) => Err(e),
        };

        let qualquer = snapshot.cpu_times.is_ok()
            || snapshot.load.is_ok()
            || snapshot.process_threads.is_ok();
        if qualquer {
            snapshot.mark_available(Group::Cpu, Source::Proc);
        } else {
            snapshot.mark_unavailable(Group::Cpu);
        }
    }

    fn collect_memory(&self, snapshot: &mut Snapshot) {
        snapshot.memory = self.read.text("proc/meminfo").and_then(|text| proc::meminfo(&text));
        if snapshot.memory.is_ok() {
            snapshot.mark_available(Group::Memory, Source::Proc);
        } else {
            snapshot.mark_unavailable(Group::Memory);
        }
    }

    fn collect_pressure(
        &self,
        snapshot: &mut Snapshot,
        cgroup_info: Option<&CgroupInfo>,
    ) {
        let mut found = Vec::new();
        let mut host_source = None;

        for resource in proc::PRESSURE_RESOURCES {
            let path = format!("proc/pressure/{resource}");
            match self.read.text(&path) {
                Ok(text) => {
                    if let Ok(pressure) = proc::pressure(resource, PressureScope::Host, &text) {
                        found.push(pressure);
                        host_source = Some(Source::Proc);
                    }
                }
                // PSI ausente do kernel (CONFIG_PSI=n) é `Missing`: degrada em
                // silêncio, sem poluir o painel com zero.
                Err(Unavailable::Missing) | Err(Unavailable::Malformed) => {}
                Err(e) => {
                    // Legível mas proibido: é requisito que falta, e isso vai
                    // para o dump, não para uma série.
                    let _ = e;
                }
            }
        }

        if let Some(info) = cgroup_info {
            for resource in proc::PRESSURE_RESOURCES {
                let path = info.file(&format!("{resource}.pressure"));
                if let Ok(text) = self.read.text(&path)
                    && let Ok(pressure) = proc::pressure(resource, PressureScope::Cgroup, &text)
                {
                    found.push(pressure);
                }
            }
        }

        let from_cgroup = found.iter().any(|p| p.scope == PressureScope::Cgroup);
        if found.is_empty() {
            snapshot.pressure.clear();
            snapshot.mark_unavailable(Group::Pressure);
        } else {
            snapshot.pressure = found;
            let source = host_source.unwrap_or(if from_cgroup {
                Source::Cgroup
            } else {
                Source::None
            });
            snapshot.mark_available(Group::Pressure, source);
        }
    }

    fn collect_descriptors(&self, snapshot: &mut Snapshot) {
        let open = self
            .read
            .entries("proc/self/fd")
            .map(|entries| entries.len() as u64);

        let limits = self
            .read
            .text("proc/self/limits")
            .ok()
            .and_then(|text| proc::max_open_files(&text));

        snapshot.descriptors = match (open, limits) {
            (Ok(open), Some((soft_limit, hard_limit))) => {
                snapshot.mark_available(Group::Descriptors, Source::Proc);
                Ok(Descriptors {
                    open,
                    soft_limit,
                    hard_limit,
                })
            }
            // A contagem de `fd` é real mesmo sem `/proc/self/limits`: o grupo
            // fica disponível e a série de limite simplesmente não sai.
            (Ok(open), None) => {
                snapshot.mark_available(Group::Descriptors, Source::Proc);
                Ok(Descriptors {
                    open,
                    soft_limit: None,
                    hard_limit: None,
                })
            }
            (Err(e), _) => {
                snapshot.mark_unavailable(Group::Descriptors);
                Err(e)
            }
        };
    }

    fn collect_sockets(&self, snapshot: &mut Snapshot) {
        snapshot.sockets = self
            .read
            .text("proc/net/sockstat")
            .map(|text| proc::sockstat(&text));
        if snapshot.sockets.is_ok() {
            snapshot.mark_available(Group::Sockets, Source::Proc);
        } else {
            snapshot.mark_unavailable(Group::Sockets);
        }
    }

    fn collect_tcp(&self, snapshot: &mut Snapshot) {
        let mut states = TcpStates::default();
        let mut read_any = false;
        let mut error = None;

        for path in ["proc/net/tcp", "proc/net/tcp6"] {
            match self.read.text(path) {
                Ok(text) => {
                    if proc::tcp_states(&text, &mut states).is_ok() {
                        read_any = true;
                    }
                }
                // tcp6 ausente em kernel sem IPv6 não é falha do grupo: o IPv4
                // lido continua valendo.
                Err(Unavailable::Missing) => {}
                Err(e) => error = Some(e),
            }
        }

        snapshot.tcp = if read_any {
            snapshot.mark_available(Group::Tcp, Source::Proc);
            Ok(states)
        } else {
            snapshot.mark_unavailable(Group::Tcp);
            Err(error.unwrap_or(Unavailable::Missing))
        };
    }

    fn collect_conntrack(&self, snapshot: &mut Snapshot) {
        let count = self
            .read
            .token("proc/sys/net/netfilter/nf_conntrack_count")
            .and_then(|text| proc::conntrack_value(&text));
        let max = self
            .read
            .token("proc/sys/net/netfilter/nf_conntrack_max")
            .ok()
            .and_then(|text| text.parse::<u64>().ok());

        snapshot.conntrack = match count {
            Ok(count) => {
                snapshot.mark_available(Group::Conntrack, Source::Proc);
                Ok(Conntrack { count, max })
            }
            // Módulo não carregado (`Missing`) e sem privilégio
            // (`PermissionDenied`) são coisas diferentes no dump, iguais no
            // painel: nenhuma série de conntrack sai.
            Err(e) => {
                snapshot.mark_unavailable(Group::Conntrack);
                Err(e)
            }
        };
    }

    fn collect_interfaces(&self, snapshot: &mut Snapshot) {
        snapshot.interfaces = Vec::new();
        snapshot.interfaces_truncated = false;
        let text = match self.read.text("proc/net/dev") {
            Ok(text) => text,
            Err(_) => {
                snapshot.mark_unavailable(Group::NetDev);
                return;
            }
        };
        let mut parsed = proc::net_dev(&text);
        snapshot.interfaces_truncated = parsed.len() > MAX_INTERFACES;
        parsed.truncate(MAX_INTERFACES);
        snapshot.interfaces = parsed;
        snapshot.mark_available(Group::NetDev, Source::Proc);
    }

    fn collect_volumes(&self, snapshot: &mut Snapshot) {
        let mut all: Vec<Volume> = self
            .volume_source
            .volumes()
            .into_iter()
            .filter(|volume| !volumes::is_pseudo(&volume.file_system))
            .collect();
        all.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        snapshot.volumes_truncated = all.len() > MAX_VOLUMES;
        all.truncate(MAX_VOLUMES);
        if all.is_empty() {
            snapshot.volumes.clear();
            snapshot.mark_unavailable(Group::Disk);
        } else {
            snapshot.volumes = all;
            snapshot.mark_available(Group::Disk, Source::Statfs);
        }
    }

    fn collect_cgroup(&self, snapshot: &mut Snapshot, info: &CgroupInfo) {
        let mut cgroup = Cgroup {
            version: Some(info.version),
            ..Cgroup::default()
        };

        if info.version == 2 {
            cgroup.memory_current_bytes = self.u64_file(&info.file("memory.current"));
            cgroup.memory_limit_bytes = self.limit_file(&info.file("memory.max"));
            cgroup.cpu_quota_cores = self.cpu_quota_file(&info.file("cpu.max"));
            if let Ok(text) = self.read.text(&info.file("cpu.stat")) {
                cgroup.cpu_stat = proc::cpu_stat_pairs(&text);
            }
        } else {
            // v1: nomes clássicos de memória. Pressão por cgroup não existe
            // em v1, e a quota de CPU está em outra subtree — nada aqui é
            // lido por adivinhação de caminho.
            cgroup.memory_current_bytes = self.u64_file(&info.file("memory.usage_in_bytes"));
            cgroup.memory_limit_bytes = self.limit_file_v1(&info.file("memory.limit_in_bytes"));
        }

        snapshot.cgroup = Ok(cgroup);
        snapshot.mark_available(Group::Cgroup, Source::Cgroup);
    }

    // --- utilidade ---------------------------------------------------------

    /// Detecta se estamos dentro de um namespace de contêiner e onde ficam os
    /// arquivos do nosso cgroup.
    ///
    /// Critério honesto: `/proc/self/cgroup` com um caminho **não-root**. Em
    /// um processo de host comum o caminho é `/` e nada aqui é afirmado sobre
    /// contêiner.
    ///
    /// v1: usamos o caminho da controller de **memória** junto da montagem
    /// escolhida por `cgroup_mount` (a subtree de memória). Os arquivos de
    /// quota de CPU ficam em outra subtree, e esta fatia não os lê — o painel
    /// então não publica `cgroup_cpu_limit_cores` em v1, o que é declarado na
    /// evidência em vez de ser adivinhado.
    fn cgroup_info(&self) -> Option<CgroupInfo> {
        let text = self.read.text("proc/self/cgroup").ok()?;
        let mounts = self
            .read
            .text("proc/self/mounts")
            .or_else(|_| self.read.text("proc/mounts"))
            .ok()?;
        let (mount, version) = proc::cgroup_mount(&mounts)?;
        let path = if version == 2 {
            proc::cgroup_path(&text)?
        } else {
            proc::cgroup_memory_path(&text).or_else(|| proc::cgroup_path(&text))?
        };
        if path.is_empty() {
            return None;
        }
        Some(CgroupInfo {
            base: format!("{mount}/{path}"),
            version,
        })
    }

    fn u64_file(&self, relative: &str) -> Option<u64> {
        let text = self.read.text(relative).ok()?;
        proc::cgroup_limit(&text).ok().flatten()
    }

    /// Limite de cgroup v2: `max`, ilegível e ausente são todos `None`; a
    /// série não sai e a `HELP` diz por quê.
    fn limit_file(&self, relative: &str) -> Option<u64> {
        let text = self.read.text(relative).ok()?;
        proc::cgroup_limit(&text).ok().flatten()
    }

    /// Limite em cgroup v1, onde "sem limite" é uma sentinela numérica e não
    /// o literal `max` de v2 — ver [`proc::cgroup_limit_v1`].
    fn limit_file_v1(&self, relative: &str) -> Option<u64> {
        let text = self.read.text(relative).ok()?;
        proc::cgroup_limit_v1(&text).ok().flatten()
    }

    fn cpu_quota_file(&self, relative: &str) -> Option<f64> {
        let text = self.read.text(relative).ok()?;
        proc::cgroup_cpu_quota(&text).ok().flatten()
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

/// Onde estão os arquivos do nosso cgroup.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CgroupInfo {
    base: String,
    version: u8,
}

impl CgroupInfo {
    fn file(&self, name: &str) -> String {
        format!("{}/{}", self.base.trim_start_matches('/'), name)
    }
}

/// Fonte dominante do snapshot inteiro, do mais informativo para o menos.
fn dominant_source(snapshot: &Snapshot) -> Source {
    let proc_ok = Group::ALL.iter().any(|group| {
        matches!(
            snapshot.groups.get(group),
            Some(GroupStatus::Available(Source::Proc))
        )
    });
    if proc_ok {
        return Source::Proc;
    }
    if snapshot.group_available(Group::Cgroup) {
        return Source::Cgroup;
    }
    if snapshot.group_available(Group::Disk) {
        return Source::Statfs;
    }
    Source::None
}

/// Snapshot publicado, lido pelo handler de `/metrics` e pelo `/healthz`.
///
/// `Mutex` e não átomos porque o snapshot é uma estrutura grande e trocada
/// inteira; a seção crítica é uma cópia de ponteiro (`Arc::swap`), nunca um
/// parse. Um handler que pega o lock envenenado (pânico de outra tarefa) serve
/// "sem snapshot" — diagnóstico não pode propagar pânico.
#[derive(Debug, Clone, Default)]
pub struct SharedSnapshot {
    inner: Arc<Mutex<Option<Arc<Snapshot>>>>,
}

impl SharedSnapshot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Publica um snapshot, substituindo o anterior.
    pub fn store(&self, snapshot: Snapshot) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(Arc::new(snapshot));
        }
    }

    /// Última coleta publicada, ou `None` antes da primeira.
    #[must_use]
    pub fn latest(&self) -> Option<Arc<Snapshot>> {
        self.inner.lock().ok().and_then(|guard| guard.clone())
    }

    /// Custo de uma leitura: é `Arc::clone` de um snapshot pronto. Nada de
    /// `/proc` no caminho da requisição.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.latest().is_none()
    }
}

fn lock(mutex: &Arc<Mutex<Previous>>) -> std::sync::MutexGuard<'_, Previous> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// ms desde o epoch. Usado só para idade de snapshot; janelas usam
/// relógio monotônico.
#[must_use]
pub fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// Mapa de grupos → disponibilidade, para o `--dump` de evidência.
#[must_use]
pub fn group_report(snapshot: &Snapshot) -> BTreeMap<&'static str, &'static str> {
    Group::ALL
        .iter()
        .map(|group| {
            (
                group.as_str(),
                match snapshot.groups.get(group) {
                    Some(GroupStatus::Available(source)) => source.as_str(),
                    _ => "none",
                },
            )
        })
        .collect()
}
