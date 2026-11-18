//! Exposition Prometheus das métricas de host, escrito à mão como o resto de
//! `/metrics` (sem dependência nova de formatação).
//!
//! Quatro regras, que são todo o desenho deste arquivo:
//!
//! 1. **unidade no nome**: `_bytes`, `_seconds`, `_ratio`; contagem que só
//!    cresce é `_total`; contagem instantânea não tem sufixo e a `HELP` diz a
//!    unidade (`files`, `sockets`, `packets`).
//! 2. **métrica ausente não é zero**: um grupo não lido não tem linha, **e a
//!    família inteira não é escrita** — um `# TYPE` sem série faria o scraper
//!    achar que a série existe e está vazia. O que afirma a falha são
//!    `godmesh_host_metrics_available{source=…}` e
//!    `godmesh_host_group_available{group=…}`, sempre emitidos.
//! 3. **cardinalidade limitada**: vocabulário fechado de rótulos (grupo, classe
//!    de CPU, estado TCP, protocolo, recurso/escopo de pressão, direção) mais
//!    dois abertos e *capados* — `volume` e `interface` — com
//!    `godmesh_host_cardinality_truncated` declarando o corte. Nunca uma série
//!    por conexão.
//! 4. **ordem determinística**: famílias em ordem fixa, amostras em ordem de
//!    vocabulário; `diff` entre dois scrapes de um host parado é vazio.
//!
//! Em contêiner, o que vem de cgroup tem `HELP` própria dizendo que é limite do
//! **namespace**, não do hipervisor físico (ADR-073, evidência obrigatória).

use std::fmt::Write as _;


use crate::snapshot::{
    Group, Pressure, PressureScope, Snapshot, Source, Volume, CPU_STATE_ORDER,
    TCP_STATE_NAMES,
};

/// Prefixo único das métricas de host, separando-as das de aplicação.
pub const PREFIX: &str = "godmesh_host";

/// Uma família de séries, acumulada antes de ser escrita.
///
/// O buffer existe por causa da regra 2: só se sabe se a família tem número
/// depois de tentar todos os números dela.
struct Family {
    name: String,
    help: &'static str,
    kind: &'static str,
    lines: String,
    count: usize,
}

impl Family {
    fn new(suffix: &'static str, help: &'static str, kind: &'static str) -> Self {
        Self {
            name: format!("{PREFIX}_{suffix}"),
            help,
            kind,
            lines: String::new(),
            count: 0,
        }
    }

    /// Escreve `nome{rótulos} valor`; `None` não escreve nada.
    fn sample(&mut self, value: Option<f64>, labels: &[(&str, &str)]) {
        let Some(value) = value else { return };
        if !value.is_finite() {
            // NaN/±Inf não têm representação estável no exposition 0.0.4.
            return;
        }
        self.count += 1;
        if labels.is_empty() {
            let _ = writeln!(self.lines, "{} {}", self.name, format_value(value));
            return;
        }
        let rendered = labels
            .iter()
            .map(|(key, value)| format!("{key}=\"{}\"", escape_label(value)))
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            self.lines,
            "{}{{{rendered}}} {}",
            self.name,
            format_value(value)
        );
    }

    /// Conveniência para as muitas séries sem rótulo.
    fn one(&mut self, value: Option<f64>) {
        self.sample(value, &[]);
    }

    /// Conveniência com um único rótulo.
    fn labelled(&mut self, key: &str, value: &str, number: Option<f64>) {
        self.sample(number, &[(key, value)]);
    }

    /// Escreve `# HELP`/`# TYPE` + linhas, **só se houver linha**.
    fn finish(self, out: &mut String) {
        if self.count == 0 {
            return;
        }
        let _ = writeln!(out, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(out, "# TYPE {} {}", self.name, self.kind);
        out.push_str(&self.lines);
    }
}

fn format_value(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value:.6}")
    }
}

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

