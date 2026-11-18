//! Parsers puros do formato de `/proc` e dos arquivos de cgroup.
//!
//! Cada função recebe `&str` e devolve `Option`/`Result` com **motivo**. Nada
//! aqui toca disco: é o que permite cobrir o formato com fixtures sem depender
//! do host do CI, e é também o que permite comparar o parse com a saída de
//! `top`/`free`/`ss` linha a linha numa evidência manual.
//!
//! Convenção de erro que atravesta o módulo:
//!  * `Malformed` quando o arquivo existe mas não é o formato esperado — a
//!    leitura inteira daquele grupo é declarada indisponível, em vez de
//!    preencher os campos que faltam com zero.
//!  * campo opcional ausente vira `None` **e é documentado** (MemAvailable em
//!    kernel < 3.14, linha `full:` em PSI < 5.x, controller que não existe).

use std::collections::BTreeMap;

use crate::snapshot::{
    CpuState, CpuTimes, CpuWindow, Interface, LoadAverage, Memory, Pressure, PressureLine,
    PressureScope, Sockets, TcpStates, Unavailable, CPU_STATE_ORDER, TCP_STATE_NAMES,
};

/// `USER_HZ`: a interface de `/proc/stat` é definida em jiffies de 1/100 s
/// (procps), e é o que `top` usa. Não é lido de lugar nenhum nem é parâmetro.
pub const USER_HZ: u64 = 100;

/// Arquivos de pressão que um kernel com `CONFIG_PSI` expõe.
pub const PRESSURE_RESOURCES: [&str; 3] = ["cpu", "memory", "io"];

// --- CPU -------------------------------------------------------------------

/// Linha `cpu` de `/proc/stat` + contagem de núcleos lógicos.
///
/// As colunas `guest` e `guest_nice` (9ª e 10ª) são **ignoradas**: elas já
/// estão contadas dentro de `user`, e somá-las de novo produziria busy > 1.
#[must_use]
pub fn cpu_stat(text: &str) -> Option<CpuTimes> {
    let mut jiffies = BTreeMap::new();
    let mut cores = 0_u32;

    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else { continue };
        if label == "cpu" {
            for (index, value) in fields.enumerate() {
                let Some(state) = CpuState::column(index) else {
                    break;
                };
                let Ok(jiffies_value) = value.parse::<u64>() else {
                    return None;
                };
                jiffies.insert(state, jiffies_value);
            }
        } else if is_cpu_line(label) {
            cores = cores.saturating_add(1);
        }
    }

    if jiffies.is_empty() {
        return None;
    }
    // Classe ausente (kernel sem `steal`, por exemplo) vale 0 jiffies: aqui
    // "ausente" é "não acumulou tempo", não "desconhecido".
    for state in CPU_STATE_ORDER {
        jiffies.entry(state).or_insert(0);
    }

    Some(CpuTimes {
        jiffies,
        hertz: USER_HZ,
        cores,
    })
}

