//! Modelo de dados de um snapshot do host.
//!
//! A regra que governa este arquivo inteiro: **ausência é `None`/grupo
//! indisponível, nunca `0`**. Um grupo que não pôde ser lido não tem série; o
//! que afirma a falha é `godmesh_host_group_available` (ver [`crate::expose`]).
//! Zero é um valor medido, não um valor de erro — e "não sei a pressão" não é
//! "pressão zero".
//!
//! Nenhum campo é identificador de tenant, usuário ou conexão: agregação por
//! volume, por interface e por estado TCP, com teto de cardinalidade aplicado
//! em [`crate::collect`] (ADR-073, consequências).

use std::collections::BTreeMap;

/// Fonte efetiva de uma leitura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// Nada legível: `/proc` ausente (fora de Linux) ou permissão negada.
    None,
    /// `statfs(2)` via `sysinfo` — só existem os volumes.
    Statfs,
    /// Cgroup deste namespace. **É o limite do contêiner, não o hipervisor
    /// físico**; o rótulo da série declara isso (ADR-073, evidência obrigatória).
    Cgroup,
    /// `/proc` do Linux.
    Proc,
}

impl Source {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Statfs => "statfs",
            Self::Cgroup => "cgroup",
            Self::Proc => "proc",
        }
    }

    /// Alguma leitura de host funcionou.
    #[must_use]
    pub const fn available(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Grupos de leitura, com cardinalidade fechada (rótulo fixo por grupo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Cpu,
    Memory,
    Pressure,
    Disk,
    Descriptors,
    Sockets,
    Tcp,
    Conntrack,
    NetDev,
    Cgroup,
}

impl Group {
    /// Todos os grupos, na ordem de renderização.
    pub const ALL: [Self; 10] = [
        Self::Cpu,
        Self::Memory,
        Self::Pressure,
        Self::Disk,
        Self::Descriptors,
        Self::Sockets,
        Self::Tcp,
        Self::Conntrack,
        Self::NetDev,
        Self::Cgroup,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Pressure => "pressure",
            Self::Disk => "disk",
            Self::Descriptors => "descriptors",
            Self::Sockets => "sockets",
            Self::Tcp => "tcp",
            Self::Conntrack => "conntrack",
            Self::NetDev => "netdev",
            Self::Cgroup => "cgroup",
        }
    }
}

/// Por que um grupo não tem número. O motivo vai no relatório de `--dump`,
/// não no exposition: um rótulo por motivo seria cardinalidade por erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// O arquivo não existe no caminho lido (PSI sem `CONFIG_PSI`, `/proc`
    /// ausente em macOS, conntrack sem o módulo carregado).
    Missing,
    /// Existe mas não é legível sem privilégio (ex.: `nf_conntrack_count`
    /// `0644` em alguns kernels, `0440 root:conntrack` noutros).
    PermissionDenied,
    /// Leu, mas o conteúdo não é o formato esperado.
    Malformed,
    /// O arquivo apareceu e falhou no meio da leitura (pseudofs sob pressão);
    /// a mensagem do SO vai junto, só no relatório de `--dump`.
    Io(String),
}

/// Vocabulário fechado dos estados TCP, **indexado pelo valor** da coluna `st`
/// de `/proc/net/tcp` (hex).
///
/// A numeração é a do `sk_state` do kernel (TCP_ESTABLISHED = 1 …), espelhada
/// pelo enum `SS_*` de `misc/ss.c` do iproute2 — conferido em 18/11/2026 em
/// <https://github.com/iproute2/iproute2/blob/main/misc/ss.c>. O índice 0 não
/// é um estado de socket TCP utilizável, por isso "unknown".
///
/// Nomear por índice (e não por posição em lista) é o que impede que um estado
/// novo do kernel seja silenciosamente rotulado como um estado velho.
pub const TCP_STATE_NAMES: [&str; 13] = [
    "unknown",
    "established",
    "syn_sent",
    "syn_recv",
    "fin_wait1",
    "fin_wait2",
    "time_wait",
    "close",
    "close_wait",
    "last_ack",
    "listen",
    "closing",
    "new_syn_recv",
];