/// Renderiza a seção `godmesh_host_*` do `/metrics`.
///
/// `now_millis` é o instante do scrape (para a idade do snapshot);
/// `stale_after_millis` é a idade máxima tolerada — acima dela a seção é
/// servida como indisponível, porque um número velho sem aviso é pior que
/// nenhum número. `None` desliga o julgamento de idade (testes de exposição
/// pura).
#[must_use]
pub fn render(snapshot: &Snapshot, now_millis: u64, stale_after_millis: Option<u64>) -> String {
    let mut out = String::with_capacity(6_144);
    let observed = snapshot.taken_at_millis != 0;
    let fresh = stale_after_millis.is_none_or(|limit| {
        observed && now_millis.saturating_sub(snapshot.taken_at_millis) <= limit
    });
    let available = observed && fresh && snapshot.source.available();

    render_availability(&mut out, snapshot, available);
    if !available {
        return out;
    }

    render_cpu(&mut out, snapshot);
    render_memory(&mut out, snapshot);
    render_pressure(&mut out, snapshot);
    render_cgroup(&mut out, snapshot);
    render_volumes(&mut out, snapshot);
    render_descriptors(&mut out, snapshot);
    render_sockets(&mut out, snapshot);
    render_tcp(&mut out, snapshot);
    render_conntrack(&mut out, snapshot);
    render_interfaces(&mut out, snapshot);
    render_collection(&mut out, snapshot, now_millis);
    out
}

fn render_availability(out: &mut String, snapshot: &Snapshot, available: bool) {
    let mut family = Family::new(
        "metrics_available",
        "1 quando alguma telemetria de host foi lida nesta coleta, 0 quando a leitura \
         falhou. source=\"proc\" é /proc do Linux; \"cgroup\" é o cgroup deste namespace \
         (limite do contêiner, NÃO o hipervisor físico); \"statfs\" são só volumes; \
         \"none\" é nenhuma leitura. Uma série ausente deste painel nunca deve ser \
         interpretada como zero: use este gauge para distinguir.",
        "gauge",
    );
    let active = if available { snapshot.source } else { Source::None };
    for candidate in [Source::Proc, Source::Cgroup, Source::Statfs] {
        if available {
            family.labelled("source", candidate.as_str(), Some(f64::from(candidate == active)));
        }
    }
    if !available {
        family.labelled("source", Source::None.as_str(), Some(0.0));
    }
    family.finish(out);

    let mut family = Family::new(
        "group_available",
        "1/0 por grupo de leitura de host (cpu, memory, pressure, disk, descriptors, \
         sockets, tcp, conntrack, netdev, cgroup). 0 significa \"não medimos\": as séries \
         do grupo não aparecem no /metrics. A fonte de cada grupo está na HELP da própria \
         família.",
        "gauge",
    );
    for group in Group::ALL {
        family.labelled(
            "group",
            group.as_str(),
            Some(f64::from(snapshot.group_available(group))),
        );
    }
    family.finish(out);
}

