//! Ferramenta de evidência da camada "observar" (ADR-073).
//!
//! Ela existe para que a conferência seja um comando repetível, não uma
//! descrição em prosa. Quatro modos:
//!
//! ```text
//! host-evidence dump                 # o que este host sabe sobre si mesmo
//! host-evidence metrics              # a seção godmesh_host_* do /metrics
//! host-evidence proc-fs DIR ARQUIVO  # lê uma árvore de /proc copiada
//! host-evidence volumes              # volumes por statfs (o que o coletor vê)
//! host-evidence tree-write DIR       # copia a árvore de /proc legível para DIR
//! ```
//!
//! `proc-fs` existe porque a comparação que a tarefa pede — coletor ×
//! ferramenta de sistema — precisa rodar contra a **mesma** árvore que o
//! coletor lê, sem exigir privilégio nem contêiner: no Linux do nó,
//! `cp -a /proc` de um processo não-root copia o que ele pode ler, e é isso
//! que se confere com `free`/`df`.
//!
//! Nenhum modo escreve em `/proc`, nenhum chama `stat` de outro usuário,
//! nenhum abre netlink: é a camada 1, read-only.

use std::path::PathBuf;

use godmesh_host::collect::{Collector, SharedSnapshot};
use godmesh_host::read::Read;
use godmesh_host::snapshot::{Snapshot, Source};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("host-evidence: {message}");
            2
        }
    };
    std::process::exit(code);
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("dump") | Some("metrics") => {
            let collector = Collector::new();
            let snapshot = evidence_snapshot(&collector)?;
            if args.first().map(String::as_str) == Some("metrics") {
                print!(
                    "{}",
                    godmesh_host::expose::render(
                        &snapshot,
                        godmesh_host::collect::unix_millis(),
                        None,
                    )
                );
            } else {
                dump(&snapshot);
            }
            Ok(())
        }
        Some("proc-fs") => {
            let root = PathBuf::from(args.get(1).ok_or("esperado: proc-fs DIR [ARQUIVO]")?);
            if !root.is_absolute() {
                return Err(format!("DIR precisa ser absoluto: {}", root.display()));
            }
            let collector = Collector::with_read(
                Read::new(&root),
                std::sync::Arc::new(godmesh_host::collect::SysinfoVolumes),
            );
            let snapshot = evidence_snapshot(&collector)?;
            let texto = godmesh_host::expose::render(
                &snapshot,
                godmesh_host::collect::unix_millis(),
                None,
            );
            dump(&snapshot);
            if let Some(destino) = args.get(2) {
                std::fs::write(destino, &texto)
                    .map_err(|e| format!("escrever {destino}: {e}"))?;
                println!(
                    "exposition escrito em {destino}: {} linhas",
                    texto.lines().count()
                );
            } else {
                print!("{texto}");
            }
            Ok(())
        }
        Some("volumes") => {
            let mut last: Option<Snapshot> = None;
            for _ in 0..2 {
                last = Some(evidence_snapshot(&Collector::new())?);
            }
            let snapshot = last.expect("coletado");
            println!("statfs (o que o coletor de volumes vê):");
            if snapshot.volumes.is_empty() {
                println!("  nenhum volume não-pseudo lido — grupo indisponível, não zero");
            }
            for volume in &snapshot.volumes {
                println!(
                    "  {:<24} {:<10} total {:>12} bytes  available {:>12} bytes  free {:.4}{}",
                    volume.mount_point,
                    volume.file_system,
                    volume.total_bytes,
                    volume.available_bytes,
                    volume.free_ratio().unwrap_or(f64::NAN),
                    if volume.read_only { "  read-only" } else { "" },
                );
            }
            Ok(())
        }
        Some("tree-write") => {
            let destino = PathBuf::from(args.get(1).ok_or("esperado: tree-write DIR")?);
            if !destino.is_absolute() {
                return Err(format!("DIR precisa ser absoluto: {}", destino.display()));
            }
            let faltantes = godmesh_host::tree::write(&Read::default(), &destino)
                .map_err(|e| format!("copiando para {}: {e}", destino.display()))?;
            println!(
                "árvore copiada para {} ({}/{} caminhos fixos + cgroup)",
                destino.display(),
                godmesh_host::tree::PATHS.len() + godmesh_host::tree::CGROUP_FILES.len()
                    - faltantes.len(),
                godmesh_host::tree::PATHS.len() + godmesh_host::tree::CGROUP_FILES.len(),
            );
            if faltantes.is_empty() {
                println!("  nada faltou: este host expõe todos os caminhos lidos");
            } else {
                println!("  faltou (ilegível sem privilégio, ou inexistente):");
                for item in &faltantes {
                    println!("    {item}");
                }
            }
            println!(
                "  conferência: host-evidence proc-fs {} /tmp/host-metrics.txt",
                destino.display()
            );
            Ok(())
        }
        Some(outra) => Err(format!(
            "modo desconhecido {outra}; uso: host-evidence \
                 [dump|metrics|proc-fs DIR [ARQUIVO]|volumes|tree-write DIR]"
        )),
    }
}