/// Estado fora da tabela: nunca mapeado num estado velho.
pub const TCP_STATE_UNKNOWN: &str = "unknown";

/// Resultado da leitura de um grupo: os dados, ou o motivo da ausência.
pub type GroupResult<T> = Result<T, Unavailable>;

/// Estado de um grupo nesta coleta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupStatus {
    /// Lido com sucesso; `source` diz **de onde** veio.
    Available(Source),
    /// Indisponível: nenhuma série do grupo sai no `/metrics`.
    Unavailable,
}

/// Classes de uso de CPU, na ordem das colunas da linha `cpu` de `/proc/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CpuState {
    User,
    Nice,
    System,
    Idle,
    IoWait,
    Irq,
    SoftIrq,
    Steal,
}

/// Ordem fixa das classes, usada por parser e renderização.
pub const CPU_STATE_ORDER: [CpuState; 8] = [
    CpuState::User,
    CpuState::Nice,
    CpuState::System,
    CpuState::Idle,
    CpuState::IoWait,
    CpuState::Irq,
    CpuState::SoftIrq,
    CpuState::Steal,
];

impl CpuState {
    /// Rótulo estável no exposition.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Nice => "nice",
            Self::System => "system",
            Self::Idle => "idle",
            Self::IoWait => "io_wait",
            Self::Irq => "irq",
            Self::SoftIrq => "soft_irq",
            Self::Steal => "steal",
        }
    }

    /// Classe da coluna `index` da linha `cpu`.
    #[must_use]
    pub const fn column(index: usize) -> Option<Self> {
        if index < CPU_STATE_ORDER.len() {
            Some(CPU_STATE_ORDER[index])
        } else {
            None
        }
    }

    /// Entra em "ocupada" na fração `busy` (user+nice+system+irq+softirq).
    /// `idle` não conta; `io_wait` e `steal` têm séries próprias.
    #[must_use]
    pub const fn is_busy(self) -> bool {
        matches!(
            self,
            Self::User | Self::Nice | Self::System | Self::Irq | Self::SoftIrq
        )
    }
}

/// Tempos de CPU desde o boot, por classe, em jiffies (unidade de `/proc/stat`).
///
/// A conversão para segundos é feita com `f64` no momento do uso: dividir
/// jiffies inteiros por `USER_HZ` truncaria cada classe em até 10 ms.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpuTimes {
    /// jiffies acumulados por classe
    pub jiffies: BTreeMap<CpuState, u64>,
    /// `USER_HZ` (historicamente 100 no Linux; vem de `/proc/stat` por definição)
    pub hertz: u64,
    /// núcleos lógicos, contados nas linhas `cpuN`
    pub cores: u32,
}

impl CpuTimes {
    /// Segundos acumulados numa classe.
    #[must_use]
    pub fn seconds(&self, state: CpuState) -> f64 {
        let hertz = u64::max(self.hertz, 1) as f64;
        *self.jiffies.get(&state).unwrap_or(&0) as f64 / hertz
    }

    /// Segundos totais desde o boot, somando todas as classes.
    #[must_use]
    pub fn total_seconds(&self) -> f64 {
        let hertz = u64::max(self.hertz, 1) as f64;
        self.jiffies.values().sum::<u64>() as f64 / hertz
    }
}

/// Uso de CPU na **janela entre duas coletas**.
///
/// Sem amostra anterior não existe janela: o campo é `None` e a série de razão
/// não sai no `/metrics`. Uma taxa exige dois instantes; um instante não taxa.
#[derive(Debug, Clone, PartialEq)]
pub struct CpuWindow {
    /// Duração real da janela em segundos (relógio monotônico).
    pub window_seconds: f64,
    /// Fração do tempo com CPU ocupada (user+nice+system+irq+softirq).
    pub busy_ratio: f64,
    /// Fração por classe, no mesmo denominador.
    pub ratios: BTreeMap<CpuState, f64>,
}

/// Carga média de `/proc/loadavg`.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
    /// `running/total` de threads no instante da leitura.
    pub running_threads: u64,
    pub total_threads: u64,
}

