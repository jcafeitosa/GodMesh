//! Contrato do exposition: formato válido, determinismo e cardinalidade.
//!
//! Estes testes são a defesa contra o erro mais caro de um `/metrics`: uma
//! linha malformada que derruba o scrape inteiro, ou uma série nova que ninguém
//! contou. Eles medem o painel, não só afirmam que ele é pequeno.

use std::collections::BTreeSet;

mod fixtures;

/// Toda linha de série: `nome{rótulos}? valor`. Nada além disso pode aparecer
/// depois do `#`.
fn assert_sintaxe_prometheus(text: &str) {
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if line.starts_with("# HELP ") || line.starts_with("# TYPE ") {
            continue;
        }
        assert!(
            !line.starts_with('#'),
            "comentário fora de HELP/TYPE não existe no exposition 0.0.4: {line}"
        );
        let (metric_and_labels, value) = line
            .rsplit_once(' ')
            .unwrap_or_else(|| panic!("linha sem valor: {line}"));
        value
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("valor {value:?} em {line}: {e}"));
        let name = metric_and_labels
            .split_once('{')
            .map_or(metric_and_labels, |(name, _)| name);
        assert!(
            name.starts_with("godmesh_host_"),
            "prefixo obrigatório: {line}"
        );
        assert!(
            name.len() <= 80,
            "nome de série absurdamente longo: {}",
            name.len()
        );
        if let Some((_, labels)) = metric_and_labels.split_once('{') {
            let labels = labels.strip_suffix('}').unwrap_or_else(|| panic!("sem `}}`: {line}"));
            for pair in split_labels(labels) {
                let (key, val) = pair
                    .split_once('=')
                    .unwrap_or_else(|| panic!("rótulo sem `=`: {pair} em {line}"));
                assert!(!key.is_empty(), "rótulo sem nome: {line}");
                assert!(
                    val.starts_with('"') && val.ends_with('"'),
                    "valor de rótulo sem aspas: {val} em {line}"
                );
            }
        }
    }
}

/// Separa `a="1",b="2"` respeitando aspas escapadas.
fn split_labels(labels: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, ch) in labels.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(&labels[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    out.push(&labels[start..]);
    out
}

/// Toda família declarada tem TYPE e o TYPE é um dos quatro válidos.
fn assert_types(text: &str) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# TYPE ") {
            let (name, kind) = rest
                .split_once(' ')
                .unwrap_or_else(|| panic!("TYPE sem nome: {line}"));
            assert!(
                matches!(kind, "counter" | "gauge" | "histogram" | "summary"),
                "TYPE inválido {kind:?}"
            );
            assert!(seen.insert(name), "TYPE duplicado para {name}");
        }
        if let Some(rest) = line.strip_prefix("# HELP ") {
            let name = rest.split(' ').next().expect("HELP com nome");
            assert!(name.starts_with("godmesh_host_"), "{name}");
        }
    }
    // nenhuma série sem TYPE: é o que faz um exporter quebrar o scrape
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line.split(['{', ' ']).next().expect("linha com nome");
        assert!(seen.contains(name), "série {name} publicada sem TYPE");
    }
}

#[test]
fn exposition_valido_para_host_e_container() {
    for fixture in ["linux", "container"] {
        let snapshot = fixtures::collector(fixture).collect();
        let text = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
        assert!(!text.is_empty(), "{fixture} não renderizou nada");
        assert_sintaxe_prometheus(&text);
        assert_types(&text);
    }
}

#[test]
fn renderizacao_e_deterministica() {
    let snapshot = fixtures::collector("linux").collect();
    let agora = snapshot.taken_at_millis;
    let primeiro = godmesh_host::expose::render(&snapshot, agora, None);
    let segundo = godmesh_host::expose::render(&snapshot, agora, None);
    assert_eq!(primeiro, segundo, "dois scrapes do mesmo snapshot");
}

#[test]
fn cardinalidade_medida_antes_e_depois() {
    // Este é o número que a tarefa pede para comparar. Se ele crescer com um
    // rótulo por conexão/porta/tenant, este teste estoura.
    let snapshot = fixtures::collector("linux").collect();
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    let linhas = texto.lines().count();
    let series = texto
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count();
    let familias = texto
        .lines()
        .filter(|line| line.starts_with("# TYPE "))
        .count();

    println!("cardinalidade (fixture linux): {linhas} linhas, {series} séries, {familias} famílias");
    // Medido nesta fatia: 123 séries / 34 famílias para a fixture `linux`
    // (piso de 160 = ~30% de folga). O crescimento que um rótulo por conexão
    // produziria é o que este assert pega, não a diferença de duas ou três.
    assert!(
        series <= 160,
        "{series} séries: painel de host com cardinalidade limitada fica sob 160"
    );
    assert!(familias <= 40, "{familias} famílias é demais para um nó");

    // teto mesmo com o corte de interfaces e muitos volumes
    let temp = fixtures::TempFixture::new("linux");
    let mut dev = String::from("Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n");
    for index in 0..64 {
        dev.push_str(&format!("   ifx{index}: 1 1 0 0 0 0 0 0 1 1 0 0 0 0 0 0\n"));
    }
    temp.write("proc/net/dev", &dev);
    let snapshot = fixtures::collector_at(temp.root()).collect();
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    let series = texto
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count();
    println!("cardinalidade (64 interfaces, teto 16): {series} séries");
    // Medido: o teto de 16 interfaces custa 128 séries de contadores (4
    // famílias × 16 × rx/tx). Com os grupos fixos, o pior caso é 227 — e é
    // esse número que este assert segura, não uma estimativa.
    assert!(series <= 240, "o teto de interfaces precisa segurar: {series}");
    assert_sintaxe_prometheus(&texto);
}

