//! Coleta completa contra fixtures de `/proc`.
//!
//! Estes testes exercitam o **mesmo** `Collector` da produção, apenas com a
//! raiz apontando para uma árvore de fixture. É isso que autoriza dizer que a
//! evidência cobre o coletor e não só os parsers.

use godmesh_host::proc;
use godmesh_host::snapshot::{
    Group, GroupStatus, PressureScope, Source, Unavailable,
};

mod fixtures;

#[test]
fn fixture_linux_produz_todos_os_grupos_legiveis() {
    let snapshot = fixtures::collector("linux").collect();

    assert_eq!(snapshot.source, Source::Proc);
    assert!(!snapshot.container, "cgroup path root não afirma contêiner");

    for group in Group::ALL {
        if group == Group::Cgroup {
            assert_eq!(
                snapshot.groups.get(&group),
                Some(&GroupStatus::Unavailable),
                "sem namespace de contêiner não há cgroup próprio a afirmar"
            );
            continue;
        }
        assert!(
            snapshot.group_available(group),
            "grupo {group:?} deveria estar disponível: {:?}",
            snapshot.groups
        );
    }
}

#[test]
fn cpu_memoria_e_carga_conferem_com_as_contas_do_fixture() {
    let snapshot = fixtures::collector("linux").collect();

    let times = snapshot.cpu_times.as_ref().expect("cpu");
    // `cpu  1000 200 300 5000 400 10 20 50` / 100 Hz
    assert_eq!(times.seconds(godmesh_host::snapshot::CpuState::User), 10.0);
    assert_eq!(times.cores, 4);
    assert_eq!(times.total_seconds(), 69.8);

    // Primeira coleta: não existe janela, e por isso a série não sai.
    assert!(
        snapshot.cpu_window.is_none(),
        "taxa exige duas amostras; primeira coleta não tem janela"
    );

    let memory = snapshot.memory.as_ref().expect("meminfo");
    assert_eq!(memory.total_bytes, 16_777_216_000);
    assert_eq!(memory.usage_ratio(), Some(0.375));

    let load = snapshot.load.as_ref().expect("loadavg");
    assert_eq!(load.one, 0.42);
    assert_eq!(load.total_threads, 556);

    assert_eq!(snapshot.process_threads, Ok(9));
}

#[test]
fn segunda_coleta_deriva_a_janela_de_uso() {
    let temp = fixtures::TempFixture::new("linux");
    let collector = fixtures::collector_at(temp.root());

    let primeiro = collector.collect();
    assert!(primeiro.cpu_window.is_none());

    // Mesmo formato, contadores maiores: user +100, system +50, idle +400,
    // iowait +20, steal +10 → total 580, busy 150.
    temp.write("proc/stat", "cpu  1100 200 350 5400 420 10 20 60 0 0\ncpu0 1 0 0 0 0 0 0 0 0 0\n");
    let segundo = collector.collect();

    let window = segundo.cpu_window.expect("segunda coleta tem janela");
    assert!((window.busy_ratio - 150.0 / 580.0).abs() < 1e-9, "{}", window.busy_ratio);
    assert!(window.window_seconds > 0.0, "janela medida em relógio monotônico");
    assert_eq!(window.window_seconds, window.window_seconds, "sem NaN");
    let soma: f64 = window.ratios.values().sum();
    assert!((soma - 1.0).abs() < 1e-9);
}

#[test]
fn pressao_tem_escopo_e_recurso_e_o_arquivo_nao_lido_nao_sai() {
    let snapshot = fixtures::collector("linux").collect();
    assert_eq!(snapshot.pressure.len(), 3);
    for resource in proc::PRESSURE_RESOURCES {
        let pressure = snapshot
            .pressure_for(resource, PressureScope::Host)
            .unwrap_or_else(|| panic!("pressão de {resource} lida"));
        assert_eq!(pressure.scope, PressureScope::Host);
    }
    assert_eq!(
        snapshot.pressure_for("memory", PressureScope::Host).and_then(|p| p.some_avg10()),
        Some(6.0)
    );
    assert_eq!(snapshot.pressure_for("memory", PressureScope::Cgroup), None);
}

