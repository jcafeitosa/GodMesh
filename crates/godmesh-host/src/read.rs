//! Raiz de leitura dos arquivos de host.
//!
//! Em produção a raiz é `/`. No teste é um diretório de fixture com a mesma
//! árvore (`proc/stat`, `proc/meminfo`, `proc/pressure/cpu`, …). Isso é o que
//! permite dizer que **o mesmo caminho de código** que roda no nó foi
//! exercitado: não existe "versão de teste" do coletor, e uma prova feita só
//! nos parsers não prova a montagem dos caminhos.

use std::io;
use std::path::{Path, PathBuf};

use crate::snapshot::Unavailable;

/// Raiz a partir da qual os caminhos relativos de `/proc` e `/sys` são lidos.
#[derive(Debug, Clone)]
pub struct Read {
    root: PathBuf,
}

impl Default for Read {
    fn default() -> Self {
        Self::new("/")
    }
}

impl Read {
    #[must_use]
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            root: if root.is_absolute() {
                root.to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(root)
            },
        }
    }

    /// Raiz efetiva, para o `--dump` declarar de onde vieram os números.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Caminho absoluto de um arquivo relativo à raiz.
    #[must_use]
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative.strip_prefix('/').unwrap_or(relative))
    }

    /// Conteúdo de um arquivo como texto, classificando a falha.
    ///
    /// A classificação importa porque as três falhas significam coisas
    /// diferentes para quem lê o painel:
    ///  * `Missing` — o kernel/SO não expõe aquilo (PSI sem `CONFIG_PSI`,
    ///    `/proc` em macOS). Degradar é o comportamento correto.
    ///  * `PermissionDenied` — existe e é nosso direito ler com privilégio
    ///    (`nf_conntrack_count` em alguns kernels). Declara-se o requisito.
    ///  * `Io` — o arquivo apareceu e falhou no meio (pseudofs sob pressão).
    pub fn text(&self, relative: &str) -> Result<String, Unavailable> {
        let path = self.path(relative);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(text),
            Err(e) => Err(unavailable_from(&e)),
        }
    }

    /// [`Self::text`] com `trim`: os arquivos escalares de `/proc` terminam em
    /// newline, e `max` de cgroup é o formato normal do arquivo.
    pub fn token(&self, relative: &str) -> Result<String, Unavailable> {
        Ok(self.text(relative)?.trim().to_owned())
    }

    /// Nomes do diretório — usado para contar `fd` e achar diretórios de
    /// cgroup. Falha de permissão é distinta de diretório inexistente.
    pub fn entries(&self, relative: &str) -> Result<Vec<String>, Unavailable> {
        let path = self.path(relative);
        let dir = std::fs::read_dir(&path).map_err(|e| unavailable_from(&e))?;
        let mut out = Vec::new();
        for entry in dir {
            match entry {
                Ok(entry) => out.push(entry.file_name().to_string_lossy().into_owned()),
                Err(e) => return Err(unavailable_from(&e)),
            }
        }
        Ok(out)
    }

    #[must_use]
    pub fn exists(&self, relative: &str) -> bool {
        self.path(relative).exists()
    }
}

/// Só o `kind()` nos dá a distinção sem `unsafe` e sem `errno` cru.
fn unavailable_from(error: &io::Error) -> Unavailable {
    match error.kind() {
        io::ErrorKind::NotFound => Unavailable::Missing,
        io::ErrorKind::PermissionDenied => Unavailable::PermissionDenied,
        other => Unavailable::Io(other.to_string()),
    }
}