#[test]
fn nada_e_publicado_sem_leitura_nem_para_o_snapshot_vazio() {
    let vazio = godmesh_host::snapshot::Snapshot::empty();
    let texto = godmesh_host::expose::render(&vazio, 0, None);
    assert_types(&texto);
    assert_sintaxe_prometheus(&texto);
    // só o painel de disponibilidade, e ele diz 0
    assert!(
        texto.contains("godmesh_host_metrics_available{source=\"none\"} 0"),
        "{texto}"
    );
    assert!(!texto.contains("godmesh_host_cpu_seconds_total"), "{texto}");
    assert!(!texto.contains("godmesh_host_memory_bytes"), "{texto}");
}

#[test]
fn unidades_estao_no_nome_das_series() {
    let snapshot = fixtures::collector("linux").collect();
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    let familias: BTreeSet<&str> = texto
        .lines()
        .filter_map(|line| line.strip_prefix("# TYPE "))
        .filter_map(|line| line.split(' ').next())
        .collect();

    // Nenhuma família pode usar unidade fora do padrão (bytes/seconds/ratio):
    // `_kb`/`_ms` num nome é exatamente o tipo de ambiguidade que a tarefa
    // proíbe, e um scraper que soma `_bytes` com `_kb` erra por 1024.
    for family in &familias {
        let name = family.strip_prefix("godmesh_host_").expect("prefixo");
        assert!(
            !name.contains("_kb") && !name.contains("_mb") && !name.contains("_ms"),
            "{family}: unidade fora do padrão (use bytes/seconds/ratio)"
        );
    }
    // nomes que a tarefa exige com unidade explícita
    for esperado in [
        "godmesh_host_memory_bytes",
        "godmesh_host_swap_bytes",
        "godmesh_host_disk_bytes",
        "godmesh_host_cpu_seconds_total",
        "godmesh_host_pressure_stalled_seconds_total",
        "godmesh_host_collection_duration_seconds",
        "godmesh_host_memory_usage_ratio",
    ] {
        assert!(
            texto.contains(&format!("# TYPE {esperado} ")),
            "{esperado} ausente do painel"
        );
    }
}

#[test]
fn nenhum_rotulo_parece_identificador_de_conexao_ou_tenant() {
    // A regra de ADR-073 é agregação; o teste procura o rótulo proibido.
    let snapshot = fixtures::collector("container").collect();
    let texto = godmesh_host::expose::render(&snapshot, snapshot.taken_at_millis, None);
    for proibido in [
        "peer=",
        "tenant=",
        "connection=",
        "remote=",
        "port=",
        "inode=",
        "pid=",
        "addr=",
        "uid=",
    ] {
        assert!(
            !texto.contains(proibido),
            "rótulo {proibido} é cardinalidade de conexão/identidade: proibido"
        );
    }
}

/// Round trip: a árvore copiada por [`godmesh_host::tree::write`] e lida pelo
/// coletor tem que produzir **o mesmo bytes** de exposition que a árvore
/// original. É a prova de que a evidência gerada num nó (com privilégio de
/// leitura) é a mesma interface que o coletor serve.
#[test]
fn arvore_copiada_renderiza_identica() {
    for fixture in ["linux", "container"] {
        let origem = godmesh_host::read::Read::new(fixtures::fixture_root(fixture));
        let copia = fixtures::TempFixture::new(fixture);
        // sobrescreve a cópia com a árvore escrita pelo próprio módulo de cópia
        let dir = std::env::temp_dir().join(format!(
            "godmesh-tree-{}-{fixture}",
            std::process::id()
        ));
        let faltantes =
            godmesh_host::tree::write(&origem, &dir).expect("copia da árvore");
        let collector = fixtures::collector_at(&dir);

        let esperado = fixtures::collector(fixture).collect();
        let obtido = collector.collect();

        let agora = 1_700_000_000_000;
        let texto_esperado = strip_datas(godmesh_host::expose::render(&esperado, agora, None));
        let texto_obtido = strip_datas(godmesh_host::expose::render(&obtido, agora, None));
        assert_eq!(
            texto_esperado, texto_obtido,
            "árvore copiada de {fixture} divergiu (faltantes: {faltantes:?})"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = copia.root();
    }
}

/// Remove as famílias de tempo relativo (idade do snapshot e custo da coleta),
/// que por definição mudam entre execuções — inclusive os cabeçalhos `HELP` e
/// `TYPE` delas, para não deixar uma família sem série no texto comparado.
fn strip_datas(texto: String) -> String {
    const RELATIVAS: [&str; 2] = [
        "godmesh_host_snapshot_age_seconds",
        "godmesh_host_collection_duration_seconds",
    ];
    let mut blocos: Vec<Vec<String>> = Vec::new();
    for line in texto.lines() {
        if line.starts_with("# HELP ") {
            blocos.push(Vec::new());
        }
        if let Some(bloco) = blocos.last_mut() {
            bloco.push(line.to_owned());
        }
    }
    blocos
        .retain(|bloco| {
            !bloco
                .iter()
                .any(|line| RELATIVAS.iter().any(|nome| line.starts_with(nome)))
        });
    blocos
        .into_iter()
        .flatten()
        .map(|line| format!("{line}\n"))
        .collect()
}