#[test]
fn sem_psi_no_kernel_o_grupo_fica_indisponivel_e_nao_pressao_zero() {
    let temp = fixtures::TempFixture::new("linux");
    temp.remove("proc/pressure/cpu");
    temp.remove("proc/pressure/memory");
    temp.remove("proc/pressure/io");

    let snapshot = fixtures::collector_at(temp.root()).collect();
    assert!(snapshot.pressure.is_empty());
    assert!(!snapshot.group_available(Group::Pressure));

    let text = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    assert!(
        !text.contains("godmesh_host_pressure_stalled_ratio"),
        "nenhuma série de pressão quando nada foi lido:\n{text}"
    );
    assert!(
        text.contains("godmesh_host_group_available{group=\"pressure\"} 0"),
        "a falha é afirmada explicitamente:\n{text}"
    );
    // e as outras leituras continuam
    assert!(snapshot.group_available(Group::Memory));
    assert!(text.contains("godmesh_host_group_available{group=\"memory\"} 1"));
}

#[test]
fn container_reporta_cgroup_e_deixa_claro_que_o_numero_do_namespace() {
    let snapshot = fixtures::collector("container").collect();
    assert!(snapshot.container);
    assert_eq!(snapshot.source, Source::Proc);

    let cgroup = snapshot.cgroup.as_ref().expect("cgroup lido");
    assert_eq!(cgroup.version, Some(2));
    assert_eq!(cgroup.memory_current_bytes, Some(734_003_200));
    assert_eq!(cgroup.memory_limit_bytes, Some(1_073_741_824));
    assert_eq!(cgroup.cpu_quota_cores, Some(2.0));
    assert_eq!(cgroup.cpu_stat.get("throttled_usec"), Some(&5_000_000));

    // A prova de que as duas coisas são diferentes, e o painel diz qual é qual:
    // o limite do contêiner (2 núcleos) não é o que /proc/stat mostra (4).
    let times = snapshot.cpu_times.as_ref().expect("cpu");
    assert_eq!(times.cores, 4);
    assert_ne!(
        cgroup.cpu_quota_cores.expect("quota do namespace"),
        f64::from(times.cores),
        "cgroup ≠ host é justamente o ponto da fatia"
    );

    // memória: current do cgroup não é o usado de meminfo
    let memory = snapshot.memory.as_ref().expect("meminfo");
    assert_ne!(cgroup.memory_current_bytes, memory.used_bytes());

    // pressão dos dois escopos coexistem, com números diferentes
    let host = snapshot
        .pressure_for("cpu", PressureScope::Host)
        .and_then(|p| p.some_avg10());
    let own = snapshot
        .pressure_for("cpu", PressureScope::Cgroup)
        .and_then(|p| p.some_avg10());
    assert_eq!(host, Some(12.5));
    assert_eq!(own, Some(30.0));
    assert_ne!(host, own);
}

#[test]
fn resumo_de_capacidade_usa_a_pressao_do_namespace_em_container() {
    let snapshot = fixtures::collector("container").collect();
    let capacity = snapshot.capacity();
    // Em contêiner, o resumo é o número do cgroup (30.0), não o do host (12.5).
    assert_eq!(capacity.cpu_pressure, Some(30.0));
    assert_eq!(capacity.memory_pressure, Some(25.0));
    // pior caso entre os volumes: /var/lib/godmesh (4 de 100 GiB livres)
    assert!(
        (capacity.disk_free_ratio.expect("ratio") - 0.04).abs() < 1e-9,
        "{:?}",
        capacity.disk_free_ratio
    );
}

#[test]
fn sem_leitura_nao_ha_capacidade_inventada() {
    let snapshot = godmesh_host::snapshot::Snapshot::empty();
    let capacity = snapshot.capacity();
    assert_eq!(capacity, godmesh_host::snapshot::Capacity::default());
    assert_eq!(capacity.cpu_pressure, None);
    assert_eq!(capacity.memory_pressure, None);
    assert_eq!(capacity.disk_free_ratio, None);
}

#[test]
fn tcp_descricao_agregada_e_limite_de_cardinalidade_do_fixture() {
    let snapshot = fixtures::collector("linux").collect();
    let states = snapshot.tcp.as_ref().expect("tcp");
    assert_eq!(states.by_state.get("listen"), Some(&3));
    assert_eq!(states.total(), 7);
    // 4 estados presentes + 1 série de soma: nunca uma série por conexão.
    assert_eq!(states.by_state.len(), 4);
}

