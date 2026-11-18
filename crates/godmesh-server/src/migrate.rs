//! Migrações aplicadas na inicialização, em ordem de nome de arquivo.
//!
//! Formato do controle — **`schema_migration` v1**:
//!
//! ```sql
//! CREATE TABLE schema_migration (
//!     nome       text        PRIMARY KEY,   -- nome do arquivo, ex.: 0001_core.sql
//!     checksum   text        NOT NULL,      -- sha256 do conteúdo do arquivo (hex)
//!     aplicado_em timestamptz NOT NULL DEFAULT now()
//! );
//! ```
//!
//! Regras:
//! * `migrations/*.sql` são aplicadas em ordem lexicográfica do nome;
//! * rodar de novo é idempotente: migração com o mesmo nome e o mesmo checksum é
//!   ignorada, e `aplicado_em` não é reescrito;
//! * nome conhecido com **checksum diferente** falha alto — histórico que
//!   divergiu não é "atualizado", é erro (a correção vai em migração nova);
//! * cada arquivo + o INSERT de controle são enviados ao servidor em **um único
//!   script**, que o Postgres executa como um bloco transacional implícito
//!   (`raw_sql` → protocolo simples): ou a migração inteira entra, ou nada entra;
//!   por isso os arquivos de migração **não** abrem `BEGIN`/`COMMIT` — o runner é
//!   dono da transação;
//! * falha em qualquer migração é erro, nunca aviso: o processo não sobe;
//! * um advisory lock serializa processos migrando o mesmo banco ao mesmo tempo.
//!
//! O checksum é calculado pelo próprio Postgres (`sha256` embutido desde o
//! Postgres 11) para não acrescentar dependência de hash ao binário.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use sqlx::PgPool;
use sqlx::postgres::PgConnection;

/// Variável de ambiente que aponta o diretório das migrações.
pub const MIGRATIONS_DIR_ENV: &str = "GODMESH_MIGRATIONS_DIR";

/// Diretório padrão, relativo ao diretório de trabalho do processo.
pub const DEFAULT_MIGRATIONS_DIR: &str = "migrations";

/// Versão do formato da tabela de controle. Mudança de formato = nova versão.
pub const SCHEMA_VERSION: &str = "schema_migration v1";

/// Chave fixa do advisory lock (`"godmesi"` em ASCII): migrações de processos
/// diferentes no mesmo banco se serializam em vez de disputar DDL.
const ADVISORY_LOCK_KEY: i64 = 0x0067_6f64_6d65_7369;

/// Resultado de uma execução do runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Diretório lido.
    pub dir: PathBuf,
    /// Migrações aplicadas nesta execução, em ordem.
    pub applied: Vec<String>,
    /// Migrações já aplicadas com o mesmo checksum, em ordem.
    pub skipped: Vec<String>,
}

impl MigrationReport {
    /// Quantidade de migrações que o banco já tinha.
    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }
}