/// Memória de `/proc/meminfo`, em bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    pub total_bytes: u64,
    pub free_bytes: u64,
    /// `None` em kernel sem `MemAvailable` (< 3.14). Não estimamos por
    /// heurística de cache: um número inventado seria pior que nenhum.
    pub available_bytes: Option<u64>,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}

impl Memory {
    /// Usado = total − disponível. Derivação declarada na HELP; se disponível
    /// é desconhecido, usado é desconhecido.
    #[must_use]
    pub fn used_bytes(&self) -> Option<u64> {
        self.available_bytes
            .and_then(|available| self.total_bytes.checked_sub(available))
    }

    /// `None` quando a base de comparação falta ou é zero.
    #[must_use]
    pub fn usage_ratio(&self) -> Option<f64> {
        let used = self.used_bytes()?;
        if self.total_bytes == 0 {
            return None;
        }
        Some(used as f64 / self.total_bytes as f64)
    }

    ///swap: fração em uso do espaço de troca, `None` com swap de zero bytes.
    #[must_use]
    pub fn swap_used_ratio(&self) -> Option<f64> {
        if self.swap_total_bytes == 0 {
            return None;
        }
        let used = self.swap_total_bytes.saturating_sub(self.swap_free_bytes);
        Some(used as f64 / self.swap_total_bytes as f64)
    }
}

/// Escopo de uma leitura de pressão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PressureScope {
    /// `/proc/pressure/<recurso>`: pressão do kernel como este namespace a vê.
    Host,
    /// `<cgroup>/<recurso>.pressure`: pressão do **cgroup próprio**, que em
    /// contêiner é o número que diz respeito a este processo.
    Cgroup,
}

impl PressureScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Cgroup => "cgroup",
        }
    }
}

/// Uma linha `some=` ou `full=` de PSI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureLine {
    /// percentual médio de tarefas stalled nos últimos 10 s (0..100)
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    /// `total=` em microssegundos acumulados de stall
    pub total_microseconds: u64,
}

impl PressureLine {
    /// `total=` em segundos (a série vai com unidade no nome).
    #[must_use]
    pub fn total_seconds(&self) -> f64 {
        self.total_microseconds as f64 / 1_000_000.0
    }
}

/// Pressão de um recurso (`cpu` | `memory` | `io`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pressure {
    /// Recurso do arquivo lido — vira o rótulo `resource`.
    pub resource: &'static str,
    pub scope: PressureScope,
    /// Linha `some:` — `None` em kernel sem `full` (PSI < 5.x só tem `some`).
    pub some: Option<PressureLine>,
    /// Linha `full:` — `None` quando o recurso não reporta stall total.
    pub full: Option<PressureLine>,
}

impl Pressure {
    /// Média de 10 s da linha `some`, a mais sensível para um resumo.
    #[must_use]
    pub fn some_avg10(&self) -> Option<f64> {
        self.some.map(|line| line.avg10)
    }
}

/// Volume montado, lido por `statfs(2)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Volume {
    /// Ponto de montagem, rótulo `volume`. Não o device: muda com hotplug e
    /// com o nome do disco do provedor.
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub read_only: bool,
}

impl Volume {
    #[must_use]
    pub fn used_ratio(&self) -> Option<f64> {
        if self.total_bytes == 0 {
            return None;
        }
        let used = self.total_bytes.saturating_sub(self.available_bytes);
        Some(used as f64 / self.total_bytes as f64)
    }

    #[must_use]
    pub fn free_ratio(&self) -> Option<f64> {
        if self.total_bytes == 0 {
            return None;
        }
        Some(self.available_bytes as f64 / self.total_bytes as f64)
    }
}

/// Descritores e limites de arquivo deste **processo** (não do host).
///
/// Os limites são `Option`: `unlimited` de `/proc/self/limits` não tem número,
/// e preenchê-lo com `0` faria um processo sem limite parecer um processo com o
/// limite estourado.
#[derive(Debug, Clone, PartialEq)]
pub struct Descriptors {
    pub open: u64,
    pub soft_limit: Option<u64>,
    pub hard_limit: Option<u64>,
}

