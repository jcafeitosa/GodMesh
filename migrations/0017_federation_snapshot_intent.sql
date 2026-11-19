-- M4-B2-B. O marcador sobrevive à redução do detalhe e ao desligamento do harness.
CREATE TABLE federation_snapshot_intent_policy (
    policy_id text NOT NULL CHECK (btrim(policy_id) <> ''),
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    jurisdiction text NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    purpose text NOT NULL CHECK (purpose = 'snapshot_intent'),
    duration_seconds bigint NOT NULL CHECK (duration_seconds > 0),
    allow_reduce boolean NOT NULL,
    active boolean NOT NULL,
    PRIMARY KEY (policy_id, policy_version),
    UNIQUE (policy_id, policy_version, local_node, origin_node, tenant_id,
            stream_id, partition, authority_epoch, jurisdiction),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
        REFERENCES federation_replica_namespace
            (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);
CREATE UNIQUE INDEX federation_snapshot_intent_one_current_policy
    ON federation_snapshot_intent_policy
        (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch)
    WHERE active;
CREATE TRIGGER federation_snapshot_intent_policy_immutable
    BEFORE UPDATE ON federation_snapshot_intent_policy
    FOR EACH ROW EXECUTE FUNCTION federation_snapshot_policy_immutable();

CREATE TABLE federation_snapshot_intent_marker (
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    attempt_id bigint GENERATED ALWAYS AS IDENTITY (NO CYCLE) CHECK (attempt_id > 0),
    state text NOT NULL DEFAULT 'pending' CHECK (state = 'pending'),
    PRIMARY KEY (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch),
    UNIQUE (attempt_id),
    UNIQUE (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch, attempt_id),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
        REFERENCES federation_replica_namespace
            (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);

CREATE TABLE federation_snapshot_intent_detail (
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    attempt_id bigint NOT NULL,
    base_seq bigint NOT NULL CHECK (base_seq >= 0),
    cut_seq bigint NOT NULL CHECK (cut_seq > base_seq),
    event_count bigint NOT NULL CHECK (event_count = cut_seq - base_seq AND event_count <= 4294967295),
    manifest_digest bytea NOT NULL CHECK (octet_length(manifest_digest) = 32),
    package_digest bytea NOT NULL CHECK (octet_length(package_digest) = 32),
    events_digest bytea NOT NULL CHECK (octet_length(events_digest) = 32),
    not_before bigint NOT NULL,
    signed_expires_at bigint NOT NULL CHECK (signed_expires_at > not_before),
    created_at bigint NOT NULL CHECK (created_at >= not_before),
    detail_expires_at bigint NOT NULL CHECK (detail_expires_at > created_at AND detail_expires_at <= signed_expires_at),
    policy_id text NOT NULL,
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    jurisdiction text NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    PRIMARY KEY (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch),
    FOREIGN KEY (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch, attempt_id)
        REFERENCES federation_snapshot_intent_marker
            (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch, attempt_id)
        ON DELETE RESTRICT ON UPDATE RESTRICT,
    FOREIGN KEY (policy_id, policy_version, local_node, origin_node, tenant_id,
                 stream_id, partition, authority_epoch, jurisdiction)
        REFERENCES federation_snapshot_intent_policy
            (policy_id, policy_version, local_node, origin_node, tenant_id,
             stream_id, partition, authority_epoch, jurisdiction)
        ON DELETE RESTRICT ON UPDATE RESTRICT
);