#[test]
fn conntrack_ilegivel_degrada_o_grupo_e_nao_publica_zero() {
    let snapshot = fixtures::collector("linux").collect();
    let conntrack = snapshot.conntrack.as_ref().expect("conntrack do fixture");
    assert_eq!(conntrack.count, 1234);
    assert_eq!(conntrack.max, Some(65_536));
    assert!((conntrack.usage_ratio().expect("ratio") - 1234.0 / 65536.0).abs() < 1e-12);

    // sem o módulo: arquivo ausente
    let temp = fixtures::TempFixture::new("linux");
    temp.remove("proc/sys/net/netfilter/nf_conntrack_count");
    let sem_modulo = fixtures::collector_at(temp.root()).collect();
    assert_eq!(sem_modulo.conntrack.as_ref().err(), Some(&Unavailable::Missing));
    assert!(!sem_modulo.group_available(Group::Conntrack));
    let text = godmesh_host::expose::render(&sem_modulo, sem_modulo.taken_at_millis, None);
    assert!(!text.contains("godmesh_host_conntrack_entries"), "{text}");
    assert!(text.contains("godmesh_host_group_available{group=\"conntrack\"} 0"));

    // sem max legível: entries sai, limit e ratio não
    let temp2 = fixtures::TempFixture::new("linux");
    temp2.remove("proc/sys/net/netfilter/nf_conntrack_max");
    let sem_max = fixtures::collector_at(temp2.root()).collect();
    assert_eq!(sem_max.conntrack.as_ref().ok().and_then(|c| c.max), None);
    let texto = godmesh_host::expose::render(&sem_max, sem_max.taken_at_millis, None);
    assert!(texto.contains("godmesh_host_conntrack_entries 1234"), "{texto}");
    assert!(!texto.contains("godmesh_host_conntrack_limit"), "{texto}");
    assert!(!texto.contains("godmesh_host_conntrack_usage_ratio"), "{texto}");
}

#[test]
fn interfaces_sao_agregadas_por_interface_com_o_teto_aplicado() {
    let snapshot = fixtures::collector("linux").collect();
    assert_eq!(snapshot.interfaces.len(), 3);
    assert!(!snapshot.interfaces_truncated);

    // tetos declarados em `volumes`; 20 interfaces viram 16 + o aviso de corte
    let temp = fixtures::TempFixture::new("linux");
    let mut dev = String::from("Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n");
    for index in 0..20 {
        dev.push_str(&format!(
            "   if{index}: {index}000 1 0 0 0 0 0 0 {index}000 1 0 0 0 0 0 0\n"
        ));
    }
    temp.write("proc/net/dev", &dev);
    let many = fixtures::collector_at(temp.root()).collect();
    assert_eq!(many.interfaces.len(), godmesh_host::volumes::MAX_INTERFACES);
    assert!(many.interfaces_truncated, "o corte tem que ser declarado");

    let texto = godmesh_host::expose::render(&many, many.taken_at_millis, None);
    assert!(
        texto.contains("godmesh_host_cardinality_truncated{type=\"interface\"} 1"),
        "{texto}"
    );
}

#[test]
fn volumes_de_pseudo_fs_nao_chegam_a_ser_publicados() {
    // O coletor filtra; a fixture de volumes de teste é real (ext4/xfs) para
    // as razões, e um volume tmpfs entra já filtrado pela fonte.
    let collector = godmesh_host::collect::Collector::with_read(
        godmesh_host::read::Read::new(fixtures::fixture_root("linux")),
        std::sync::Arc::new(godmesh_host::collect::StaticVolumes(vec![
            godmesh_host::snapshot::Volume {
                mount_point: "/".to_owned(),
                file_system: "ext4".to_owned(),
                total_bytes: 1000,
                available_bytes: 500,
                read_only: false,
            },
            godmesh_host::snapshot::Volume {
                mount_point: "/proc".to_owned(),
                file_system: "proc".to_owned(),
                total_bytes: 0,
                available_bytes: 0,
                read_only: false,
            },
            godmesh_host::snapshot::Volume {
                mount_point: "/dev/shm".to_owned(),
                file_system: "tmpfs".to_owned(),
                total_bytes: 65_536,
                available_bytes: 65_536,
                read_only: false,
            },
        ])),
    );
    let snapshot = collector.collect();
    assert_eq!(snapshot.volumes.len(), 1, "só ext4 é capacidade de disco");
    assert_eq!(snapshot.volumes[0].mount_point, "/");
    assert!(snapshot.group_available(Group::Disk));
    assert_eq!(snapshot.group_source(Group::Disk), Some(Source::Statfs));
}

