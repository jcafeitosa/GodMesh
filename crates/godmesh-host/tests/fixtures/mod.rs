//! Acesso às fixtures de `/proc` pelos testes.
//!
//! `fixture_root("linux")` devolve um `PathBuf` para a raiz da árvore, que é o
//! que se passa a [`godmesh_host::read::Read`] — o coletor então lê
//! `linux/proc/stat` exatamente como leria `/proc/stat`. Não há segunda
//! implementação "de teste" da coleta.
//!
//! `temp_copy` cria uma cópia mutável da árvore quando o teste precisa mudar
//! um arquivo entre duas coletas (janela de CPU) ou simular falha de leitura.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Raiz da árvore de fixtures.
#[must_use]
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Raiz de uma fixture nomeada (`linux`, `container`, …).
#[must_use]
pub fn fixture_root(name: &str) -> PathBuf {
    let path = fixtures_dir().join(name);
    assert!(path.is_dir(), "fixture {name} não existe em {}", path.display());
    path
}

/// Conteúdo de um arquivo da fixture, para asserções sobre parsers.
#[must_use]
pub fn read(name: &str, relative: &str) -> String {
    let path = fixture_root(name).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Cópia temporária e mutável de uma fixture. Remove-se no `Drop`.
///
/// O nome leva o PID e um contador para que testes em paralelo não se
/// enxerguem.
pub struct TempFixture {
    path: PathBuf,
}

impl TempFixture {
    #[must_use]
    pub fn new(name: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "godmesh-host-fixture-{}-{}-{seq}-{}",
            std::process::id(),
            name,
            uuid_like()
        ));
        copy_dir(&fixture_root(name), &path);
        Self { path }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.path
    }

    /// Substitui o conteúdo de um arquivo da cópia.
    pub fn write(&self, relative: &str, content: &str) {
        let path = self.path.join(relative);
        std::fs::create_dir_all(path.parent().expect("arquivo com diretório"))
            .expect("cria diretório");
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }

    /// Remove o arquivo da cópia: simula o kernel que não expõe aquilo (PSI
    /// sem `CONFIG_PSI`, conntrack sem módulo, `/proc` sem `tcp6`).
    pub fn remove(&self, relative: &str) {
        std::fs::remove_file(self.path.join(relative))
            .unwrap_or_else(|e| panic!("remove {relative}: {e}"));
    }

    /// Torna o arquivo ilegível por permissão, e devolve `false` quando o
    /// processo de teste é privilegiado demais para que `chmod 000` impeça a
    /// leitura (root). Nesse caso o chamador deve **remover** o arquivo e
    /// dizer por quê, em vez de fingir que provou a falha de permissão.
    pub fn make_unreadable(&self, relative: &str) -> bool {
        let path = self.path.join(relative);
        if std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).is_err() {
            return false;
        }
        std::fs::read_to_string(&path).is_err()
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// `chmod` de `std::fs::Permissions`, que é só em unix.
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Identificador do diretório temporário: hash do relógio + PID.
///
/// Sem `rand`: o nome só precisa ser único entre testes em paralelo, e `u128`
/// de `RandomState` com o instante é suficiente e não usa `unsafe`.
fn uuid_like() -> u128 {
    use std::hash::{BuildHasher, Hasher};
    let hash = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish() as u128;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    hash ^ nanos
}

/// Cópia recursiva que preserva o formato dos arquivos (nenhum é processado).
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("{}: {e}", to.display()));
    for entry in
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("{}: {e}", from.display()))
    {
        let entry = entry.expect("entrada de diretório");
        let target = to.join(entry.file_name());
        let file_type = entry.file_type().expect("tipo de entrada");
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copia arquivo de fixture");
        }
    }
}

/// Fonte de volumes fixa para os testes de coleta: `statfs` não tem fixture.
///
/// Os números são os de um contêiner típico de CI (raiz de 40 GiB com 20 GiB
/// livres), o que permite conferir a razão de espaço livre sem ler o disco.
#[must_use]
pub fn volumes() -> Vec<godmesh_host::snapshot::Volume> {
    vec![
        godmesh_host::snapshot::Volume {
            mount_point: "/".to_owned(),
            file_system: "ext4".to_owned(),
            total_bytes: 40 * 1024 * 1024 * 1024,
            available_bytes: 20 * 1024 * 1024 * 1024,
            read_only: false,
        },
        godmesh_host::snapshot::Volume {
            mount_point: "/var/lib/godmesh".to_owned(),
            file_system: "xfs".to_owned(),
            total_bytes: 100 * 1024 * 1024 * 1024,
            available_bytes: 4 * 1024 * 1024 * 1024,
            read_only: false,
        },
    ]
}

#[must_use]
pub fn collector(name: &str) -> godmesh_host::collect::Collector {
    godmesh_host::collect::Collector::with_read(
        godmesh_host::read::Read::new(fixture_root(name)),
        std::sync::Arc::new(godmesh_host::collect::StaticVolumes(volumes())),
    )
}

/// `true` se a família existe como **série**, não apenas citada numa `HELP`.
///
/// Detalhado de propósito: um teste que procura o nome da série em texto
/// puro quebra quando outra família menciona aquela série na documentação, e
/// isso é exatamente o tipo de asserção que faria um painel correto parecer
/// errado.
#[must_use]
pub fn tem_serie(texto: &str, familia: &str) -> bool {
    texto.lines().any(|line| {
        !line.starts_with('#') && (line.starts_with(&format!("{familia} ")) || line.starts_with(&format!("{familia}{{")))
    })
}

#[must_use]
pub fn collector_at(path: &Path) -> godmesh_host::collect::Collector {
    godmesh_host::collect::Collector::with_read(
        godmesh_host::read::Read::new(path),
        std::sync::Arc::new(godmesh_host::collect::StaticVolumes(volumes())),
    )
}