fn render_cpu(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Cpu) {
        return;
    }

    if let Ok(times) = &snapshot.cpu_times {
        let mut family = Family::new(
            "cpu_cores",
            "Núcleos lógicos visíveis neste namespace (linhas cpuN de /proc/stat). Em \
             contêiner é o que /proc mostra, que não é necessariamente a quota do \
             cgroup: ver godmesh_host_cgroup_cpu_limit_cores.",
            "gauge",
        );
        family.one(Some(f64::from(times.cores)));
        family.finish(out);

        let mut family = Family::new(
            "cpu_seconds_total",
            "Tempo de CPU por classe desde o boot, em SEGUNDOS (colunas da linha cpu de \
             /proc/stat divididas por 100 Hz, a definição pública de procps). \
             guest/guest_nice não são somados à parte: o kernel já os conta dentro de \
             user, e somá-los daria busy acima de 1. class=\"total\" é a soma das \
             oito classes.",
            "counter",
        );
        for state in CPU_STATE_ORDER {
            family.labelled("class", state.as_str(), Some(times.seconds(state)));
        }
        family.labelled("class", "total", Some(times.total_seconds()));
        family.finish(out);
    }

    if let Some(window) = &snapshot.cpu_window {
        let mut family = Family::new(
            "cpu_usage_ratio",
            "Fração (0..1) do tempo de CPU da última janela por classe; o denominador é \
             o total de jiffies da janela, então as classes somam 1 junto com idle. \
             class=\"busy\" = user+nice+system+irq+softirq. Ausente na primeira coleta: \
             taxa exige duas amostras, e uma taxa de uma amostra seria inventada.",
            "gauge",
        );
        family.labelled("class", "busy", Some(window.busy_ratio));
        for state in CPU_STATE_ORDER {
            family.labelled(
                "class",
                state.as_str(),
                window.ratios.get(&state).copied(),
            );
        }
        family.finish(out);

        let mut family = Family::new(
            "collection_window_seconds",
            "Duração real (relógio monotônico) da janela usada nas razões de CPU, em \
             segundos. Não é o intervalo configurado: atraso de escala aparece aqui.",
            "gauge",
        );
        family.one(Some(window.window_seconds));
        family.finish(out);
    }

    if let Ok(load) = &snapshot.load {
        let mut family = Family::new(
            "load_average",
            "Carga média do Linux — número médio de tarefas executáveis + não \
             interruptíveis — adimensional, de /proc/loadavg.",
            "gauge",
        );
        for (window, value) in [("1m", load.one), ("5m", load.five), ("15m", load.fifteen)] {
            family.labelled("window", window, Some(value));
        }
        family.finish(out);

        let mut family = Family::new(
            "threads",
            "Threads do sistema no instante da leitura de /proc/loadavg, \
             adimensional (contagem de threads).",
            "gauge",
        );
        family.labelled("state", "runnable", Some(load.running_threads as f64));
        family.labelled("state", "total", Some(load.total_threads as f64));
        family.finish(out);
    }

    if let Ok(threads) = snapshot.process_threads {
        let mut family = Family::new(
            "process_threads",
            "Threads DESTE PROCESSO (campo Threads: de /proc/self/status), \
             adimensional. Não é a contagem do host.",
            "gauge",
        );
        family.one(Some(threads as f64));
        family.finish(out);
    }
}

fn render_memory(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Memory) {
        return;
    }
    let Ok(memory) = &snapshot.memory else { return };

    let mut family = Family::new(
        "memory_bytes",
        "Memória em bytes, de /proc/meminfo. state=\"used\" é DERIVADO \
         (MemTotal − MemAvailable), não uma leitura do kernel; state=\"available\" é \
         ausente em kernel < 3.14.",
        "gauge",
    );
    family.labelled("state", "total", Some(memory.total_bytes as f64));
    family.labelled("state", "free", Some(memory.free_bytes as f64));
    family.labelled(
        "state",
        "available",
        memory.available_bytes.map(|value| value as f64),
    );
    family.labelled("state", "used", memory.used_bytes().map(|value| value as f64));
    family.finish(out);

    let mut family = Family::new(
        "swap_bytes",
        "Espaço de troca em bytes, de /proc/meminfo. state=\"used\" é diferença \
         derivada (SwapTotal − SwapFree).",
        "gauge",
    );
    family.labelled("state", "total", Some(memory.swap_total_bytes as f64));
    family.labelled("state", "free", Some(memory.swap_free_bytes as f64));
    family.labelled(
        "state",
        "used",
        Some(memory.swap_total_bytes.saturating_sub(memory.swap_free_bytes) as f64),
    );
    family.finish(out);

    let mut family = Family::new(
        "memory_usage_ratio",
        "used/total (0..1), com base em /proc/meminfo. EM CONTÊINER isso é a memória do \
         host físico, não o limite do namespace — a interface de meminfo não conhece \
         cgroups. O número do namespace está em godmesh_host_cgroup_memory_*. Ausente se \
         o kernel não expõe MemAvailable: não estimamos por heurística de cache, porque \
         um número inventado seria pior que nenhum.",
        "gauge",
    );
    family.one(memory.usage_ratio());
    family.finish(out);
}