#[test]
fn sem_volume_legivel_o_grupo_disk_fica_indisponivel() {
    let collector = godmesh_host::collect::Collector::with_read(
        godmesh_host::read::Read::new(fixtures::fixture_root("linux")),
        std::sync::Arc::new(godmesh_host::collect::StaticVolumes(Vec::new())),
    );
    let snapshot = collector.collect();
    assert!(!snapshot.group_available(Group::Disk));
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    assert!(!texto.contains("godmesh_host_disk_bytes"), "{texto}");
    assert!(texto.contains("godmesh_host_group_available{group=\"disk\"} 0"));
}

#[test]
fn sem_proc_a_feita_o_snapshot_declara_none_e_nao_zeros() {
    // Raiz que não tem nada: é o que acontece num host não-Linux, e o painel
    // tem que dizer isso em vez de publicar zeros.
    let temp = fixtures::TempFixture::new("linux");
    std::fs::remove_dir_all(temp.root().join("proc")).expect("remove proc");
    let snapshot = fixtures::collector_at(temp.root()).collect();

    assert!(!snapshot.group_available(Group::Cpu));
    assert!(!snapshot.group_available(Group::Memory));
    assert_eq!(snapshot.source, Source::Statfs, "sobrou só statfs dos volumes");

    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    assert!(!texto.contains("godmesh_host_cpu_seconds_total"), "{texto}");
    assert!(!texto.contains("godmesh_host_memory_bytes"), "{texto}");
    // o painel de disponibilidade continua inteiro: é ele que afirma a falha
    for group in Group::ALL {
        if group == Group::Disk {
            assert!(texto.contains(&format!("godmesh_host_group_available{{group=\"{}\"}} 1", group.as_str())), "{texto}");
        } else if group != Group::Cgroup {
            assert!(texto.contains(&format!("godmesh_host_group_available{{group=\"{}\"}} 0", group.as_str())), "{texto}");
        }
    }
}

#[test]
fn snapshot_velho_servido_como_indisponivel() {
    let snapshot = fixtures::collector("linux").collect();
    let depois = snapshot.taken_at_millis + 20_000;
    let texto = godmesh_host::expose::render(&snapshot, depois, Some(15_000));
    assert!(
        texto.contains("godmesh_host_metrics_available{source=\"none\"} 0"),
        "snapshot velho não pode ser servido como atual:\n{texto}"
    );
    assert!(!texto.contains("godmesh_host_cpu_seconds_total"), "{texto}");

    // e o frescor dentro da janela serve normally
    let fresco = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis + 1_000, Some(15_000));
    assert!(fresco.contains("godmesh_host_metrics_available{source=\"proc\"} 1"), "{fresco}");
    assert!(fresco.contains("godmesh_host_snapshot_age_seconds 1"), "{fresco}");
}

#[test]
fn limites_ilimitados_nao_viram_numero() {
    let temp = fixtures::TempFixture::new("linux");
    temp.write(
        "proc/self/limits",
        "Limit Soft Limit Hard Limit Units\nMax open files unlimited unlimited files\n",
    );
    let snapshot = fixtures::collector_at(temp.root()).collect();
    let descriptors = snapshot.descriptors.as_ref().expect("fd conta sempre");
    assert_eq!(descriptors.open, 8, "oito descritores na fixture");
    assert_eq!(
        descriptors.soft_limit, None,
        "\"unlimited\" não tem número: ausencia, não zero"
    );
    assert_eq!(descriptors.hard_limit, None);
    assert_eq!(descriptors.usage_ratio(), None, "sem limite não há fração");

    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    assert!(
        texto.contains("godmesh_host_process_descriptors{state=\"open\"} 8"),
        "{texto}"
    );
    assert!(
        !texto.contains("godmesh_host_process_descriptors{state=\"limit_soft\"}"),
        "nenhum número é publicado para um limite ilimitado:\n{texto}"
    );
    assert!(
        !texto.contains("godmesh_host_process_descriptors_usage_ratio"),
        "razão com limite ilimitado não é publicada:\n{texto}"
    );
}

