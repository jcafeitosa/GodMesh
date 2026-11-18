//! Árvore de `/proc` copiada para disco, para evidência repetível.
//!
//! É a costura que permite medir o coletor contra a árvore real de um nó sem
//! privilégio e sem contêiner: `host-evidence tree-write DIR` copia o que o
//! processo consegue ler; `host-evidence proc-fs DIR` roda o **mesmo**
//! coletor sobre essa cópia.
//!
//! O que a cópia não tem é declarado como faltante — não é falha da
//! ferramenta, é a interface real do kernel (`nf_conntrack_max`, por exemplo,
//! é `0440 root:conntrack` em muitos kernels).

use std::io;
use std::path::Path;

use crate::proc;
use crate::read::Read;

/// Caminhos fixos que o coletor lê, na ordem da leitura.
pub const PATHS: &[&str] = &[
    "proc/stat",
    "proc/loadavg",
    "proc/meminfo",
    "proc/pressure/cpu",
    "proc/pressure/memory",
    "proc/pressure/io",
    "proc/self/status",
    "proc/self/limits",
    "proc/self/cgroup",
    "proc/self/mounts",
    "proc/mounts",
    "proc/net/dev",
    "proc/net/sockstat",
    "proc/net/tcp",
    "proc/net/tcp6",
    "proc/sys/net/netfilter/nf_conntrack_count",
    "proc/sys/net/netfilter/nf_conntrack_max",
];

/// Arquivos de cgroup do próprio processo (caminho dinâmico, descoberto por
/// `proc/self/cgroup` + mounts, como no coletor).
pub const CGROUP_FILES: &[&str] = &[
    "cpu.max",
    "cpu.pressure",
    "cpu.stat",
    "io.pressure",
    "memory.current",
    "memory.max",
    "memory.pressure",
];

/// Copia para `destino` os caminhos que `read` consegue ler, preservando a
/// árvore relativa. Cada arquivo é escrito com o conteúdo **exatamente como
/// lido**: nenhum caminho é inventado e nenhum valor é reformatado.
///
/// `proc/self/fd` é um diretório — o que importa é a contagem, então a cópia
/// grava um arquivo vazio por descritor, com o nome numérico.
///
/// Devolve o que faltou (ilegível ou inexistente), para o relatório dizer
/// quais números da evidência vêm de fixture e quais vêm do nó.
///
/// # Errors
/// Falha de I/O ao escrever em `destino`.
pub fn write(read: &Read, destino: &Path) -> Result<Vec<String>, io::Error> {
    let mut faltantes = Vec::new();

    for relative in PATHS {
        match read.text(relative) {
            Ok(texto) => gravar(&destino.join(relative), &texto, &mut faltantes, relative)?,
            Err(_) => faltantes.push((*relative).to_owned()),
        }
    }

    match read.entries("proc/self/fd") {
        Ok(entries) => {
            let dir = destino.join("proc/self/fd");
            std::fs::create_dir_all(&dir)?;
            for entry in entries {
                std::fs::write(dir.join(entry), "")?;
            }
        }
        Err(_) => faltantes.push("proc/self/fd".to_owned()),
    }

    if let Some(base) = cgroup_base(read) {
        for nome in CGROUP_FILES {
            let relative = format!("{base}/{nome}");
            match read.text(&relative) {
                Ok(texto) => gravar(&destino.join(&relative), &texto, &mut faltantes, &relative)?,
                Err(_) => faltantes.push(relative),
            }
        }
    } else {
        faltantes.push("cgroup (namespace root: sem arquivos próprios)".to_owned());
    }

    Ok(faltantes)
}

fn gravar(
    path: &Path,
    texto: &str,
    faltantes: &mut Vec<String>,
    relative: &str,
) -> Result<(), io::Error> {
    if texto.is_empty() {
        // Arquivo legível mas vazio: grava vazio (é o conteúdo real) e marca.
        faltantes.push(format!("{relative} (vazio)"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, texto)
}

/// Raiz do nosso cgroup, **relativa à raiz de leitura** — mesma detecção do
/// coletor, sem regra nova.
///
/// O ponto que já pegou um bug: o ponto de montagem é `/sys/fs/cgroup`, um
/// caminho absoluto do *host*, e a raiz pode ser um diretório de cópia. A
/// convenção é a do coletor — remove a barra inicial e junta à raiz —, então
/// a cópia cai exatamente onde o coletor procura.
fn cgroup_base(read: &Read) -> Option<String> {
    let caminho = proc::cgroup_path(&read.text("proc/self/cgroup").ok()?)?;
    let mounts = read
        .text("proc/self/mounts")
        .or_else(|_| read.text("proc/mounts"))
        .ok()?;
    let (mount, _version) = proc::cgroup_mount(&mounts)?;
    let absoluto = format!("{mount}/{caminho}");
    Some(absoluto.trim_start_matches('/').to_owned())
}

