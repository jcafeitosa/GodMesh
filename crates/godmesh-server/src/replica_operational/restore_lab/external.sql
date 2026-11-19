-- Somente no banco externo descartável, excluído dos artefatos de restore.
CREATE TABLE federation_restore_control (
    fixture_id text NOT NULL, cluster text NOT NULL, local_node text NOT NULL, origin_node text NOT NULL,
    tenant_id text NOT NULL, stream_id text NOT NULL, partition integer NOT NULL CHECK (partition=0),
    authority_epoch bigint NOT NULL CHECK (authority_epoch>0),
    generation bigint NOT NULL CHECK (generation>0),
    target_db_id text NOT NULL,
    high_water_sha256 bytea NOT NULL CHECK (octet_length(high_water_sha256)=32),
    fence_sha256 bytea NOT NULL CHECK (octet_length(fence_sha256)=32),
    origin_observation text,
    phase text NOT NULL CHECK (phase IN ('fenced','promotion_consumed')),
    PRIMARY KEY (fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)
);
CREATE TABLE federation_restore_decision (
    fixture_id text NOT NULL, cluster text NOT NULL, local_node text NOT NULL, origin_node text NOT NULL,
    tenant_id text NOT NULL, stream_id text NOT NULL, partition integer NOT NULL CHECK (partition=0),
    authority_epoch bigint NOT NULL CHECK (authority_epoch>0),
    generation bigint NOT NULL CHECK (generation>0),
    authorization_sha256 bytea NOT NULL CHECK (octet_length(authorization_sha256)=32),
    opening_origin_observation text,
    consumed_at bigint NOT NULL,
    reconciliation_sha256 bytea NOT NULL CHECK (octet_length(reconciliation_sha256)=32),
    PRIMARY KEY (fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,generation),
    FOREIGN KEY (fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)
        REFERENCES federation_restore_control ON DELETE RESTRICT ON UPDATE RESTRICT
);

CREATE TABLE federation_restore_generation_logins (
    fixture_id text NOT NULL, cluster text NOT NULL, local_node text NOT NULL, origin_node text NOT NULL,
    tenant_id text NOT NULL, stream_id text NOT NULL, partition integer NOT NULL CHECK (partition=0),
    authority_epoch bigint NOT NULL CHECK (authority_epoch>0),
    generation bigint NOT NULL CHECK (generation>0),
    target_db_id text NOT NULL CHECK (length(btrim(target_db_id))>0),
    target_login text NOT NULL CHECK (octet_length(target_login) BETWEEN 1 AND 63 AND length(btrim(target_login))>0),
    observer_login text NOT NULL CHECK (octet_length(observer_login) BETWEEN 1 AND 63 AND length(btrim(observer_login))>0),
    cas_login text NOT NULL CHECK (octet_length(cas_login) BETWEEN 1 AND 63 AND length(btrim(cas_login))>0),
    CHECK (target_login<>observer_login AND target_login<>cas_login AND observer_login<>cas_login),
    PRIMARY KEY (fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,generation),
    UNIQUE (fixture_id,target_login)
);
REVOKE ALL ON federation_restore_generation_logins FROM PUBLIC;

CREATE FUNCTION federation_restore_generation_logins_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'restore generation logins are immutable' USING ERRCODE='23514';
END
$$;

CREATE TRIGGER federation_restore_generation_logins_immutable
BEFORE UPDATE OR DELETE OR TRUNCATE ON federation_restore_generation_logins
FOR EACH STATEMENT EXECUTE FUNCTION federation_restore_generation_logins_immutable();
