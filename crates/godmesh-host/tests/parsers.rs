//! Parsers de `/proc` contra fixtures, com valores conferíveis à mão.
//!
//! Cada asserção diz de onde veio o número esperado: a linha do fixture e a
//! conta. Isso é o que torna a revisão possível sem rodar num Linux.

use godmesh_host::proc;
use godmesh_host::snapshot::{CpuState, PressureScope};

mod fixtures;

#[test]
fn cpu_stat_converte_jiffies_e_conta_nucleos() {
    let text = fixtures::read("linux", "proc/stat");
    let times = proc::cpu_stat(&text).expect("parse de cpu");

    // `cpu  1000 200 300 5000 400 10 20 50 0 0` / 100 Hz:
    assert_eq!(times.seconds(CpuState::User), 10.0);
    assert_eq!(times.seconds(CpuState::Nice), 2.0);
    assert_eq!(times.seconds(CpuState::System), 3.0);
    assert_eq!(times.seconds(CpuState::Idle), 50.0);
    assert_eq!(times.seconds(CpuState::IoWait), 4.0);
    assert_eq!(times.seconds(CpuState::Irq), 0.1);
    assert_eq!(times.seconds(CpuState::SoftIrq), 0.2);
    assert_eq!(times.seconds(CpuState::Steal), 0.5);

    // Quatro linhas cpuN.
    assert_eq!(times.cores, 4);
    assert_eq!(times.hertz, proc::USER_HZ);

    // Total = soma das oito classes (1000+200+300+5000+400+10+20+50 = 6980 jiffies).
    assert_eq!(times.total_seconds(), 69.8);
    // guest/guest_nice (as duas últimas colunas, 0 0) não entram.
    assert_eq!(times.jiffies.len(), 8);
}

#[test]
fn cpu_stat_recusa_texto_sem_linha_cpu() {
    assert!(proc::cpu_stat("procs_running 1\n").is_none());
    assert!(proc::cpu_stat("").is_none());
    // Coluna não numérica é formato inesperado, não "0 jiffies".
    assert!(proc::cpu_stat("cpu  1000 abc 300 5000 400 10 20 50\n").is_none());
}

#[test]
fn cpu_stat_tolera_classe_nova_do_kernel() {
    // Kernel sem `steal` (colunas param depois de idle): as classes ausentes
    // valem 0 jiffies e o parse continua válido.
    let text = "cpu  100 0 100 1000\n";
    let times = proc::cpu_stat(text).expect("parse curto");
    assert_eq!(times.seconds(CpuState::Steal), 0.0);
    assert_eq!(times.jiffies.get(&CpuState::IoWait), Some(&0));
}

#[test]
fn loadavg_tem_cinco_campos_uteis() {
    let load = proc::loadavg("0.42 0.38 0.31 2/556 12345\n").expect("loadavg");
    assert_eq!(load.one, 0.42);
    assert_eq!(load.five, 0.38);
    assert_eq!(load.fifteen, 0.31);
    assert_eq!(load.running_threads, 2);
    assert_eq!(load.total_threads, 556);

    assert!(proc::loadavg("0.42 0.38\n").is_none());
    assert!(proc::loadavg("0.42 0.38 0.31 2556 12345\n").is_none());
}

#[test]
fn threads_do_processo_vem_de_self_status() {
    let text = fixtures::read("linux", "proc/self/status");
    assert_eq!(proc::status_threads(&text), Some(9));
    assert_eq!(proc::status_threads("Pid:\t1\n"), None);
}

#[test]
fn janela_de_cpu_e_a_fracao_do_delta_entre_duas_amostras() {
    let before = proc::cpu_stat("cpu  1000 200 300 5000 400 10 20 50 0 0\n").unwrap();
    let after = proc::cpu_stat("cpu  1100 200 350 5400 420 10 20 60 0 0\n").unwrap();
    let window = proc::cpu_window(&before, &after, 5.0).expect("janela");

    // deltas: user 100, nice 0, system 50, idle 400, io_wait 20, irq 0,
    // softirq 0, steal 10 → total 580, busy (user+nice+system+irq+softirq) 150
    assert!((window.busy_ratio - 150.0 / 580.0).abs() < 1e-12, "{}", window.busy_ratio);
    assert!((window.ratios[&CpuState::Idle] - 400.0 / 580.0).abs() < 1e-12);
    assert!((window.ratios[&CpuState::IoWait] - 20.0 / 580.0).abs() < 1e-12);
    assert!((window.ratios[&CpuState::Steal] - 10.0 / 580.0).abs() < 1e-12);
    assert_eq!(window.window_seconds, 5.0);

    // As frações somam 1 (o denominador é comum), o que é a prova de que
    // nenhuma coluna foi perdida nem duplicada.
    let soma: f64 = window.ratios.values().sum();
    assert!((soma - 1.0).abs() < 1e-9, "{soma}");
}