/// Falha do runner. Toda variante impede o processo de subir.
#[derive(Debug)]
pub enum MigrationError {
    /// O diretório não existe ou não pôde ser lido.
    DirUnreadable { dir: PathBuf, detail: String },
    /// O diretório existe mas não tem nenhum `*.sql`: vazio não é sucesso.
    Empty { dir: PathBuf },
    /// Nome de arquivo fora de `[A-Za-z0-9._-]` (vai para o SQL de controle).
    InvalidName { name: String },
    /// Arquivo não pôde ser lido ou não é UTF-8.
    Read { name: String, detail: String },
    /// Falha de banco em uma etapa nomeada.
    Database { step: &'static str, detail: String },
    /// Arquivo já aplicado foi editado: o histórico divergiu.
    ChecksumDrift {
        name: String,
        gravado: String,
        atual: String,
    },
    /// O banco já tem os objetos da primeira migração, mas não tem histórico:
    /// foi criado antes da tabela de controle.
    LegacyDatabase { name: String, detail: String },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirUnreadable { dir, detail } => write!(
                f,
                "não foi possível ler o diretório de migrações {}: {detail}",
                dir.display()
            ),
            Self::Empty { dir } => write!(
                f,
                "nenhuma migração *.sql encontrada em {}; diretório vazio não é sucesso",
                dir.display()
            ),
            Self::InvalidName { name } => write!(
                f,
                "nome de migração inválido: {name}; use apenas [A-Za-z0-9._-].sql"
            ),
            Self::Read { name, detail } => {
                write!(f, "não foi possível ler a migração {name}: {detail}")
            }
            Self::Database { step, detail } => {
                write!(f, "falha de banco durante {step}: {detail}")
            }
            Self::ChecksumDrift {
                name,
                gravado,
                atual,
            } => write!(
                f,
                "migração {name} já aplicada com checksum {gravado}, mas o arquivo atual tem \
                 {atual}; o histórico divergiu — a correção vai em migração nova, não editando \
                 a antiga"
            ),
            Self::LegacyDatabase { name, detail } => write!(
                f,
                "a primeira migração ({name}) falhou porque o banco já contém os objetos dela, \
                 mas não há registro em schema_migration: banco criado antes do controle de \
                 migrações. Não há adoção automática de histórico — em dev, recrie o banco \
                 ({detail})"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

/// Diretório de migrações resolvido do ambiente, com default `migrations`.
#[must_use]
pub fn dir_from_env() -> PathBuf {
    match std::env::var(MIGRATIONS_DIR_ENV) {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => PathBuf::from(DEFAULT_MIGRATIONS_DIR),
    }
}

/// Aplica as migrações pendentes de `dir` no banco do `pool`.
///
/// # Errors
/// Diretório ausente/vazio, arquivo ilegível, nome inválido, falha de SQL ou
/// divergência de checksum de migração já aplicada. Em qualquer caso o banco
/// fica sem a migração que falhou (bloco transacional implícito).
pub async fn migrate(pool: &PgPool, dir: &Path) -> Result<MigrationReport, MigrationError> {
    let files = collect(dir)?;
    let mut conn = pool.acquire().await.map_err(|e| MigrationError::Database {
        step: "obter conexão para migrar",
        detail: e.to_string(),
    })?;

    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .map_err(|e| MigrationError::Database {
            step: "travar as migrações (advisory lock)",
            detail: e.to_string(),
        })?;

    let outcome = run_all(&mut conn, dir, &files).await;

    // O lock é de sessão: devolver a conexão ao pool sem soltá-lo travaria a
    // próxima execução. A liberação é tentativa; o erro que importa é o de cima.
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .execute(&mut *conn)
        .await;

    outcome
}

/// Lê o diretório e devolve os nomes `*.sql` em ordem lexicográfica.
fn collect(dir: &Path) -> Result<Vec<String>, MigrationError> {
    let entries = std::fs::read_dir(dir).map_err(|e| MigrationError::DirUnreadable {
        dir: dir.to_path_buf(),
        detail: e.to_string(),
    })?;

    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| MigrationError::DirUnreadable {
            dir: dir.to_path_buf(),
            detail: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("sql") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| MigrationError::InvalidName {
                name: path.display().to_string(),
            })?
            .to_owned();
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(MigrationError::InvalidName { name });
        }
        names.push(name);
    }

    names.sort();
    if names.is_empty() {
        return Err(MigrationError::Empty {
            dir: dir.to_path_buf(),
        });
    }
    Ok(names)
}

/// Aplica uma migração pendente de cada vez, mantendo o controle em
/// `schema_migration`.
async fn run_all(
    conn: &mut PgConnection,
    dir: &Path,
    files: &[String],
) -> Result<MigrationReport, MigrationError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migration (\
             nome        text        PRIMARY KEY,\
             checksum    text        NOT NULL,\
             aplicado_em timestamptz NOT NULL DEFAULT now()\
         )",
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| MigrationError::Database {
        step: "criar a tabela de controle schema_migration",
        detail: e.to_string(),
    })?;

    let rows: Vec<(String, String)> = sqlx::query_as("SELECT nome, checksum FROM schema_migration")
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| MigrationError::Database {
            step: "ler a tabela de controle schema_migration",
            detail: e.to_string(),
        })?;
    let existing: BTreeMap<String, String> = rows.into_iter().collect();

    let mut report = MigrationReport {
        dir: dir.to_path_buf(),
        applied: Vec::new(),
        skipped: Vec::new(),
    };

    for name in files {
        let path = dir.join(name);
        let sql = std::fs::read_to_string(&path).map_err(|e| MigrationError::Read {
            name: name.clone(),
            detail: e.to_string(),
        })?;
        let checksum = checksum_of(conn, &sql).await?;

        if let Some(gravado) = existing.get(name) {
            if *gravado != checksum {
                return Err(MigrationError::ChecksumDrift {
                    name: name.clone(),
                    gravado: gravado.clone(),
                    atual: checksum,
                });
            }
            report.skipped.push(name.clone());
            continue;
        }

        apply(conn, name, &sql, &checksum).await.map_err(|e| {
            // Só a primeira migração da execução pode denunciar banco pré-controle:
            // se algo já foi aplicado, o histórico existe e o erro é outro.
            match e {
                MigrationError::Database { detail, .. }
                    if report.applied.is_empty() && detail.contains("already exists") =>
                {
                    MigrationError::LegacyDatabase {
                        name: name.clone(),
                        detail,
                    }
                }
                other => other,
            }
        })?;
        report.applied.push(name.clone());
    }

    Ok(report)
}

/// sha256 (hex) do conteúdo, calculado pelo Postgres.
async fn checksum_of(conn: &mut PgConnection, sql: &str) -> Result<String, MigrationError> {
    sqlx::query_scalar("SELECT encode(sha256(convert_to($1, 'UTF8')), 'hex')")
        .bind(sql)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| MigrationError::Database {
            step: "calcular o checksum da migração",
            detail: e.to_string(),
        })
}

/// Envia migração + registro de controle em um único script: um bloco
/// transacional implícito no servidor.
async fn apply(
    conn: &mut PgConnection,
    name: &str,
    sql: &str,
    checksum: &str,
) -> Result<(), MigrationError> {
    let script = format!(
        "{sql}\n;\nINSERT INTO schema_migration (nome, checksum) VALUES ('{name}', '{checksum}');\n"
    );
    // `AssertSqlSafe` é deliberado e auditado: o único texto interpolado é o nome
    // do arquivo (validado em `[A-Za-z0-9._-]` por `collect`) e o checksum em hex
    // produzido pelo próprio Postgres. O corpo do SQL é o arquivo do repositório,
    // não entrada de rede.
    sqlx::raw_sql(sqlx::AssertSqlSafe(script))
        .execute(&mut *conn)
        .await
        .map_err(|e| MigrationError::Database {
            step: "aplicar a migração e registrar o controle",
            detail: format!("{name}: {e}"),
        })?;
    Ok(())
}