fn render_pressure(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Pressure) || snapshot.pressure.is_empty() {
        return;
    }

    let mut family = Family::new(
        "pressure_stalled_ratio",
        "PSI (pressure stall information): porcentagem média (0..100) de tarefas \
         stalled por janela. scope=\"host\" é /proc/pressure (a visão deste namespace \
         de kernel); scope=\"cgroup\" é o arquivo de pressão do cgroup do processo, que \
         em contêiner é o número que diz respeito a este processo. kind=\"full\" só \
         existe em Linux >= 5.x. PSI ausente do kernel NÃO é pressão 0.",
        "gauge",
    );
    for pressure in &snapshot.pressure {
        for (kind, line) in [("some", pressure.some), ("full", pressure.full)] {
            let Some(line) = line else { continue };
            for (window, value) in [("10", line.avg10), ("60", line.avg60), ("300", line.avg300)] {
                family.sample(
                    Some(value),
                    &[
                        ("resource", pressure.resource),
                        ("kind", kind),
                        ("window", window),
                        ("scope", pressure.scope.as_str()),
                    ],
                );
            }
        }
    }
    family.finish(out);

    let mut family = Family::new(
        "pressure_stalled_seconds_total",
        "PSI: segundos acumulados desde o boot com tarefas stalled (campo total= dos \
         arquivos de pressão, que o kernel reporta em microssegundos).",
        "counter",
    );
    for pressure in &snapshot.pressure {
        for (kind, line) in [("some", pressure.some), ("full", pressure.full)] {
            let Some(line) = line else { continue };
            family.sample(
                Some(line.total_seconds()),
                &[
                    ("resource", pressure.resource),
                    ("kind", kind),
                    ("scope", pressure.scope.as_str()),
                ],
            );
        }
    }
    family.finish(out);
}

fn render_cgroup(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Cgroup) {
        return;
    }
    let Ok(cgroup) = &snapshot.cgroup else { return };

    let mut family = Family::new(
        "cgroup_memory_limit_bytes",
        "Limite de memória do CGROUP deste processo, em bytes. É o limite do namespace \
         (contêiner), NÃO a memória do hipervisor físico. Ausente = sem limite (o \
         literal \"max\" de v2, ou a sentinela LONG_MAX de v1) ou ilegível.",
        "gauge",
    );
    family.one(cgroup.memory_limit_bytes.map(|value| value as f64));
    family.finish(out);

    let mut family = Family::new(
        "cgroup_memory_current_bytes",
        "Memória usada agora pelo cgroup deste processo, em bytes (namespace, não a \
         máquina física). Em v1, memory.usage_in_bytes; em v2, memory.current.",
        "gauge",
    );
    family.one(cgroup.memory_current_bytes.map(|value| value as f64));
    family.finish(out);

    let mut family = Family::new(
        "cgroup_memory_usage_ratio",
        "current/limit do cgroup (0..1). Ausente sem limite: \"sem limite\" não é 0 \
         nem 1.",
        "gauge",
    );
    family.one(match (cgroup.memory_current_bytes, cgroup.memory_limit_bytes) {
        (Some(current), Some(limit)) if limit > 0 => Some(current as f64 / limit as f64),
        _ => None,
    });
    family.finish(out);

    let mut family = Family::new(
        "cgroup_cpu_limit_cores",
        "Quota de CPU do cgroup em núcleos (cpu.max = quota/período). Número do \
         namespace; compare com godmesh_host_cpu_cores, que é o que /proc mostra. \
         Em cgroup v1 esta fatia não lê a quota (fica em outra subtree, \
         /sys/fs/cgroup/cpu*, e o caminho por controller é outro caso) — a série \
         está ausente em v1, o que não é \"sem quota\".",
        "gauge",
    );
    family.one(cgroup.cpu_quota_cores);
    family.finish(out);

    let mut family = Family::new(
        "cgroup_cpu_throttled_seconds_total",
        "Segundos em que o cgroup foi throttled pela quota de CPU (throttled_usec de \
         cpu.stat / 1e6). É a evidência de que a quota limitou o processo, não a \
         máquina.",
        "counter",
    );
    family.one(
        cgroup
            .cpu_stat
            .get("throttled_usec")
            .map(|value| *value as f64 / 1e6),
    );
    family.finish(out);

    let mut family = Family::new(
        "cgroup_cpu_throttled_periods_total",
        "Períodos de quota em que o cgroup foi throttled (nr_throttled de cpu.stat), \
         adimensional.",
        "counter",
    );
    family.one(cgroup.cpu_stat.get("nr_throttled").map(|value| *value as f64));
    family.finish(out);

    let mut family = Family::new(
        "cgroup_version",
        "Versão da hierarquia de cgroups lida (2 = unified, 1 = v1). Ausente quando não \
         foi possível determinar: não adivinhamos o formato dos arquivos.",
        "gauge",
    );
    family.one(cgroup.version.map(f64::from));
    family.finish(out);
}