#[test]
fn janela_de_cpu_exige_duas_amostras_crescentes() {
    let same = proc::cpu_stat("cpu  1000 200 300 5000 400 10 20 50 0 0\n").unwrap();
    // Mesma amostra: janela de 0 delta → None, não taxa 0.
    assert!(proc::cpu_window(&same, &same, 5.0).is_none());
    // Contador que recuou (troca de kernel): a janela não é confiável.
    let before = proc::cpu_stat("cpu  2000 200 300 5000 400 10 20 50 0 0\n").unwrap();
    assert!(proc::cpu_window(&before, &same, 5.0).is_none());
    // Janela de duração zero não é taxa.
    assert!(proc::cpu_window(&same, &before, 0.0).is_none());
}

#[test]
fn meminfo_converte_kb_para_bytes_e_deriva_usado() {
    let text = fixtures::read("linux", "proc/meminfo");
    let memory = proc::meminfo(&text).expect("meminfo");

    // MemTotal 16384000 kB → 16384000 * 1024
    assert_eq!(memory.total_bytes, 16_777_216_000);
    assert_eq!(memory.free_bytes, 4_194_304_000);
    assert_eq!(memory.available_bytes, Some(10_485_760_000));
    assert_eq!(memory.swap_total_bytes, 4_194_304_000);
    assert_eq!(memory.swap_free_bytes, 3_145_728_000);

    // usado = total − available = 6_291_456_000 (6000000 kB), exatamente 37,5%
    assert_eq!(memory.used_bytes(), Some(6_291_456_000));
    assert!((memory.usage_ratio().unwrap() - 0.375).abs() < 1e-12);
    // swap usado = 4096000 − 3072000 kB = 1024000 kB
    assert!(
        (memory.swap_used_ratio().unwrap() - (1_024_000.0 / 4_096_000.0)).abs() < 1e-12
    );
}

#[test]
fn meminfo_sem_memavailable_nao_inventa_usado() {
    let text = "MemTotal: 1000 kB\nMemFree: 400 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n";
    let memory = proc::meminfo(text).expect("meminfo antigo");
    assert_eq!(memory.available_bytes, None);
    assert_eq!(memory.used_bytes(), None);
    assert_eq!(memory.usage_ratio(), None);
    assert_eq!(memory.swap_used_ratio(), None, "swap de 0 bytes não tem fração");
}

#[test]
fn meminfo_sem_memtotal_e_formato_inesperado() {
    assert!(proc::meminfo("MemFree: 1 kB\n").is_err());
    assert!(proc::meminfo("").is_err());
}

#[test]
fn psi_le_some_e_full_de_um_arquivo_real() {
    let text = fixtures::read("linux", "proc/pressure/memory");
    let pressure = proc::pressure("memory", PressureScope::Host, &text).expect("psi");
    let some = pressure.some.expect("linha some");
    assert_eq!(some.avg10, 6.0);
    assert_eq!(some.avg60, 4.0);
    assert_eq!(some.avg300, 2.0);
    // total= em µs → segundos
    assert_eq!(some.total_microseconds, 8_000_000);
    assert_eq!(some.total_seconds(), 8.0);
    let full = pressure.full.expect("linha full");
    assert_eq!(full.avg10, 3.0);
    assert_eq!(pressure.some_avg10(), Some(6.0));
}

#[test]
fn psi_sem_full_em_kernel_antigo_continuma_leitura() {
    let pressure = proc::pressure(
        "io",
        PressureScope::Cgroup,
        "some avg10=0.10 avg60=0.20 avg300=0.30 total=1000\n",
    )
    .expect("psi antigo");
    assert!(pressure.full.is_none());
    assert_eq!(pressure.some.map(|l| l.avg10), Some(0.1));
}

#[test]
fn psi_sem_nenhuma_linha_valida_nao_e_pressao_zero() {
    assert_eq!(
        proc::pressure("cpu", PressureScope::Host, "foo avg10=1.0\n").err(),
        Some(godmesh_host::snapshot::Unavailable::Malformed)
    );
    assert_eq!(
        proc::pressure("cpu", PressureScope::Host, "").err(),
        Some(godmesh_host::snapshot::Unavailable::Malformed)
    );
}

