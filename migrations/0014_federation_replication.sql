-- M4-A1: reserva estática e metadados de réplica. Nenhuma política é semeada.
CREATE TABLE federation_replica_namespace (
    origin_node text NOT NULL CHECK (origin_node ~ '^gmn_[0-9a-f]{64}$'),
    tenant_id text NOT NULL CHECK (btrim(tenant_id) <> ''),
    stream_id text NOT NULL CHECK (stream_id LIKE 'm4a.metadata.%' AND length(stream_id) > 13),
    partition integer NOT NULL CHECK (partition = 0),
    authority_epoch bigint NOT NULL CHECK (authority_epoch > 0),
    local_node text NOT NULL CHECK (local_node ~ '^gmn_[0-9a-f]{64}$'),
    profile text NOT NULL CHECK (profile = 'metadata'),
    profile_version bigint NOT NULL CHECK (profile_version = 1),
    retention_class text NOT NULL CHECK (btrim(retention_class) <> ''),
    class_version bigint NOT NULL CHECK (class_version > 0),
    jurisdiction text NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    policy_id text NOT NULL CHECK (btrim(policy_id) <> ''),
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition),
    UNIQUE (origin_node, tenant_id, stream_id, partition, authority_epoch),
    UNIQUE (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);

CREATE TABLE federation_replica_policy (
    policy_id text NOT NULL CHECK (btrim(policy_id) <> ''),
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    tenant_id text NOT NULL CHECK (btrim(tenant_id) <> ''),
    retention_class text NOT NULL CHECK (btrim(retention_class) <> ''),
    class_version bigint NOT NULL CHECK (class_version > 0),
    jurisdiction text NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    copy_node text NOT NULL CHECK (copy_node ~ '^gmn_[0-9a-f]{64}$'),
    copy_kind text NOT NULL CHECK (copy_kind IN ('primary', 'secondary')),
    purpose text NOT NULL CHECK (purpose = 'replica_metadata'),
    trigger_event text NOT NULL CHECK (trigger_event = 'issued_at'),
    duration_seconds bigint NOT NULL CHECK (duration_seconds > 0),
    allow_discard boolean NOT NULL,
    hold_mode text NOT NULL CHECK (hold_mode = 'explicit'),
    active boolean NOT NULL,
    PRIMARY KEY (policy_id, policy_version)
);

CREATE UNIQUE INDEX federation_replica_one_active_version
    ON federation_replica_policy (tenant_id, retention_class, jurisdiction, copy_node, copy_kind)
    WHERE active;

CREATE FUNCTION federation_replica_policy_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(OLD) - 'active') IS DISTINCT FROM (to_jsonb(NEW) - 'active') THEN
        RAISE EXCEPTION 'versão histórica de política de réplica imutável' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER federation_replica_policy_immutable
    BEFORE UPDATE ON federation_replica_policy
    FOR EACH ROW EXECUTE FUNCTION federation_replica_policy_immutable();

-- ACLs podem ser provisionadas antes da reserva. A correspondência completa
-- com namespace, versão e política efetiva é exigida pelo serviço.
CREATE TABLE federation_replica_acl (
    origin_node text NOT NULL CHECK (origin_node ~ '^gmn_[0-9a-f]{64}$'),
    tenant_id text NOT NULL CHECK (btrim(tenant_id) <> ''),
    stream_id text NOT NULL CHECK (stream_id LIKE 'm4a.metadata.%' AND length(stream_id) > 13),
    partition integer NOT NULL CHECK (partition = 0),
    authority_epoch bigint NOT NULL CHECK (authority_epoch > 0),
    destination text NOT NULL CHECK (destination ~ '^gmn_[0-9a-f]{64}$' AND destination <> origin_node),
    direction text NOT NULL CHECK (direction IN ('export', 'import')),
    retention_class text NOT NULL CHECK (btrim(retention_class) <> ''),
    class_version bigint NOT NULL CHECK (class_version > 0),
    jurisdiction text NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    copy_kind text NOT NULL CHECK (copy_kind = 'secondary'),
    policy_id text NOT NULL,
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    not_before bigint NOT NULL,
    expires_at bigint NOT NULL CHECK (expires_at > not_before),
    revoked boolean NOT NULL DEFAULT false,
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, destination, direction)
);

CREATE TABLE federation_replica_checkpoint (
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    prefix bigint NOT NULL CHECK (prefix >= 0),
    PRIMARY KEY (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
        REFERENCES federation_replica_namespace
            (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);

CREATE TABLE federation_replica_proof (
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    seq bigint NOT NULL,
    frame bytea NOT NULL CHECK (octet_length(frame) > 0),
    issued_at bigint NOT NULL,
    expires_at bigint NOT NULL CHECK (expires_at > issued_at),
    policy_id text NOT NULL,
    policy_version bigint NOT NULL CHECK (policy_version > 0),
    published boolean NOT NULL,
    discard_allowed boolean NOT NULL,
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch)
        REFERENCES federation_replica_namespace (origin_node, tenant_id, stream_id, partition, authority_epoch),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq)
        REFERENCES event_position
);

CREATE TABLE federation_replica_hold (
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    seq bigint NOT NULL CHECK (seq > 0),
    active boolean NOT NULL,
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch)
        REFERENCES federation_replica_namespace (origin_node, tenant_id, stream_id, partition, authority_epoch)
);

CREATE TABLE federation_replica_disposition (
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    seq bigint NOT NULL CHECK (seq > 0),
    removed_at bigint NOT NULL,
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch)
        REFERENCES federation_replica_namespace (origin_node, tenant_id, stream_id, partition, authority_epoch)
);