/// cgroup **v1** híbrido: a subtree de memória tem o limite, e a sentinela de
/// "sem limite" de v1 não é o literal `max` de v2 — é um número. Interpretá-lo
/// como limite publicaria um "limite" de 8 EiB.
#[test]
fn container_v1_le_memoria_e_nao_confunde_sentinela_com_limite() {
    let snapshot = fixtures::collector("container-v1").collect();
    assert!(snapshot.container);
    let cgroup = snapshot.cgroup.as_ref().expect("cgroup v1 lido");
    assert_eq!(cgroup.version, Some(1));
    assert_eq!(cgroup.memory_current_bytes, Some(512_000_000));
    assert_eq!(cgroup.memory_limit_bytes, Some(1_073_741_824));

    // Em v1 esta fatia não lê quota de CPU (fica em outra subtree): ausência
    // declarada, não quota igual ao que /proc mostra.
    assert_eq!(cgroup.cpu_quota_cores, None);
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    assert!(
        !fixtures::tem_serie(&texto, "godmesh_host_cgroup_cpu_limit_cores"),
        "sem leitura de quota não pode sair número:\n{texto}"
    );
    assert!(texto.contains("godmesh_host_cgroup_version 1"), "{texto}");

    // pressão: só existe a de /proc; nenhum arquivo *.pressure de v1 é lido
    assert_eq!(
        snapshot.pressure_for("cpu", PressureScope::Cgroup),
        None,
        "v1 não tem *.pressure por cgroup nesta leitura"
    );
    assert_eq!(
        snapshot.capacity().cpu_pressure,
        Some(4.0),
        "o resumo cai no número de /proc quando não há do cgroup"
    );

    // a sentinela
    let sem_limite = fixtures::collector("container-v1-unlimited").collect();
    let cgroup = sem_limite.cgroup.as_ref().expect("lido");
    assert_eq!(
        cgroup.memory_limit_bytes, None,
        "9223372036854771712 é LONG_MAX arredondado a página: \"sem limite\""
    );
    assert_eq!(cgroup.memory_current_bytes, Some(512_000_000));
    let texto = godmesh_host::expose::render(&sem_limite, sem_limite.taken_at_millis, None);
    assert!(
        !fixtures::tem_serie(&texto, "godmesh_host_cgroup_memory_limit_bytes"),
        "{texto}"
    );
    assert!(
        !fixtures::tem_serie(&texto, "godmesh_host_cgroup_memory_usage_ratio"),
        "sem limite não há fração:\n{texto}"
    );
    assert!(
        texto.contains("godmesh_host_cgroup_memory_current_bytes 512000000"),
        "o que foi lido continua publicado:\n{texto}"
    );
}

#[test]
fn caminho_de_v1_nao_e_lido_na_arvore_de_v2_e_vice_versa() {
    // A fixture v1 tem `/sys/fs/cgroup/memory/...`; a v2 tem
    // `/sys/fs/cgroup/<caminho>/memory.max`. Trocar as duas produziria `None`
    // em tudo — o teste garante que cada uma acha o que é seu.
    let v2 = fixtures::collector("container").collect();
    assert_eq!(v2.cgroup.as_ref().unwrap().memory_limit_bytes, Some(1_073_741_824));
    assert_eq!(v2.cgroup.as_ref().unwrap().version, Some(2));

    let v1 = fixtures::collector("container-v1").collect();
    assert_eq!(v1.cgroup.as_ref().unwrap().memory_limit_bytes, Some(1_073_741_824));
    assert_eq!(v1.cgroup.as_ref().unwrap().version, Some(1));

    // Host sem namespace: nenhum dos dois.
    let host = fixtures::collector("linux").collect();
    assert!(host.cgroup.is_err(), "{:?}", host.cgroup);
}
