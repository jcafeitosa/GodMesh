//! Contexto externo do snapshot no executável de testes.

use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

use godmesh_federation::{identity::is_node_id, manifest::MAX_FIELD_LEN};
use serde::Deserialize;

/// Pontos de interrupção controlados pelo ensaio, visíveis no log da fixture.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum Fault {
    #[serde(rename = "after_intent")]
    IntentPersisted,
    #[serde(rename = "after_commit")]
    SnapshotCommitted,
    #[serde(rename = "after_commit_unknown")]
    UnknownCommit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LabContext {
    pub binding: LabBinding,
    pub bootstrap_base: Option<i64>,
    pub max_control_bytes: NonZeroU32,
    pub max_snapshot_bytes: NonZeroU32,
    pub max_manifest_bytes: NonZeroU32,
    pub max_event_bytes: NonZeroU32,
    pub max_events: NonZeroU32,
    pub validity_seconds: NonZeroU32,
    pub fault: Option<Fault>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct LabBinding {
    pub database: String,
    pub database_port: NonZeroU16,
    pub generation: NonZeroU64,
    pub node_id: String,
    pub origin_node: String,
    pub cluster: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: i64,
    pub retention_class: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LabRegistration {
    binding: LabBinding,
    system_identifier: String,
    database_oid: NonZeroU32,
}

impl LabRegistration {
    pub async fn verify_database(&self, pool: &sqlx::PgPool) -> Result<(), String> {
        let (system_identifier, oid): (String, i64) = sqlx::query_as(
            "SELECT control.system_identifier::text, database.oid::bigint
             FROM pg_catalog.pg_control_system() AS control
             CROSS JOIN pg_catalog.pg_database AS database
             WHERE database.datname = pg_catalog.current_database()",
        )
        .fetch_one(pool)
        .await
        .map_err(|_| "identidade PostgreSQL do laboratório indisponível".to_owned())?;
        if system_identifier == self.system_identifier && oid == i64::from(self.database_oid.get())
        {
            Ok(())
        } else {
            Err("instância PostgreSQL diverge do registro corrente do laboratório".to_owned())
        }
    }
}

impl LabContext {
    /// Só aceita a forma de conexão emitida pelo runner PostgreSQL descartável.
    pub fn decode(
        bytes: &[u8],
        registration: &[u8],
        database_url: &str,
    ) -> Result<(Self, LabRegistration), &'static str> {
        let context: Self =
            serde_json::from_slice(bytes).map_err(|_| "contexto snapshot inválido")?;
        let registration: LabRegistration = serde_json::from_slice(registration)
            .map_err(|_| "registro corrente do laboratório inválido")?;
        let binding = &context.binding;
        if binding != &registration.binding
            || registration
                .system_identifier
                .parse::<NonZeroU64>()
                .is_err()
        {
            return Err("contexto snapshot diverge do registro corrente do laboratório");
        }
        let matches_connection = database_url
            .strip_prefix("postgresql://gm_m1c@127.0.0.1:")
            .and_then(|rest| rest.split_once('/'))
            .is_some_and(|(port, database)| {
                port.parse::<NonZeroU16>().ok() == Some(binding.database_port)
                    && database == binding.database
                    && matches!(database, "gm_m1c_a" | "gm_m1c_b" | "gm_m1c_c")
            });
        if !matches_connection {
            return Err("contexto snapshot exige o banco descartável correspondente");
        }
        let text = |value: &str| !value.trim().is_empty() && value.len() <= MAX_FIELD_LEN;
        if !is_node_id(&binding.node_id)
            || !is_node_id(&binding.origin_node)
            || !text(&binding.cluster)
            || !text(&binding.tenant_id)
            || !text(&binding.retention_class)
            || !text(&binding.stream_id)
            || !binding
                .stream_id
                .strip_prefix("m4a.metadata.")
                .is_some_and(text)
            || binding.partition != 0
            || binding.authority_epoch <= 0
            || context.bootstrap_base.is_some_and(|base| base < 0)
            || context.bootstrap_base.is_none() != (binding.node_id == binding.origin_node)
        {
            return Err("escopo do contexto snapshot inválido");
        }
        Ok((context, registration))
    }
}

#[test]
#[ignore = "executado em processo separado pela fixture A/B/C"]
fn process() {
    let path = std::env::var_os("GODMESH_TEST_SNAPSHOT_CONTEXT")
        .expect("a fixture deve fornecer o contexto externo");
    let mut config = super::Config::from_env(super::Args::Serve).expect("configuração da fixture");
    let bytes = std::fs::read(path).expect("leitura do contexto da fixture");
    let registry_path = std::env::var_os("GODMESH_TEST_SNAPSHOT_REGISTRY")
        .expect("a fixture deve fornecer o registro corrente externo");
    let registry = std::fs::read(registry_path).expect("leitura do registro corrente");
    config.snapshot_lab = Some(
        LabContext::decode(&bytes, &registry, &config.database_url).expect("contexto da fixture"),
    );
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime da fixture")
        .block_on(super::run(config))
        .expect("servidor da fixture");
}

#[cfg(test)]
mod tests {
    use super::LabContext;
    use serde_json::json;

    const LAB_DATABASE: &str = "postgresql://gm_m1c@127.0.0.1:56000/gm_m1c_b";

    fn context() -> serde_json::Value {
        json!({
            "binding": {
            "database": "gm_m1c_b",
            "database_port": 56000,
            "generation": 1,
            "node_id": format!("gmn_{}", "b".repeat(64)),
            "origin_node": format!("gmn_{}", "a".repeat(64)),
            "cluster": "cluster-fixture",
            "tenant_id": "tenant-fixture",
            "stream_id": "m4a.metadata.snapshot",
            "partition": 0,
            "authority_epoch": 7,
            "retention_class": "m4a_metadados_posicao"
            },
            "bootstrap_base": 0,
            "max_control_bytes": 8192,
            "max_snapshot_bytes": 32768,
            "max_manifest_bytes": 4096,
            "max_event_bytes": 4096,
            "max_events": 16,
            "validity_seconds": 60,
        })
    }

    fn registration(context: &serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "binding": context["binding"],
            "system_identifier": "7646167893808755902",
            "database_oid": 16405
        }))
        .unwrap()
    }

    #[test]
    fn laboratorio_exige_banco_descartavel_correspondente() {
        let context = context();
        let bytes = serde_json::to_vec(&context).unwrap();
        let current = registration(&context);
        assert!(LabContext::decode(&bytes, &current, LAB_DATABASE).is_ok());
        for database in [
            "postgresql://gm_m1c@127.0.0.1:56000/gm_m1c_c",
            "postgresql://gm_m1c@127.0.0.1:56001/gm_m1c_b",
            "postgresql://gm_m1c@192.0.2.1:56000/gm_m1c_b",
            "postgresql://outro@127.0.0.1:56000/gm_m1c_b",
            "postgresql://gm_m1c@127.0.0.1:56000/gm_m1c_b?socket=/tmp/pg",
        ] {
            assert!(LabContext::decode(&bytes, &current, database).is_err());
        }
    }

    #[test]
    fn registro_corrente_prevalece_sobre_o_contexto_de_outra_geracao() {
        let stale = context();
        let mut current = stale.clone();
        current["binding"]["generation"] = json!(2);
        let registration = registration(&current);
        let stale = serde_json::to_vec(&stale).unwrap();
        assert!(LabContext::decode(&stale, &registration, LAB_DATABASE).is_err());
        assert!(LabContext::decode(&stale, b"", LAB_DATABASE).is_err());
    }

    #[test]
    fn laboratorio_recusa_geracao_limites_ou_escopo_incoerentes() {
        for (field, invalid) in [
            ("generation", json!(0)),
            ("authority_epoch", json!(0)),
            ("partition", json!(1)),
            ("bootstrap_base", json!(-1)),
            ("max_control_bytes", json!(0)),
            ("max_manifest_bytes", json!(0)),
            ("stream_id", json!("m4a.metadata.")),
        ] {
            let mut value = context();
            if value["binding"].get(field).is_some() {
                value["binding"][field] = invalid;
            } else {
                value[field] = invalid;
            }
            let bytes = serde_json::to_vec(&value).unwrap();
            let current = registration(&value);
            assert!(
                LabContext::decode(&bytes, &current, LAB_DATABASE).is_err(),
                "{field}"
            );
        }
    }
}
