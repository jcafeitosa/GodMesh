//! Uso de espaço por volume montado.
//!
//! Por que `sysinfo` aqui e nada em `/proc`: espaço livre de um filesystem é
//! `statfs(2)`, que `/proc` **não** expõe de forma nenhuma. Chamar `statfs`
//! direto exigiria `unsafe`, proibido no workspace ([`crate`] `/AGENTS.md`),
//! então a leitura vem do wrapper seguro já resolvido por ADR-073 —
//! `sysinfo` 0.40.4, feature `disk` apenas (crates.io, consulta 18/11/2026).
//!
//! O resto do painel (CPU, memória, PSI, conexões, interfaces, limites) é
//! `/proc` puro, onde o crate não agrega valor e só adicionaria uma abstração
//! entre o número e a documentação do kernel.
//!
//! **inodes não são publicados.** `statfs` devolve `f_files`/`f_ffree`, mas
//! `sysinfo` 0.40.4 não expõe esse par, e `sysfs`/`/proc` não têm substituto.
//! Preferimos a série ausente e documentada à introdução de uma dependência
//! nova por dois números (ver `docs/09-equipe/evidencias/2026-11-18-host-telemetria.md`
//! § lacunas).

/// Tetos de cardinalidade (ADR-073: agregar por volume/interface, nunca por
/// conexão). São escolhas *desta* fatia, não parâmetros aprovados: ficam
/// visíveis aqui para a próxima revisão poder discordar com um número.
pub const MAX_VOLUMES: usize = 16;
pub const MAX_INTERFACES: usize = 16;

use crate::snapshot::Volume;

/// Filesystems que não são capacidade de disco.
///
/// A lista é por tipo de filesystem, não por ponto de montagem: um ponto pode
/// ser qualquer string do operador, o tipo é o que o kernel declara. Em um
/// contêiner típico isso remove `proc`, `sysfs`, `devtmpfs`, `cgroup2` e
/// `mqueue`, e mantém `overlay` — que é o `/` realmente escrevível do
/// contêiner.
const PSEUDO_FILE_SYSTEMS: &[&str] = &[
    "autofs",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fuse.gvfsd-fuse",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "proc",
    "ramfs",
    "rpc_pipefs",
    "securityfs",
    "selinuxfs",
    "squashfs",
    "sysfs",
    "tmpfs",
    "tracefs",
];

/// Lê os volumes reais da máquina.
///
/// Falha de listagem devolve lista **vazia**, que o coletor traduz em
/// `group_available{group="disk"} 0`. Não há valor padrão honesto para "não
/// consegui perguntar ao kernel".
#[must_use]
pub fn volumes() -> Vec<Volume> {
    match sysinfo::Disks::new_with_refreshed_list() {
        Ok(disks) => build(&disks),
        Err(_) => Vec::new(),
    }
}

/// Separa a filtragem da chamada de sistema, para o teste poder cobrir a
/// regra de pseudo-fs sem depender do disco do CI.
#[must_use]
pub fn build(disks: &[sysinfo::Disk]) -> Vec<Volume> {
    let mut out: Vec<Volume> = disks
        .iter()
        .filter(|disk| {
            let file_system = disk.file_system().to_string_lossy();
            !is_pseudo(&file_system) && !file_system.starts_with("fuse.")
        })
        .map(|disk| Volume {
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
            read_only: disk.is_read_only(),
        })
        .collect();
    out.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    out
}

#[must_use]
pub fn is_pseudo(file_system: &str) -> bool {
    PSEUDO_FILE_SYSTEMS.contains(&file_system)
}

#[cfg(test)]
mod tests {
    use super::is_pseudo;

    #[test]
    fn pseudo_fs_nao_vira_volume() {
        for fs in ["proc", "sysfs", "tmpfs", "cgroup2", "devtmpfs", "squashfs"] {
            assert!(is_pseudo(fs), "{fs} deveria ser filtrado");
        }
    }

    #[test]
    fn fs_reais_sao_volume() {
        for fs in ["ext4", "xfs", "btrfs", "zfs", "apfs", "overlay", "vfat"] {
            assert!(!is_pseudo(fs), "{fs} é capacidade real");
        }
    }
}