/// Coleta duas vezes (a segunda produz a janela de taxa) e publica num
/// [`SharedSnapshot`], que é exatamente o objeto que o servidor serve.
fn evidence_snapshot(collector: &Collector) -> Result<Snapshot, String> {
    let shared = SharedSnapshot::new();
    shared.store(collector.collect());
    let snapshot = collector.collect();
    if let Some(publicado) = shared.latest() {
        // Custo do *publish*, medido na mesma estrutura do servidor.
        let started = std::time::Instant::now();
        for _ in 0..1_000 {
            std::hint::black_box(shared.latest());
        }
        let _ = &publicado;
        eprintln!(
            "custo de ler o snapshot publicado: {} ns por leitura",
            started.elapsed().as_nanos() / 1_000
        );
    }
    Ok(snapshot)
}

/// O auto-relato do host: fonte, grupos, e por que cada grupo está ausente.
fn dump(snapshot: &Snapshot) {
    println!("host-evidence — relatório da coleta");
    println!("  sistema operacional: {}", std::env::consts::OS);
    println!("  raiz lida: {}", Collector::new().root().display());
    println!("  fonte dominante: {:?}", snapshot.source);
    println!("  contêiner (cgroup próprio lido): {}", snapshot.container);
    println!(
        "  custo desta coleta: {:.6} s",
        snapshot.collection_seconds
    );
    println!("  grupos (fonte ou \"none\" = não medido):");
    for (grupo, fonte) in godmesh_host::collect::group_report(snapshot) {
        println!("    {grupo:<12} {fonte}");
    }

    if let Ok(times) = &snapshot.cpu_times {
        println!(
            "  cpu: {} núcleos, {:.1} s acumulados desde o boot",
            times.cores,
            times.total_seconds()
        );
    }
    if let Some(window) = &snapshot.cpu_window {
        println!(
            "  cpu na janela de {:.3} s: busy {:.4}, io_wait {:.4}, steal {:.4}",
            window.window_seconds,
            window.busy_ratio,
            window.ratios[&godmesh_host::snapshot::CpuState::IoWait],
            window.ratios[&godmesh_host::snapshot::CpuState::Steal],
        );
    }
    if let Ok(memory) = &snapshot.memory {
        println!(
            "  memória: total {} bytes, disponível {:?}, usado {:?}",
            memory.total_bytes,
            memory.available_bytes,
            memory.used_bytes()
        );
    }
    for pressure in &snapshot.pressure {
        println!(
            "  pressão {}: scope={} some avg10={:?} full avg10={:?}",
            pressure.resource,
            pressure.scope.as_str(),
            pressure.some.map(|line| line.avg10),
            pressure.full.map(|line| line.avg10),
        );
    }
    if let Ok(descriptors) = &snapshot.descriptors {
        println!(
            "  descritores: {} abertos, soft {:?}, hard {:?}",
            descriptors.open, descriptors.soft_limit, descriptors.hard_limit
        );
    }
    if let Ok(states) = &snapshot.tcp {
        println!("  tcp por estado: {:?}", states.by_state);
    }
    if let Ok(conntrack) = &snapshot.conntrack {
        println!(
            "  conntrack: {} entradas, limite {:?}",
            conntrack.count, conntrack.max
        );
    }
    println!("  interfaces: {}", snapshot.interfaces.len());
    for interface in &snapshot.interfaces {
        println!(
            "    {:<12} rx {} bytes, tx {} bytes, rx drop {}, tx drop {}",
            interface.name,
            interface.rx_bytes,
            interface.tx_bytes,
            interface.rx_dropped,
            interface.tx_dropped,
        );
    }
    println!("  volumes: {}", snapshot.volumes.len());
    for volume in &snapshot.volumes {
        println!(
            "    {:<24} {:<10} total {:>14}  available {:>14}  free {:?}",
            volume.mount_point,
            volume.file_system,
            volume.total_bytes,
            volume.available_bytes,
            volume.free_ratio(),
        );
    }
    let capacity = snapshot.capacity();
    println!(
        "  resumo de capacidade (/healthz): cpu_pressao={:?} memoria_pressao={:?} \
         disco_livre={:?}",
        capacity.cpu_pressure, capacity.memory_pressure, capacity.disk_free_ratio
    );
    println!(
        "  atenção: pressão NÃO julga saúde nesta fatia (ADR-073 camada 2 ainda \
         sem limiar/histerese aprovados)"
    );
    if snapshot.source == Source::None {
        println!("  nenhuma leitura de host disponível: /metrics dirá source=\"none\"");
    }
}