fn is_cpu_line(label: &str) -> bool {
    let Some(suffix) = label.strip_prefix("cpu") else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

/// `loadavg`: `0.42 0.38 0.31 2/556 12345`.
#[must_use]
pub fn loadavg(text: &str) -> Option<LoadAverage> {
    let mut fields = text.split_whitespace();
    Some(LoadAverage {
        one: fields.next()?.parse().ok()?,
        five: fields.next()?.parse().ok()?,
        fifteen: fields.next()?.parse().ok()?,
        running_threads: 0,
        total_threads: 0,
    })
    .and_then(|mut load| {
        let threads = fields.next()?;
        let (running, total) = threads.split_once('/')?;
        load.running_threads = running.parse().ok()?;
        load.total_threads = total.parse().ok()?;
        Some(load)
    })
}

/// `Threads:` de `/proc/self/status`.
#[must_use]
pub fn status_threads(text: &str) -> Option<u64> {
    field_after_key(text, "Threads:")
}

/// Fração de uso entre duas amostras.
///
/// Taxa é derivada: exige dois instantes. A primeira coleta do processo devolve
/// `None`, e um recuo de contador (troca de kernel, wrap) também — uma série
/// ausente por um ciclo é melhor do que uma taxa falsa.
#[must_use]
pub fn cpu_window(previous: &CpuTimes, current: &CpuTimes, window_seconds: f64) -> Option<CpuWindow> {
    if window_seconds <= 0.0 || previous.hertz != current.hertz {
        return None;
    }

    let mut deltas = BTreeMap::new();
    let mut total = 0_u64;
    let mut busy = 0_u64;
    for state in CPU_STATE_ORDER {
        let before = *previous.jiffies.get(&state).unwrap_or(&0);
        let after = *current.jiffies.get(&state).unwrap_or(&0);
        let delta = after.checked_sub(before)?;
        total = total.saturating_add(delta);
        if state.is_busy() {
            busy = busy.saturating_add(delta);
        }
        deltas.insert(state, delta);
    }
    if total == 0 {
        return None;
    }

    Some(CpuWindow {
        window_seconds,
        busy_ratio: busy as f64 / total as f64,
        ratios: deltas
            .into_iter()
            .map(|(state, delta)| (state, delta as f64 / total as f64))
            .collect(),
    })
}

// --- Memória ---------------------------------------------------------------

/// `/proc/meminfo`: os campos usados, convertidos de kB para bytes.
///
/// `MemAvailable` é opcional (kernel < 3.14) e vira `None`; os demais são
/// obrigatórios para o grupo existir.
pub fn meminfo(text: &str) -> Result<Memory, Unavailable> {
    let value = |key: &str| -> Option<u64> {
        field_after_key(text, key).and_then(|kib| kib.checked_mul(1024))
    };

    Ok(Memory {
        total_bytes: value("MemTotal:").ok_or(Unavailable::Malformed)?,
        free_bytes: value("MemFree:").ok_or(Unavailable::Malformed)?,
        available_bytes: value("MemAvailable:"),
        swap_total_bytes: value("SwapTotal:").unwrap_or(0),
        swap_free_bytes: value("SwapFree:").unwrap_or(0),
    })
}

// --- Pressão (PSI) ---------------------------------------------------------

/// `some avg10=0.00 avg60=0.00 avg300=0.00 total=0`, mais `full=` em 5.x+.
///
/// Nem `some` nem `full` legíveis → `Malformed`: um arquivo de pressão que não
/// tem nenhuma das duas linhas não é uma fonte de nada, e "pressão zero" é uma
/// afirmação que ele não autoriza.
pub fn pressure(resource: &'static str, scope: PressureScope, text: &str) -> Result<Pressure, Unavailable> {
    let some = pressure_line(text, "some");
    let full = pressure_line(text, "full");
    match (some, full) {
        (Some(some), full) => Ok(Pressure {
            resource,
            scope,
            some: Some(some),
            full,
        }),
        (None, Some(full)) => Ok(Pressure {
            resource,
            scope,
            some: None,
            full: Some(full),
        }),
        (None, None) => Err(Unavailable::Malformed),
    }
}

fn pressure_line(text: &str, kind: &str) -> Option<PressureLine> {
    let line = text
        .lines()
        .map(str::trim_start)
        .find(|line| line.starts_with(kind))?;
    let mut avg10 = None;
    let mut avg60 = None;
    let mut avg300 = None;
    let mut total = None;
    for field in line.split_whitespace().skip(1) {
        let (key, value) = field.split_once('=')?;
        match key {
            "avg10" => avg10 = Some(value.parse::<f64>().ok()?),
            "avg60" => avg60 = Some(value.parse::<f64>().ok()?),
            "avg300" => avg300 = Some(value.parse::<f64>().ok()?),
            "total" => total = Some(value.parse::<u64>().ok()?),
            // Coluna nova de kernel: ignorar é compatibilidade.
            _ => {}
        }
    }
    Some(PressureLine {
        avg10: avg10?,
        avg60: avg60?,
        avg300: avg300?,
        total_microseconds: total?,
    })
}

// --- cgroup ----------------------------------------------------------------

/// Escalar de cgroup v2: um número, ou `max`.
///
/// `max` devolve `Ok(None)` — "sem limite" é uma informação, não uma falha.
pub fn cgroup_limit(text: &str) -> Result<Option<u64>, Unavailable> {
    match text.trim() {
        "" => Err(Unavailable::Malformed),
        "max" => Ok(None),
        token => token
            .parse::<u64>()
            .map(Some)
            .map_err(|_| Unavailable::Malformed),
    }
}

/// Limite de memória em **cgroup v1**.
///
/// `memory.limit_in_bytes` não usa o literal `max` de v2: "sem limite" é uma
/// sentinela numérica — `LONG_MAX` arredondado a página,
/// `9223372036854771712` em 64 bits (e valores próximos de `u64::MAX` em
/// kernels de 32 bits). Interpretar a sentinela como limite publicaria um
/// "limite" de 8 exbibytes e uma `usage_ratio` próxima de zero num host com
/// swap ligado. Acima de `V1_SEM_LIMITE` a resposta é `None`.
pub fn cgroup_limit_v1(text: &str) -> Result<Option<u64>, Unavailable> {
    let valor = cgroup_limit(text)?;
    Ok(valor.filter(|valor| *valor < V1_SEM_LIMITE))
}

/// Sentinela de "sem limite" do cgroup v1, com folga de 16 MiB para a
/// aritmética de páginas dos diferentes kernels.
pub const V1_SEM_LIMITE: u64 = 9_223_372_036_854_771_712 - 16 * 1024 * 1024;

/// `cpu.max` = `"<quota> <period>"` ou `"max <period>"`; devolve núcleos.
pub fn cgroup_cpu_quota(text: &str) -> Result<Option<f64>, Unavailable> {
    let mut fields = text.split_whitespace();
    let Some(quota) = fields.next() else {
        return Err(Unavailable::Malformed);
    };
    if quota == "max" {
        return Ok(None);
    }
    let Some(period) = fields.next() else {
        return Err(Unavailable::Malformed);
    };
    let period: f64 = period.parse().map_err(|_| Unavailable::Malformed)?;
    if period <= 0.0 {
        return Err(Unavailable::Malformed);
    }
    let quota: f64 = quota.parse().map_err(|_| Unavailable::Malformed)?;
    Ok(Some(quota / period))
}

/// `cpu.stat`, `chave valor` por linha; chaves desconhecidas ficam (são a
/// evolução normal da interface) e nada é inventado.
#[must_use]
pub fn cpu_stat_pairs(text: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(key), Some(value)) = (fields.next(), fields.next())
            && let Ok(value) = value.parse::<u64>()
        {
            out.insert(key.to_owned(), value);
        }
    }
    out
}