#[test]
fn psi_de_io_sem_linha_full_vem_do_fixture_container() {
    let text = fixtures::read("container", "proc/pressure/io");
    let pressure = proc::pressure("io", PressureScope::Host, &text).expect("psi io");
    assert!(pressure.full.is_none());
    assert_eq!(pressure.some.map(|l| l.avg10), Some(2.0));
}

#[test]
fn cgroup_escalar_distingue_numero_de_max() {
    assert_eq!(proc::cgroup_limit("1073741824\n"), Ok(Some(1_073_741_824)));
    assert_eq!(proc::cgroup_limit("max\n"), Ok(None), "\"max\" é sem limite");
    assert_eq!(proc::cgroup_limit(""), Err(godmesh_host::snapshot::Unavailable::Malformed));
    assert_eq!(proc::cgroup_limit("abc"), Err(godmesh_host::snapshot::Unavailable::Malformed));
}

#[test]
fn cgroup_cpu_max_vira_nucleos() {
    // 200000 µs de quota por período de 100000 µs = 2 núcleos.
    assert_eq!(proc::cgroup_cpu_quota("200000 100000\n"), Ok(Some(2.0)));
    assert_eq!(proc::cgroup_cpu_quota("150000 100000\n"), Ok(Some(1.5)));
    assert_eq!(proc::cgroup_cpu_quota("max 100000\n"), Ok(None));
    assert!(proc::cgroup_cpu_quota("200000 0\n").is_err());
    assert!(proc::cgroup_cpu_quota("200000\n").is_err());
}

#[test]
fn cgroup_cpu_stat_preserva_chaves_do_kernel() {
    let pairs = proc::cpu_stat_pairs("nr_periods 1000\nnr_throttled 25\nthrottled_usec 5000000\nburst_usec 0\n");
    assert_eq!(pairs.get("nr_periods"), Some(&1000));
    assert_eq!(pairs.get("nr_throttled"), Some(&25));
    assert_eq!(pairs.get("throttled_usec"), Some(&5_000_000));
    // chave nova de kernel não derruba o parse
    assert_eq!(pairs.get("burst_usec"), Some(&0));
}

#[test]
fn limites_de_arquivo_usam_as_duas_ultimas_colunas() {
    let text = fixtures::read("linux", "proc/self/limits");
    assert_eq!(proc::max_open_files(&text), Some((Some(1_048_576), Some(1_048_576))));

    // soft numérico, hard ilimitado: `unlimited` não tem número.
    let mixed = "Max stack size            8388608              unlimited            bytes\nMax open files            1024                 unlimited            files\n";
    assert_eq!(proc::max_open_files(mixed), Some((Some(1024), None)));

    assert_eq!(proc::max_open_files("Max processes 1 1 processes\n"), None);
}

#[test]
fn net_dev_separa_rx_e_tx_pelas_colunas_certas() {
    let text = fixtures::read("linux", "proc/net/dev");
    let interfaces = proc::net_dev(&text);
    assert_eq!(interfaces.len(), 3, "lo, eth0, wlan0");

    let eth0 = interfaces.iter().find(|i| i.name == "eth0").expect("eth0");
    // `eth0: 5000000 40000 2 3 0 0 0 10 | 7000000 50000 4 5 0 0 0 0`
    assert_eq!(eth0.rx_bytes, 5_000_000);
    assert_eq!(eth0.rx_packets, 40_000);
    assert_eq!(eth0.rx_errors, 2);
    assert_eq!(eth0.rx_dropped, 3);
    assert_eq!(eth0.tx_bytes, 7_000_000);
    assert_eq!(eth0.tx_packets, 50_000);
    assert_eq!(eth0.tx_errors, 4);
    assert_eq!(eth0.tx_dropped, 5);

    // ordem por nome: a renderização é determinística
    let nomes: Vec<&str> = interfaces.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(nomes, ["eth0", "lo", "wlan0"]);

    // cabeçalho sem `:` não vira interface
    assert_eq!(proc::net_dev("Inter-| Receive | Transmit\n face |bytes\n").len(), 0);
}

#[test]
fn tcp_agrega_por_estado_e_soma_ipv4_com_ipv6() {
    let mut states = godmesh_host::snapshot::TcpStates::default();
    proc::tcp_states(&fixtures::read("linux", "proc/net/tcp"), &mut states).expect("tcp");
    proc::tcp_states(&fixtures::read("linux", "proc/net/tcp6"), &mut states).expect("tcp6");

    // tcp: 0A ×2 (listen), 01 ×1, 06 ×1, 08 ×1; tcp6: 0A ×1, 01 ×1
    assert_eq!(states.by_state.get("listen"), Some(&3));
    assert_eq!(states.by_state.get("established"), Some(&2));
    assert_eq!(states.by_state.get("time_wait"), Some(&1));
    assert_eq!(states.by_state.get("close_wait"), Some(&1));
    assert_eq!(states.total(), 7);

    // estados que não existem naquele instante não ganham série zero
    assert!(!states.by_state.contains_key("syn_sent"));
}

