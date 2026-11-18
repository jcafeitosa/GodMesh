//! Custo da coleta: medido, com teto, e nunca em laço apertado.
//!
//! A tarefa pede "custo declarado e medido por teste de fumaça". Este arquivo é
//! a medição. Ele não afirma capacidade: afirma que **uma** coleta deste
//! snapshot, nesta máquina, neste sistema de arquivos, custou X, e que a
//! coleta não roda em laço apertado — `N` aqui é 7, não "até cansar".
//!
//! O que fica de fora, declarado: carga de produção, `/proc` real de um nó
//! federado sob tráfego, e qualquer hardware que não é esta máquina.

use std::time::Instant;

mod fixtures;

/// Amostras de coleta. Poucas, deliberadamente: um laço apertado leria o
/// cache e mediria o cache.
const COLETS: usize = 7;

/// Teto do teste: orçamento declarado do intervalo padrão de 5 s
/// ([`godmesh_host::budget`] = 5% = 250 ms).
#[test]
fn uma_coleta_de_snapshot_cabe_no_orcamento_declarado() {
    let collector = fixtures::collector("linux");

    // A primeira coleta povoa a amostra anterior de CPU; as seguintes medem o
    // trabalho normal. A janela de taxa é o que a segunda coleta produz — com
    // uma fixture parada o delta de jiffies é 0 e a série fica ausente de
    // propósito (isso é coberto por `collect_fixtures`, não aqui).
    // primeira coleta: povoa a amostra anterior (o retorno é descartado de
    // propósito — é o custo dela que interessa, não o conteúdo)
    std::mem::drop(collector.collect());
    let mut duracoes = Vec::with_capacity(COLETS);
    for _ in 0..COLETS {
        let started = Instant::now();
        let snapshot = collector.collect();
        duracoes.push(started.elapsed());
        assert_eq!(snapshot.source, godmesh_host::snapshot::Source::Proc);
    }

    let max = duracoes.iter().max().copied().expect("amostras");
    let media = duracoes.iter().sum::<std::time::Duration>() / COLETS as u32;
    let budget = godmesh_host::budget(godmesh_host::config::DEFAULT_INTERVAL);

    // Sem percentil: 7 amostras não sustentam p95, e a documentação deste
    // repositório proíbe publicar percentil sem população e método declarados.
    println!(
        "custo da coleta (fixture linux, {COLETS} coletas): média {} µs, máximo {} µs, \
        orçamento {} µs (5% do intervalo de 5 s)",
        media.as_micros(),
        max.as_micros(),
        budget.as_micros(),
    );

    assert!(
        max <= budget,
        "coleta custou {} µs, acima do orçamento de {} µs para um intervalo de 5 s",
        max.as_micros(),
        budget.as_micros()
    );

    // custo por scrape é 0 de I/O: o handler lê um snapshot pronto
    let shared = godmesh_host::collect::SharedSnapshot::new();
    assert!(shared.latest().is_none());
    let started = Instant::now();
    for _ in 0..10_000 {
        std::hint::black_box(shared.latest());
    }
    let por_leitura = started.elapsed() / 10_000;
    println!("custo de servir o snapshot: {} ns por leitura", por_leitura.as_nanos());
    assert!(
        por_leitura < std::time::Duration::from_micros(50),
        "ler o snapshot publicado não pode custar {} µs",
        por_leitura.as_micros()
    );
}

/// A árvore do coletor inteiro (fixture `container`) é a maior: cgroup
/// somado a `/proc`.
#[test]
fn coleta_de_container_cabe_no_mesmo_orcamento() {
    let collector = fixtures::collector("container");
    std::mem::drop(collector.collect());
    let mut max = std::time::Duration::ZERO;
    for _ in 0..COLETS {
        let started = Instant::now();
        let snapshot = collector.collect();
        let elapsed = started.elapsed();
        if elapsed > max {
            max = elapsed;
        }
        assert!(snapshot.container);
    }
    println!(
        "custo da coleta (fixture container, {COLETS} coletas): máximo {} µs",
        max.as_micros()
    );
    assert!(
        max <= godmesh_host::budget(godmesh_host::config::DEFAULT_INTERVAL),
        "cgroup não pode quebrar o orçamento: {} µs",
        max.as_micros()
    );
}

/// Fumaça no `/proc` **real**, quando existir.
///
/// Em macOS (host de desenvolvimento) não existe `/proc`, e o teste declara
/// isso em vez de passar em falso: é justamente o caminho de degradação que a
/// fatia precisa garantir. Em Linux, o teste roda uma coleta e exige que ela
/// termine e se auto-descreva — não que algum valor numérico confira, porque
/// número de host de CI não é evidência de nada.
#[test]
fn smoke_no_proc_real_quando_o_host_for_linux() {
    let read = godmesh_host::read::Read::new("/");
    let existe_proc = read.exists("proc/stat");

    if !existe_proc {
        // Degradação real, medida no host de desenvolvimento: sem /proc não há
        // CPU/memória/rede, e o painel tem que dizer `source="none"` (ou
        // "statfs", se sobreviver só a lista de volumes).
        let snapshot = godmesh_host::collect::Collector::new().collect();
        println!(
            "host sem /proc ({}): source={:?}, groups={:?}",
            std::env::consts::OS,
            snapshot.source,
            godmesh_host::collect::group_report(&snapshot)
        );
        assert!(!snapshot.group_available(godmesh_host::snapshot::Group::Cpu));
        assert!(!snapshot.group_available(godmesh_host::snapshot::Group::Memory));
        let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
        assert!(
            !texto.contains("godmesh_host_cpu_seconds_total"),
            "sem /proc não pode sair número de CPU"
        );
        return;
    }

    let collector = godmesh_host::collect::Collector::new();
    let started = Instant::now();
    std::mem::drop(collector.collect());
    let snapshot = collector.collect();
    let elapsed = started.elapsed();
    println!(
        "/proc real: coleta dupla em {} ms, source={:?}, cpu_cores={:?}, memória {:?} bytes",
        elapsed.as_millis(),
        snapshot.source,
        snapshot.cpu_times.as_ref().ok().map(|t| t.cores),
        snapshot.memory.as_ref().ok().map(|m| m.total_bytes),
    );
    assert!(snapshot.group_available(godmesh_host::snapshot::Group::Cpu));
    assert!(snapshot.group_available(godmesh_host::snapshot::Group::Memory));
    assert!(
        snapshot.cpu_window.is_some(),
        "com duas amostras reais existe janela de taxa"
    );
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    let series = texto
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count();
    println!("/proc real: {series} séries publicadas");
    assert!(series <= 400, "/metrics real explodiu: {series} séries");
}

/// A raiz configurável é o que permite rodar a árvore real do nó dentro de um
/// diretório — é assim que a evidência compara coletor e `df`.
#[test]
fn raiz_de_fixture_e_lida_pelo_mesmo_coletor_da_producao() {
    let collector = fixtures::collector("linux");
    let raiz = collector.root();
    assert!(raiz.join("proc/stat").is_file(), "{}/proc/stat", raiz.display());
    let snapshot = collector.collect();
    assert_eq!(snapshot.source, godmesh_host::snapshot::Source::Proc);
}