impl Descriptors {
    /// `None` sem limite conhecido ou com limite zero (não se divide por zero).
    #[must_use]
    pub fn usage_ratio(&self) -> Option<f64> {
        let limit = self.soft_limit?;
        if limit == 0 {
            return None;
        }
        Some(self.open as f64 / limit as f64)
    }
}

/// Sockets em uso, de `/proc/net/sockstat`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sockets {
    /// `sockets: used` — total de sockets do kernel.
    pub used: Option<u64>,
    /// `inuse` por protocolo (`tcp`, `udp`, `udp_lite`, `unix`).
    pub by_protocol: BTreeMap<String, u64>,
    /// memória por protocolo, **em páginas** (unidade declarada no nome).
    pub memory_pages: BTreeMap<String, u64>,
}

/// Estados TCP agregados de `/proc/net/tcp` e `/proc/net/tcp6`.
///
/// A contagem é por estado, nunca por conexão: uma série por socket em um nó
/// com 200 mil time-wait seria um incidente de observabilidade.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TcpStates {
    /// contagem por estado, somando IPv4 e IPv6
    pub by_state: BTreeMap<&'static str, u64>,
}

impl TcpStates {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.by_state.values().sum()
    }
}

/// Contadores de conntrack lidos por arquivo. **Sem netlink nesta fatia**
/// (ADR-073); `max` pode exigir privilégio — nesse caso é `None`, não zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Conntrack {
    pub count: u64,
    pub max: Option<u64>,
}

impl Conntrack {
    #[must_use]
    pub fn usage_ratio(&self) -> Option<f64> {
        let max = self.max?;
        if max == 0 {
            return None;
        }
        Some(self.count as f64 / max as f64)
    }
}

/// Contadores de uma interface, de `/proc/net/dev`.
#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

/// Limites e contadores do cgroup deste processo — o número do **namespace**.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cgroup {
    /// `2` unified, `1` v1. `None` quando não sabemos qual hierarquia.
    pub version: Option<u8>,
    pub memory_current_bytes: Option<u64>,
    /// `memory.max`; `None` com `max` (sem limite) ou ilegível.
    pub memory_limit_bytes: Option<u64>,
    /// quota de CPU em núcleos (`cpu.max` = quota/período); `None` com `max`.
    pub cpu_quota_cores: Option<f64>,
    /// `cpu.stat`, chaves originais (`nr_periods`, `nr_throttled`,
    /// `throttled_usec`, …).
    pub cpu_stat: BTreeMap<String, u64>,
}

/// Um snapshot: o resultado de uma coleta. Imutável depois de pronto; o
/// handler de `/metrics` e o `/healthz` só leem o último.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Instante da coleta em ms desde o epoch (usado só para idade do
    /// snapshot; janelas de CPU usam relógio monotônico).
    pub taken_at_millis: u64,
    /// Fonte dominante desta coleta.
    pub source: Source,
    /// Custo da coleta em segundos — o orçamento declarado no task 4, medido.
    pub collection_seconds: f64,
    /// `true` quando a leitura veio de um namespace de contêiner.
    pub container: bool,
    /// Por grupo: disponível (e de onde veio) ou ausente.
    pub groups: BTreeMap<Group, GroupStatus>,
    pub cpu_times: GroupResult<CpuTimes>,
    /// `None` até a segunda coleta: taxa exige duas amostras.
    pub cpu_window: Option<CpuWindow>,
    pub load: GroupResult<LoadAverage>,
    /// threads deste processo, de `/proc/self/status`.
    pub process_threads: GroupResult<u64>,
    pub memory: GroupResult<Memory>,
    /// pressões lidas (host e/ou cgroup); vazio quando nenhuma existe.
    pub pressure: Vec<Pressure>,
    pub volumes: Vec<Volume>,
    /// `true` quando volumes foram cortados pelo teto de cardinalidade.
    pub volumes_truncated: bool,
    pub descriptors: GroupResult<Descriptors>,
    pub sockets: GroupResult<Sockets>,
    pub tcp: GroupResult<TcpStates>,
    pub conntrack: GroupResult<Conntrack>,
    pub interfaces: Vec<Interface>,
    pub interfaces_truncated: bool,
    pub cgroup: GroupResult<Cgroup>,
}