fn render_volumes(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Disk) {
        return;
    }

    let mut family = Family::new(
        "disk_bytes",
        "Espaço em bytes por volume montado, de statfs(2) (via sysinfo). \
         state=\"available\" é o espaço utilizável por não-privilegiados. Pseudo-fs \
         (proc, sysfs, tmpfs, cgroup2, squashfs, fuse*) são filtrados pelo TIPO de \
         filesystem declarado pelo kernel, não pelo ponto de montagem.",
        "gauge",
    );
    for volume in &snapshot.volumes {
        family.sample(
            Some(volume.total_bytes as f64),
            &[
                ("volume", volume.mount_point.as_str()),
                ("state", "total"),
            ],
        );
        family.sample(
            Some(volume.available_bytes as f64),
            &[
                ("volume", volume.mount_point.as_str()),
                ("state", "available"),
            ],
        );
        family.sample(
            Some(volume.total_bytes.saturating_sub(volume.available_bytes) as f64),
            &[
                ("volume", volume.mount_point.as_str()),
                ("state", "used"),
            ],
        );
    }
    family.finish(out);

    let mut family = Family::new(
        "disk_usage_ratio",
        "(total − available)/total por volume (0..1). INODES NÃO são publicados nesta \
         fatia: statfs devolve f_files/f_ffree, mas sysinfo 0.40.4 não expõe esse par e \
         /proc não tem substituto — a série está ausente de propósito, o que não é zero.",
        "gauge",
    );
    for volume in &snapshot.volumes {
        family.sample(volume.used_ratio(), &[("volume", volume.mount_point.as_str())]);
    }
    family.finish(out);

    let mut family = Family::new(
        "disk_free_ratio",
        "available/total por volume (0..1). É o número que aparece no resumo de \
         capacidade de /healthz, como pior caso entre os volumes.",
        "gauge",
    );
    for volume in &snapshot.volumes {
        family.sample(volume.free_ratio(), &[("volume", volume.mount_point.as_str())]);
    }
    family.finish(out);

    let mut family = Family::new(
        "disk_read_only",
        "1 quando o volume está montado read-only (statfs), 0 caso contrário.",
        "gauge",
    );
    for volume in &snapshot.volumes {
        family.sample(
            Some(f64::from(volume.read_only)),
            &[("volume", volume.mount_point.as_str())],
        );
    }
    family.finish(out);

    let mut family = Family::new(
        "disk_volumes",
        "Quantos volumes de dados este snapshot publica, depois do filtro de pseudo-fs \
         e do corte de cardinalidade.",
        "gauge",
    );
    family.one(Some(snapshot.volumes.len() as f64));
    family.finish(out);

    render_truncated(out, snapshot);
}

/// Marca visível de que a cardinalidade foi cortada de propósito.
fn render_truncated(out: &mut String, snapshot: &Snapshot) {
    let mut family = Family::new(
        "cardinality_truncated",
        "1 quando o teto de cardinalidade deste tipo cortou séries: o painel está \
         incompleto deliberadamente (ADR-073), não por falha de leitura.",
        "gauge",
    );
    family.labelled(
        "type",
        "volume",
        Some(f64::from(snapshot.volumes_truncated)),
    );
    family.labelled(
        "type",
        "interface",
        Some(f64::from(snapshot.interfaces_truncated)),
    );
    family.finish(out);
}

