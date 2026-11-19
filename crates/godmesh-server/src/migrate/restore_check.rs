//! Verificação somente leitura da base provisionada para restore de laboratório.

use std::collections::BTreeMap;

use sqlx::PgConnection;

use super::{MigrationError, checksum_of};

macro_rules! embedded_migrations {
    ($($name:literal),+ $(,)?) => {
        &[$(($name, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations/", $name)))),+]
    };
}

const MIGRATIONS: &[(&str, &str)] = embedded_migrations![
    "0001_core.sql",
    "0002_incident_tenancy.sql",
    "0003_outbox_publisher.sql",
    "0004_grant_revocation.sql",
    "0005_sync_inbox.sql",
    "0006_incident_listing.sql",
    "0007_grant_issuance.sql",
    "0008_federation_peers.sql",
    "0009_federation_approval.sql",
    "0010_event_signed_ingest.sql",
    "0011_federation_decision_epoch.sql",
    "0012_federation_portable_join.sql",
    "0013_federation_config_bundle.sql",
    "0014_federation_replication.sql",
    "0015_federation_replica_resume.sql",
    "0016_federation_snapshot_receipt.sql",
    "0017_federation_snapshot_intent.sql",
];

// Colunas das tabelas de 0001–0017, conferidas no catálogo de uma base
// criada do zero. O servidor reabre as rotas e os gates já existentes
// após promover, então o boot exige esses objetos antes do primeiro bind.
// As tabelas exclusivas do laboratório são validadas por seu serviço.
const REQUIRED: &[(&str, &str)] = &[
    (
        "event_position",
        "origin_node tenant_id stream_id partition authority_epoch seq retention_class jurisdiction recorded_at",
    ),
    (
        "federation_config_audit",
        "id hash happened_at transition actor",
    ),
    (
        "federation_config_frame",
        "hash series version frame applies_at expires_at effective_after floor software signers accepted incompatible resolution",
    ),
    (
        "federation_config_policy",
        "singleton profile cluster namespace scope_id epoch role voters",
    ),
    (
        "federation_config_state",
        "singleton accepted incompatible pending effective published conflict recovering",
    ),
    (
        "federation_decision_epoch",
        "approver_node_id decision_epoch",
    ),
    (
        "federation_join_request",
        "join_request_id node_id epoch manifest requested_at approved_at approved_by state approved_by_node approval_signature approval_epoch approval_expires_at",
    ),
    (
        "federation_peer",
        "node_id peer_id public_key cluster region trust_state endpoints software_version capabilities epoch last_seen_at updated_at",
    ),
    (
        "federation_peer_scope",
        "node_id tenant_id stream_id granted_at",
    ),
    (
        "federation_portable_decisions",
        "join_request_id cluster root_node_id signer_node_id leaf_digest digests chain_json decided_at approved_by",
    ),
    (
        "federation_portable_epoch",
        "cluster root_node_id signer_node_id epoch",
    ),
    (
        "federation_portable_imports",
        "cluster node_id public_key root_node_id origin peer_node_id manifest chain_json digests validity issued_at expires_at",
    ),
    (
        "federation_portable_revocations",
        "cluster root_node_id target_digest revocation_digest target_chain_json revocation receipt",
    ),
    (
        "federation_portable_roots",
        "cluster root_node_id public_key",
    ),
    (
        "federation_portable_validity",
        "cluster root_node_id leaf_digest issued_at expires_at validity",
    ),
    (
        "federation_replica_acl",
        "origin_node tenant_id stream_id partition authority_epoch destination direction retention_class class_version jurisdiction copy_kind policy_id policy_version not_before expires_at revoked",
    ),
    (
        "federation_replica_checkpoint",
        "local_node origin_node tenant_id stream_id partition authority_epoch prefix",
    ),
    (
        "federation_replica_disposition",
        "origin_node tenant_id stream_id partition authority_epoch seq removed_at",
    ),
    (
        "federation_replica_hold",
        "origin_node tenant_id stream_id partition authority_epoch seq active",
    ),
    (
        "federation_replica_namespace",
        "origin_node tenant_id stream_id partition authority_epoch local_node profile profile_version retention_class class_version jurisdiction policy_id policy_version",
    ),
    (
        "federation_replica_policy",
        "policy_id policy_version tenant_id retention_class class_version jurisdiction copy_node copy_kind purpose trigger_event duration_seconds allow_discard hold_mode active",
    ),
    (
        "federation_replica_proof",
        "origin_node tenant_id stream_id partition authority_epoch seq frame issued_at expires_at policy_id policy_version published discard_allowed",
    ),
    (
        "federation_replica_resume_block",
        "local_node origin_node tenant_id stream_id partition authority_epoch blocked_next_seq",
    ),
    (
        "federation_snapshot_intent_detail",
        "local_node origin_node tenant_id stream_id partition authority_epoch attempt_id base_seq cut_seq event_count manifest_digest package_digest events_digest not_before signed_expires_at created_at detail_expires_at policy_id policy_version jurisdiction",
    ),
    (
        "federation_snapshot_intent_marker",
        "local_node origin_node tenant_id stream_id partition authority_epoch attempt_id state",
    ),
    (
        "federation_snapshot_intent_policy",
        "policy_id policy_version local_node origin_node tenant_id stream_id partition authority_epoch jurisdiction purpose duration_seconds allow_reduce active",
    ),
    (
        "federation_snapshot_receipt",
        "local_node origin_node tenant_id stream_id partition authority_epoch cut_seq base_seq event_count manifest_digest package_digest policy_id policy_version applied_at proof_expires_at expires_at jurisdiction",
    ),
    (
        "federation_snapshot_receipt_policy",
        "policy_id policy_version local_node origin_node tenant_id stream_id partition authority_epoch jurisdiction purpose duration_seconds active",
    ),
    (
        "grant_issued",
        "token_id tenant_id principal token_hash scopes not_before expires_at authority_epoch created_at",
    ),
    ("grant_revocation", "token_id revoked_at reason"),
    (
        "outbox_entry",
        "origin_node tenant_id stream_id partition authority_epoch seq effect_kind published_at confirmed_at attempts",
    ),
    (
        "outbox_sink",
        "origin_node tenant_id stream_id partition authority_epoch seq effect_kind delivered_at",
    ),
    (
        "retention_class",
        "class jurisdiction finality months discard_trigger",
    ),
    ("retention_copy", "class jurisdiction copy_scope"),
    ("schema_migration", "nome checksum aplicado_em"),
    (
        "sos_ack",
        "incident_id ack_class source at_unix recorded_at",
    ),
    (
        "sos_incident",
        "id tenant_id principal state reason created_at updated_at retention_class retention_jurisdiction retention_policy_id list_seq",
    ),
    (
        "sync_inbox_state",
        "tenant_id origin_node stream_id partition authority_epoch contiguous_prefix updated_at",
    ),
    ("tenant_region", "tenant_id jurisdiction declared_at"),
];

fn invalid_schema(detail: impl Into<String>) -> MigrationError {
    MigrationError::Database {
        step: "verificar o schema do restore",
        detail: detail.into(),
    }
}

/// Confere o catálogo e o histórico já provisionados pelo administrador. Usa
/// a mesma sessão do preflight, sem executar o runner ou emitir DDL.
///
/// # Errors
/// Falha se o catálogo não puder ser lido, se faltar objeto exigido ou se o
/// histórico divergir da lista e dos checksums embutidos neste binário.
pub async fn verify_restore_schema(connection: &mut PgConnection) -> Result<(), MigrationError> {
    let (tables, fields): (Vec<_>, Vec<_>) = REQUIRED
        .iter()
        .flat_map(|&(table, fields)| {
            fields
                .split_ascii_whitespace()
                .map(move |field| (table, field))
        })
        .unzip();
    let missing: Option<(String, String)> = sqlx::query_as(
        "SELECT wanted.relation, wanted.field
         FROM unnest($1::text[], $2::text[]) WITH ORDINALITY
             AS wanted(relation, field, ordinal)
         WHERE NOT EXISTS (
             SELECT 1 FROM pg_catalog.pg_class AS relation
             JOIN pg_catalog.pg_namespace AS namespace ON namespace.oid = relation.relnamespace
             JOIN pg_catalog.pg_attribute AS field ON field.attrelid = relation.oid
             WHERE namespace.nspname = 'public'
               AND relation.relkind = 'r' AND relation.relpersistence = 'p'
               AND relation.relname::text = wanted.relation
               AND field.attname::text = wanted.field AND field.attnum > 0 AND NOT field.attisdropped
         )
         ORDER BY wanted.ordinal LIMIT 1",
    )
    .bind(tables)
    .bind(fields)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|error| invalid_schema(error.to_string()))?;
    if let Some((table, field)) = missing {
        return Err(invalid_schema(format!(
            "objeto obrigatório ausente: {table}.{field}"
        )));
    }

    let history: Vec<(String, String)> =
        sqlx::query_as("SELECT nome, checksum FROM public.schema_migration ORDER BY nome")
            .fetch_all(&mut *connection)
            .await
            .map_err(|error| invalid_schema(error.to_string()))?;
    let mut installed = BTreeMap::new();
    for (name, checksum) in history {
        if installed.insert(name.clone(), checksum).is_some() {
            return Err(invalid_schema(format!("migração duplicada: {name}")));
        }
    }
    for &(name, sql) in MIGRATIONS {
        let gravado = installed
            .remove(name)
            .ok_or_else(|| invalid_schema(format!("migração pendente: {name}")))?;
        let atual = checksum_of(connection, sql).await?;
        if gravado != atual {
            return Err(MigrationError::ChecksumDrift {
                name: name.to_owned(),
                gravado,
                atual,
            });
        }
    }
    if let Some((name, _)) = installed.first_key_value() {
        return Err(invalid_schema(format!("migração inesperada: {name}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{path::Path, str::FromStr};

    use sqlx::{AssertSqlSafe, postgres::PgConnectOptions, postgres::PgPoolOptions};

    use super::verify_restore_schema;
    use crate::migrate;

    #[tokio::test]
    async fn restore_check_accepts_a_migrated_database_without_ddl_privileges() {
        let Ok(url) = std::env::var("GODMESH_TEST_DATABASE_URL") else {
            eprintln!("restore_check requer GODMESH_TEST_DATABASE_URL descartável");
            return;
        };
        let options = PgConnectOptions::from_str(&url).expect("URL de teste");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .expect("conectar ao PostgreSQL descartável");
        let suffix = std::process::id();
        let database = format!("restore_schema_check_{suffix}");
        let role = format!("restore_schema_reader_{suffix}");
        sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {database}")))
            .execute(&admin)
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("CREATE ROLE {role} NOLOGIN")))
            .execute(&admin)
            .await
            .unwrap();
        let target = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.database(&database))
            .await
            .unwrap();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
        migrate::migrate(&target, &dir).await.unwrap();
        sqlx::raw_sql(AssertSqlSafe(format!(
            "REVOKE CREATE ON SCHEMA public FROM PUBLIC;
             GRANT USAGE ON SCHEMA public TO {role};
             GRANT SELECT ON public.schema_migration TO {role};"
        )))
        .execute(&target)
        .await
        .unwrap();

        let mut admin_conn = target.acquire().await.unwrap();
        sqlx::query(AssertSqlSafe(format!("SET ROLE {role}")))
            .execute(&mut *admin_conn)
            .await
            .unwrap();
        let can_create: bool =
            sqlx::query_scalar("SELECT has_schema_privilege(current_user, 'public', 'CREATE')")
                .fetch_one(&mut *admin_conn)
                .await
                .unwrap();
        assert!(!can_create);
        verify_restore_schema(&mut admin_conn).await.unwrap();
        sqlx::query("RESET ROLE")
            .execute(&mut *admin_conn)
            .await
            .unwrap();

        for (mutation, reason) in [
            (
                "DELETE FROM public.schema_migration WHERE nome LIKE '0017%'",
                "migração pendente",
            ),
            (
                "INSERT INTO public.schema_migration(nome, checksum) VALUES ('9999_unexpected.sql', '0')",
                "migração inesperada",
            ),
            (
                "UPDATE public.schema_migration SET checksum = '0' WHERE nome LIKE '0017%'",
                "checksum",
            ),
            (
                "ALTER TABLE public.federation_snapshot_intent_marker RENAME TO probe_hidden_marker",
                "federation_snapshot_intent_marker.",
            ),
            (
                "ALTER TABLE public.federation_replica_checkpoint RENAME COLUMN prefix TO probe_hidden_column",
                "federation_replica_checkpoint.prefix",
            ),
            (
                "ALTER TABLE public.federation_config_policy RENAME COLUMN profile TO probe_hidden_profile",
                "federation_config_policy.profile",
            ),
            (
                "ALTER TABLE public.federation_replica_disposition RENAME TO probe_hidden_disposition",
                "federation_replica_disposition",
            ),
            (
                "ALTER TABLE public.schema_migration RENAME TO probe_hidden_history",
                "schema_migration",
            ),
        ] {
            use sqlx::Connection;
            let mut tx = admin_conn.begin().await.unwrap();
            sqlx::query(AssertSqlSafe(mutation))
                .execute(&mut *tx)
                .await
                .unwrap();
            let error = verify_restore_schema(&mut tx).await.expect_err(reason);
            assert!(error.to_string().contains(reason), "{error}");
            tx.rollback().await.unwrap();
        }
        verify_restore_schema(&mut admin_conn).await.unwrap();
    }
}
