-- M4-B2-A. Políticas e comprovantes sintéticos, sem seed.
CREATE TABLE federation_snapshot_receipt_policy (
    policy_id text NOT NULL CHECK (btrim(policy_id) <> ''),
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    jurisdiction text NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    purpose text NOT NULL CHECK (purpose = 'snapshot_receipt'),
    duration_seconds bigint NOT NULL CHECK (duration_seconds > 0),
    active boolean NOT NULL,
    PRIMARY KEY (policy_id, policy_version),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
        REFERENCES federation_replica_namespace
            (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);
CREATE UNIQUE INDEX federation_snapshot_one_current_policy
    ON federation_snapshot_receipt_policy
        (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch)
    WHERE active;

CREATE FUNCTION federation_snapshot_policy_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(OLD) - 'active') IS DISTINCT FROM (to_jsonb(NEW) - 'active') THEN
        RAISE EXCEPTION 'versão histórica de comprovante imutável' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER federation_snapshot_policy_immutable BEFORE UPDATE ON federation_snapshot_receipt_policy
    FOR EACH ROW EXECUTE FUNCTION federation_snapshot_policy_immutable();

CREATE TABLE federation_snapshot_receipt (
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    cut_seq bigint NOT NULL CHECK (cut_seq > 0),
    base_seq bigint NOT NULL CHECK (base_seq >= 0 AND base_seq < cut_seq),
    event_count bigint NOT NULL CHECK (event_count = cut_seq - base_seq),
    manifest_digest bytea NOT NULL CHECK (octet_length(manifest_digest) = 32),
    package_digest bytea NOT NULL CHECK (octet_length(package_digest) = 32),
    policy_id text NOT NULL,
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    applied_at bigint NOT NULL,
    proof_expires_at bigint NOT NULL CHECK (proof_expires_at > applied_at),
    expires_at bigint NOT NULL CHECK (expires_at > applied_at AND expires_at <= proof_expires_at),
    PRIMARY KEY (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch, cut_seq),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
        REFERENCES federation_replica_namespace
            (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);