/// Caminho do cgroup **de memória** deste processo, para cgroup v1.
///
/// Em v1 cada controller tem seu próprio caminho e sua própria montagem; a
/// subtree que publicamos (`memory.current`, `memory.max`) é a de memória, e é
/// o caminho dela que temos que usar. Sem linha de memória, devolve `None` —
/// não caímos em "o primeiro que aparecer", que leria o limite de outra
/// hierarquia.
#[must_use]
pub fn cgroup_memory_path(text: &str) -> Option<String> {
    for line in text.lines() {
        let mut fields = line.splitn(3, ':');
        let (Some(_), Some(controllers), Some(path)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if controllers.split(',').any(|c| c == "memory") && path != "/" && !path.is_empty() {
            return Some(path.trim_start_matches('/').to_owned());
        }
    }
    None
}

/// Caminho do cgroup deste processo em `/proc/self/cgroup`.
///
/// v2 tem a linha `0::/<caminho>`; v1 tem várias `hierarchy:controllers:/caminho`.
/// Na ausência da linha unificada usamos o caminho de uma controller não-root.
#[must_use]
pub fn cgroup_path(text: &str) -> Option<String> {
    for line in text.lines() {
        let mut fields = line.splitn(3, ':');
        let (Some(_), Some(controllers), Some(path)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if controllers.is_empty() && path != "/" {
            return Some(path.trim_start_matches('/').to_owned());
        }
    }
    text.lines().find_map(|line| {
        let path = line.splitn(3, ':').nth(2)?;
        (path != "/" && !path.is_empty()).then(|| path.trim_start_matches('/').to_owned())
    })
}

/// Ponto de montagem e versão da hierarquia de cgroups.
///
/// Formato esperado: `/proc/mounts` / `/proc/self/mounts` —
/// `device mountpoint fstype opts dump pass`. **Não** é `mountinfo`, onde o
/// tipo vem depois do ` - `; alimentar `mountinfo` aqui devolve `None`, que é
/// o piso honesto: sem ponto de montagem, nenhum número de cgroup é afirmado.
#[must_use]
pub fn cgroup_mount(mounts: &str) -> Option<(String, u8)> {
    let mut v1 = None;
    for line in mounts.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }
        match fields[2] {
            "cgroup2" => return Some((fields[1].to_owned(), 2)),
            "cgroup" => {
                // v1 híbrida: a subtree que nos interessa é a de memória.
                let interesting = fields[1].ends_with("/memory")
                    || fields[1].ends_with("/cpu")
                    || fields[1].ends_with("/cpuacct");
                if interesting || v1.is_none() {
                    v1 = Some((fields[1].to_owned(), 1));
                }
            }
            _ => {}
        }
    }
    v1
}

// --- Descritores -----------------------------------------------------------