#[test]
fn tcp_de_estado_desconhecido_vira_unknown_nao_um_estado_velho() {
    let mut states = godmesh_host::snapshot::TcpStates::default();
    let texto = "  sl local rem st resto\n   0: 1:2 3:4 FF 5\n";
    proc::tcp_states(texto, &mut states).expect("parse");
    assert_eq!(states.by_state.get("unknown"), Some(&1));
}

#[test]
fn sockstat_le_inuse_mem_e_total() {
    let sockets = proc::sockstat(&fixtures::read("linux", "proc/net/sockstat"));
    assert_eq!(sockets.used, Some(512));
    assert_eq!(sockets.by_protocol.get("tcp"), Some(&17));
    assert_eq!(sockets.by_protocol.get("udp"), Some(&4));
    assert_eq!(sockets.by_protocol.get("frag"), Some(&0), "0 é medida, não ausência");
    assert_eq!(sockets.memory_pages.get("tcp"), Some(&3));
    assert_eq!(sockets.memory_pages.get("udp"), Some(&2));
    assert!(!sockets.memory_pages.contains_key("frag"), "coluna `memory` não é `mem`");
}

#[test]
fn conntrack_e_um_inteiro_por_arquivo() {
    assert_eq!(proc::conntrack_value("1234\n"), Ok(1234));
    assert!(proc::conntrack_value("max").is_err());
}

#[test]
fn cgroup_path_identifica_namespace_de_container() {
    let unificado = proc::cgroup_path("0::/kubepods.slice/pod123/container456\n");
    assert_eq!(unificado.as_deref(), Some("kubepods.slice/pod123/container456"));

    // host comum: caminho root não afirma contêiner nenhum
    assert_eq!(proc::cgroup_path("0::/\n"), None);

    // v1 híbrida: usa o caminho de uma controller
    let v1 = proc::cgroup_path(
        "12:memory:/system.slice/godmesh.service\n11:cpu,cpuacct:/system.slice/godmesh.service\n",
    );
    assert_eq!(v1.as_deref(), Some("system.slice/godmesh.service"));
}

#[test]
fn caminho_v1_usa_a_controller_de_memoria_e_nao_a_primeira_que_aparecer() {
    let texto = "10:cpuset:/kubepods/cpuset/pod123\n\
                 8:memory:/kubepods/burstable/pod123/container456\n\
                 7:cpu,cpuacct:/kubepods/cpu/pod123\n";
    assert_eq!(
        proc::cgroup_memory_path(texto).as_deref(),
        Some("kubepods/burstable/pod123/container456")
    );
    // sem linha de memória: None, e o coletor não publica limite de cgroup
    assert_eq!(proc::cgroup_memory_path("7:cpu,cpuacct:/x\n"), None);
    // caminho root não afirma contêiner
    assert_eq!(proc::cgroup_memory_path("8:memory:/\n"), None);
}

#[test]
fn mount_identifica_hierarquia_e_ponto() {
    // Formato de `/proc/mounts`: device mountpoint fstype …
    let mounts = fixtures::read("linux", "proc/self/mounts");
    assert_eq!(
        proc::cgroup_mount(&mounts),
        Some(("/sys/fs/cgroup".to_owned(), 2))
    );

    let v1 = "cgroup /sys/fs/cgroup/systemd cgroup rw,xattr 0 0\n\
              cgroup /sys/fs/cgroup/memory cgroup rw,memory 0 0\n\
              cgroup /sys/fs/cgroup/cpu,cpuacct cgroup rw,cpu,cpuacct 0 0\n";
    assert_eq!(
        proc::cgroup_mount(v1),
        Some(("/sys/fs/cgroup/memory".to_owned(), 1)),
        "em v1 a subtree de memória é a que tem os limites que publicamos"
    );

    // mountinfo NÃO é este formato: o tipo vem depois de " - ". Aceitá-lo em
    // silêncio leria "sem cgroup" onde existe um — None é o piso honesto.
    let mountinfo = fixtures::read("linux", "proc/self/mountinfo");
    assert_eq!(proc::cgroup_mount(&mountinfo), None);

    assert_eq!(proc::cgroup_mount("proc /proc proc rw 0 0\n"), None);
}