fn render_descriptors(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Descriptors) {
        return;
    }
    let Ok(descriptors) = &snapshot.descriptors else { return };

    let mut family = Family::new(
        "process_descriptors",
        "Descritores de arquivo DESTE PROCESSO, em files: state=\"open\" é a contagem \
         de /proc/self/fd; state=\"limit_soft\"/\"limit_hard\" vêm de Max open files em \
         /proc/self/limits. Limite \"unlimited\" não é publicado como número: não existe \
         número para ele, e um 0 ali leria \"limite estourado\" para quem não tem \
         limite.",
        "gauge",
    );
    family.labelled("state", "open", Some(descriptors.open as f64));
    family.labelled(
        "state",
        "limit_soft",
        descriptors.soft_limit.map(|value| value as f64),
    );
    family.labelled(
        "state",
        "limit_hard",
        descriptors.hard_limit.map(|value| value as f64),
    );
    family.finish(out);

    let mut family = Family::new(
        "process_descriptors_usage_ratio",
        "open/soft_limit deste processo (0..1). Ausente com limite ilimitado: não há \
         fração a calcular.",
        "gauge",
    );
    family.one(descriptors.usage_ratio());
    family.finish(out);
}

fn render_sockets(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Sockets) {
        return;
    }
    let Ok(sockets) = &snapshot.sockets else { return };

    let mut family = Family::new(
        "sockets",
        "Sockets em uso pelo kernel deste namespace de rede, em sockets \
         (/proc/net/sockstat). protocol=\"used\" é o total; os demais são a coluna \
         inuse por protocolo. Nunca há uma série por socket: a agregação por \
         protocolo é o teto.",
        "gauge",
    );
    family.sample(
        sockets.used.map(|value| value as f64),
        &[("protocol", "used")],
    );
    for (protocol, value) in &sockets.by_protocol {
        family.sample(Some(*value as f64), &[("protocol", protocol.as_str())]);
    }
    family.finish(out);

    let mut family = Family::new(
        "socket_memory_pages",
        "Memória dos protocolos de rede em PÁGINAS de kernel (coluna mem de \
         /proc/net/sockstat). NÃO é bytes: o kernel reporta páginas e não multiplicamos \
         por um page size chutado.",
        "gauge",
    );
    for (protocol, value) in &sockets.memory_pages {
        family.sample(Some(*value as f64), &[("protocol", protocol.as_str())]);
    }
    family.finish(out);
}

fn render_tcp(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Tcp) {
        return;
    }
    let Ok(states) = &snapshot.tcp else { return };

    let mut family = Family::new(
        "tcp_connections",
        "Conexões TCP agregadas por estado, em conexões (coluna st de /proc/net/tcp e \
         /proc/net/tcp6, IPv4+IPv6 somados). O vocabulário é fechado e indexado pelo \
         valor do kernel; um estado novo aparece como state=\"unknown\", nunca mapeado \
         num estado velho. Contagem por conexão, porta ou peer nunca é publicada.",
        "gauge",
    );
    for state in TCP_STATE_NAMES {
        family.sample(
            states.by_state.get(state).map(|value| *value as f64),
            &[("state", state)],
        );
    }
    family.finish(out);

    let mut family = Family::new(
        "tcp_connections_observed",
        "Soma das conexões dos arquivos lidos, em conexões — permite conferir se a \
         agregação por estado perdeu linhas.",
        "gauge",
    );
    family.one(Some(states.total() as f64));
    family.finish(out);
}

fn render_conntrack(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::Conntrack) {
        return;
    }
    let Ok(conntrack) = &snapshot.conntrack else { return };

    let mut family = Family::new(
        "conntrack_entries",
        "Entradas ativas de conntrack, em entradas, de \
         /proc/sys/net/netfilter/nf_conntrack_count (leitura por arquivo; netlink não é \
         usado nesta fatia). Em namespace de rede é a tabela daquele namespace.",
        "gauge",
    );
    family.one(Some(conntrack.count as f64));
    family.finish(out);

    let mut family = Family::new(
        "conntrack_limit",
        "nf_conntrack_max, em entradas. Ausente quando ilegível sem privilégio: isso é \
         declarado como requisito, não preenchido com 0.",
        "gauge",
    );
    family.one(conntrack.max.map(|value| value as f64));
    family.finish(out);

    let mut family = Family::new(
        "conntrack_usage_ratio",
        "entries/limit (0..1). Ausente com limit ilegível.",
        "gauge",
    );
    family.one(conntrack.usage_ratio());
    family.finish(out);
}