/// `Max open files` de `/proc/self/limits`: `(soft, hard)`, com `unlimited`
/// como `None` — limite ilimitado não é um número para se dividir.
#[must_use]
pub fn max_open_files(text: &str) -> Option<(Option<u64>, Option<u64>)> {
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("Max open files"))?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let soft = fields.len().checked_sub(3).and_then(|i| fields.get(i))?;
    let hard = fields.len().checked_sub(2).and_then(|i| fields.get(i))?;
    Some((limit_value(soft), limit_value(hard)))
}

fn limit_value(token: &str) -> Option<u64> {
    if token.eq_ignore_ascii_case("unlimited") {
        None
    } else {
        token.parse().ok()
    }
}

// --- Rede ------------------------------------------------------------------

/// `/proc/net/dev`: uma linha por interface; as duas linhas de cabeçalho não
/// têm `:` e são descartadas pelo próprio formato.
///
/// Colunas: `rx bytes packets errs drop fifo frame compressed multicast`
/// (0..8) e `tx bytes packets errs drop fifo colls carrier compressed`
/// (8..16). Interfaces de um contêiner são as do *namespace* de rede.
#[must_use]
pub fn net_dev(text: &str) -> Vec<Interface> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 16 {
            continue;
        }
        let number = |index: usize| fields[index].parse::<u64>().unwrap_or(0);
        out.push(Interface {
            name: name.trim().to_owned(),
            rx_bytes: number(0),
            rx_packets: number(1),
            rx_errors: number(2),
            rx_dropped: number(3),
            tx_bytes: number(8),
            tx_packets: number(9),
            tx_errors: number(10),
            tx_dropped: number(11),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// `slabinfo`-like: soma os estados TCP de um `/proc/net/tcp{,6}` em `into`.
///
/// A coluna `st` é hex (`01` established, `06` time_wait, `0A` listen). Linha
/// sem a coluna é descartada: o arquivo pode ganhar colunas entre kernels e o
/// que não sabemos ler não vira contagem zero de um estado qualquer.
pub fn tcp_states(text: &str, into: &mut TcpStates) -> Result<(), Unavailable> {
    let mut counted = 0_usize;
    // Colunas: sl(0) local_address(1) rem_address(2) st(3) tx_queue:rx_queue(4)…
    for line in text.lines().skip(1) {
        let Some(state) = line.split_whitespace().nth(3) else {
            continue;
        };
        *into.by_state.entry(tcp_state_name(state)).or_insert(0) += 1;
        counted += 1;
    }
    if counted == 0 && text.lines().count() > 1 {
        return Err(Unavailable::Malformed);
    }
    Ok(())
}

/// Nome estável do estado TCP a partir do hex da coluna `st`.
///
/// Valor fora da tabela (ou não numérico) é `unknown`: um estado novo do kernel
/// aparece como `unknown`, e nunca como um estado velho com o número errado.
#[must_use]
pub fn tcp_state_name(hex: &str) -> &'static str {
    match u32::from_str_radix(hex, 16) {
        Ok(value) => match usize::try_from(value) {
            Ok(index) if index < TCP_STATE_NAMES.len() => TCP_STATE_NAMES[index],
            _ => crate::snapshot::TCP_STATE_UNKNOWN,
        },
        Err(_) => crate::snapshot::TCP_STATE_UNKNOWN,
    }
}

/// `/proc/net/sockstat`: `sockets: used N`, `TCP: inuse N orphan N tw N alloc N mem N`.
#[must_use]
pub fn sockstat(text: &str) -> Sockets {
    let mut out = Sockets::default();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else { continue };
        if let Some("sockets") = label.strip_suffix(':') {
            let _ = fields.next(); // "used"
            out.used = fields.next().and_then(|value| value.parse().ok());
            continue;
        }
        let protocol = label.strip_suffix(':').unwrap_or(label).to_ascii_lowercase();
        let values: Vec<&str> = fields.collect();
        let mut index = 0;
        while index + 1 < values.len() {
            match values[index] {
                "inuse" => {
                    if let Ok(value) = values[index + 1].parse::<u64>() {
                        out.by_protocol.insert(protocol.clone(), value);
                    }
                }
                "mem" => {
                    if let Ok(value) = values[index + 1].parse::<u64>() {
                        out.memory_pages.insert(protocol.clone(), value);
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }
    out
}

/// `nf_conntrack_count` / `nf_conntrack_max`: um inteiro por arquivo.
pub fn conntrack_value(text: &str) -> Result<u64, Unavailable> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| Unavailable::Malformed)
}

// --- utilidade -------------------------------------------------------------

/// Valor da primeira coluna numérica depois de `chave:`.
fn field_after_key(text: &str, key: &str) -> Option<u64> {
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with(key))?;
    line.split_whitespace().nth(1)?.parse().ok()
}
