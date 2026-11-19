//! Contexto externo do snapshot no executável de testes.

use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

use godmesh_federation::{identity::is_node_id, manifest::MAX_FIELD_LEN};
use serde::Deserialize;

/// Pontos de interrupção controlados pelo ensaio, visíveis no log da fixture.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Fault {
    AfterIntent,
    AfterCommit,
    AfterCommitUnknown,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LabContext {
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
    pub bootstrap_base: Option<i64>,
    pub max_control_bytes: NonZeroU32,
    pub max_snapshot_bytes: NonZeroU32,
    pub max_manifest_bytes: NonZeroU32,
    pub max_event_bytes: NonZeroU32,
    pub max_events: NonZeroU32,
    pub validity_seconds: NonZeroU32,
    pub fault: Option<Fault>,
}

impl LabContext {
    /// Só aceita a forma de conexão emitida pelo runner PostgreSQL descartável.
    pub fn decode(bytes: &[u8], database_url: &str) -> Result<Self, &'static str> {
        let context: Self =
            serde_json::from_slice(bytes).map_err(|_| "contexto snapshot inválido")?;
        let matches_connection = database_url
            .strip_prefix("postgresql://gm_m1c@127.0.0.1:")
            .and_then(|rest| rest.split_once('/'))
            .is_some_and(|(port, database)| {
                port.parse::<NonZeroU16>().ok() == Some(context.database_port)
                    && database == context.database
                    && matches!(database, "gm_m1c_a" | "gm_m1c_b" | "gm_m1c_c")
            });
        if !matches_connection {
            return Err("contexto snapshot exige o banco descartável correspondente");
        }
        let text = |value: &str| !value.trim().is_empty() && value.len() <= MAX_FIELD_LEN;
        if !is_node_id(&context.node_id)
            || !is_node_id(&context.origin_node)
            || !text(&context.cluster)
            || !text(&context.tenant_id)
            || !text(&context.retention_class)
            || !text(&context.stream_id)
            || !context
                .stream_id
                .strip_prefix("m4a.metadata.")
                .is_some_and(text)
            || context.partition != 0
            || context.authority_epoch <= 0
            || context.bootstrap_base.is_some_and(|base| base < 0)
            || context.bootstrap_base.is_none() != (context.node_id == context.origin_node)
        {
            return Err("escopo do contexto snapshot inválido");
        }
        Ok(context)
    }
}

#[test]
#[ignore = "executado em processo separado pela fixture A/B/C"]
fn process() {
    let path = std::env::var_os("GODMESH_TEST_SNAPSHOT_CONTEXT")
        .expect("a fixture deve fornecer o contexto externo");
    let mut config = super::Config::from_env(super::Args::Serve).expect("configuração da fixture");
    let bytes = std::fs::read(path).expect("leitura do contexto da fixture");
    config.snapshot_lab =
        Some(LabContext::decode(&bytes, &config.database_url).expect("contexto da fixture"));
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
            "retention_class": "m4a_metadados_posicao",
            "bootstrap_base": 0,
            "max_control_bytes": 8192,
            "max_snapshot_bytes": 32768,
            "max_manifest_bytes": 4096,
            "max_event_bytes": 4096,
            "max_events": 16,
            "validity_seconds": 60,
        })
    }

    #[test]
    fn laboratorio_exige_banco_descartavel_correspondente() {
        let bytes = serde_json::to_vec(&context()).unwrap();
        assert!(LabContext::decode(&bytes, LAB_DATABASE).is_ok());
        for database in [
            "postgresql://gm_m1c@127.0.0.1:56000/gm_m1c_c",
            "postgresql://gm_m1c@127.0.0.1:56001/gm_m1c_b",
            "postgresql://gm_m1c@192.0.2.1:56000/gm_m1c_b",
            "postgresql://outro@127.0.0.1:56000/gm_m1c_b",
            "postgresql://gm_m1c@127.0.0.1:56000/gm_m1c_b?socket=/tmp/pg",
        ] {
            assert!(LabContext::decode(&bytes, database).is_err());
        }
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
            value[field] = invalid;
            let bytes = serde_json::to_vec(&value).unwrap();
            assert!(LabContext::decode(&bytes, LAB_DATABASE).is_err(), "{field}");
        }
    }
}