fn render_interfaces(out: &mut String, snapshot: &Snapshot) {
    if !snapshot.group_available(Group::NetDev) {
        return;
    }

    let mut bytes = Family::new(
        "network_interface_bytes_total",
        "Bytes acumulados por interface deste namespace de rede desde o boot do \
         namespace (/proc/net/dev), em bytes. direction=rx|tx.",
        "counter",
    );
    let mut packets = Family::new(
        "network_interface_packets_total",
        "Pacotes acumulados por interface desde o boot do namespace, em pacotes.",
        "counter",
    );
    let mut errors = Family::new(
        "network_interface_errors_total",
        "Erros de pilha acumulados por interface (coluna errs de /proc/net/dev), em \
         erros.",
        "counter",
    );
    let mut dropped = Family::new(
        "network_interface_dropped_total",
        "Pacotes descartados acumulados por interface (coluna drop de /proc/net/dev), \
         em pacotes.",
        "counter",
    );

    for interface in &snapshot.interfaces {
        for (family, rx, tx) in [
            (&mut bytes, interface.rx_bytes, interface.tx_bytes),
            (&mut packets, interface.rx_packets, interface.tx_packets),
            (&mut errors, interface.rx_errors, interface.tx_errors),
            (&mut dropped, interface.rx_dropped, interface.tx_dropped),
        ] {
            family.sample(
                Some(rx as f64),
                &[
                    ("interface", interface.name.as_str()),
                    ("direction", "rx"),
                ],
            );
            family.sample(
                Some(tx as f64),
                &[
                    ("interface", interface.name.as_str()),
                    ("direction", "tx"),
                ],
            );
        }
    }

    bytes.finish(out);
    packets.finish(out);
    errors.finish(out);
    dropped.finish(out);
}

fn render_collection(out: &mut String, snapshot: &Snapshot, now_millis: u64) {
    let mut family = Family::new(
        "collection_duration_seconds",
        "Custo da última coleta de host, em segundos de parede. É o orçamento \
         declarado: um snapshot por intervalo configurado, nunca uma leitura por \
         scrape.",
        "gauge",
    );
    family.one(Some(snapshot.collection_seconds));
    family.finish(out);

    let mut family = Family::new(
        "snapshot_age_seconds",
        "Idade do snapshot servido, em segundos. É como o scrape distingue \
         \"coletado agora\" de \"número velho\": o handler nunca lê /proc no caminho da \
         requisição.",
        "gauge",
    );
    family.one(Some(
        now_millis.saturating_sub(snapshot.taken_at_millis) as f64 / 1_000.0,
    ));
    family.finish(out);

    let mut family = Family::new(
        "container",
        "1 quando o cgroup próprio foi lido (namespace de contêiner): nesse caso os \
         números de cgroup são do namespace, não da máquina física.",
        "gauge",
    );
    family.one(Some(f64::from(snapshot.container)));
    family.finish(out);
}

/// Pressão `some avg10` de um recurso, preferindo o cgroup em contêiner — o
/// número do namespace é o que diz respeito a este processo.
#[must_use]
pub fn pressure_avg10(snapshot: &Snapshot, resource: &str) -> Option<f64> {
    let scope = if snapshot.container {
        PressureScope::Cgroup
    } else {
        PressureScope::Host
    };
    snapshot
        .pressure_for(resource, scope)
        .or_else(|| snapshot.pressure_for(resource, PressureScope::Host))
        .and_then(Pressure::some_avg10)
}

/// Pior `free_ratio` entre os volumes publicados (`None` sem volume legível).
#[must_use]
pub fn worst_free_ratio(volumes: &[Volume]) -> Option<f64> {
    volumes
        .iter()
        .filter_map(Volume::free_ratio)
        .min_by(f64::total_cmp)
}