impl Snapshot {
    /// Snapshot vazio: nada lido ainda. O `/metrics` responde
    /// `godmesh_host_metrics_available{source="none"} 0` — não zeros.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            taken_at_millis: 0,
            source: Source::None,
            collection_seconds: 0.0,
            container: false,
            groups: Group::ALL
                .into_iter()
                .map(|group| (group, GroupStatus::Unavailable))
                .collect(),
            cpu_times: Err(Unavailable::Missing),
            cpu_window: None,
            load: Err(Unavailable::Missing),
            process_threads: Err(Unavailable::Missing),
            memory: Err(Unavailable::Missing),
            pressure: Vec::new(),
            volumes: Vec::new(),
            volumes_truncated: false,
            descriptors: Err(Unavailable::Missing),
            sockets: Err(Unavailable::Missing),
            tcp: Err(Unavailable::Missing),
            conntrack: Err(Unavailable::Missing),
            interfaces: Vec::new(),
            interfaces_truncated: false,
            cgroup: Err(Unavailable::Missing),
        }
    }

    /// Registra um grupo como lido de uma dada fonte.
    pub fn mark_available(&mut self, group: Group, source: Source) {
        self.groups.insert(group, GroupStatus::Available(source));
    }

    /// Registra um grupo como indisponível.
    pub fn mark_unavailable(&mut self, group: Group) {
        self.groups.insert(group, GroupStatus::Unavailable);
    }

    #[must_use]
    pub fn group_available(&self, group: Group) -> bool {
        matches!(
            self.groups.get(&group),
            Some(GroupStatus::Available(_))
        )
    }

    /// De onde veio a leitura de um grupo.
    #[must_use]
    pub fn group_source(&self, group: Group) -> Option<Source> {
        match self.groups.get(&group) {
            Some(GroupStatus::Available(source)) => Some(*source),
            _ => None,
        }
    }

    /// Pressão de um recurso num escopo.
    #[must_use]
    pub fn pressure_for(&self, resource: &str, scope: PressureScope) -> Option<&Pressure> {
        self.pressure
            .iter()
            .find(|pressure| pressure.resource == resource && pressure.scope == scope)
    }

    /// Resumo de capacidade para o `/healthz`.
    #[must_use]
    pub fn capacity(&self) -> Capacity {
        Capacity::from_snapshot(self)
    }
}

/// Três números que um operador olha primeiro. **Não** são um veredito de
/// saúde nesta fatia: readiness por pressão é a próxima fatia de ADR-073 e
/// precisa de limiar e histerese aprovados.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Capacity {
    /// pressão `some avg10` de CPU, em %; `None` = sem PSI
    pub cpu_pressure: Option<f64>,
    pub memory_pressure: Option<f64>,
    /// menor fração livre entre os volumes de dados (o pior caso)
    pub disk_free_ratio: Option<f64>,
}

impl Capacity {
    /// Deriva o resumo do snapshot. Pressão do cgroup tem precedência sobre a
    /// do host em contêiner: é o número que diz respeito a este processo.
    #[must_use]
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        let pick = |resource: &str| {
            if snapshot.container {
                snapshot
                    .pressure_for(resource, PressureScope::Cgroup)
                    .or_else(|| snapshot.pressure_for(resource, PressureScope::Host))
            } else {
                snapshot.pressure_for(resource, PressureScope::Host)
            }
            .and_then(Pressure::some_avg10)
        };

        // "pior caso" é o mínimo entre volumes legíveis; sem volume legível o
        // resumo é `None`, não 1.0 (que leria "muito espaço" sem medir nada).
        let disk_free_ratio = snapshot
            .volumes
            .iter()
            .filter_map(Volume::free_ratio)
            .reduce(f64::min);

        Self {
            cpu_pressure: pick("cpu"),
            memory_pressure: pick("memory"),
            disk_free_ratio,
        }
    }
}
