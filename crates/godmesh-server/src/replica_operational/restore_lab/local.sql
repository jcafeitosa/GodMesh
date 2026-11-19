-- Administração da fixture instala ou confere este DDL antes de entregar o login.
CREATE TABLE federation_restore_local (
    fixture_id text NOT NULL, cluster text NOT NULL, local_node text NOT NULL, origin_node text NOT NULL,
    tenant_id text NOT NULL, stream_id text NOT NULL, partition integer NOT NULL CHECK (partition=0),
    authority_epoch bigint NOT NULL CHECK (authority_epoch>0),
    generation bigint NOT NULL CHECK (generation>0),
    target_db_id text NOT NULL,
    high_water_sha256 bytea NOT NULL CHECK (octet_length(high_water_sha256)=32),
    fence_sha256 bytea NOT NULL CHECK (octet_length(fence_sha256)=32),
    phase text NOT NULL CHECK (phase IN ('restore_closed','reconciled_closed','open','retired_closed')),
    reconciliation_sha256 bytea CHECK (octet_length(reconciliation_sha256)=32),
    authorization_sha256 bytea CHECK (octet_length(authorization_sha256)=32),
    final_prefix bigint CHECK (final_prefix>=0),
    proofs_sha256 bytea CHECK (octet_length(proofs_sha256)=32),
    current_control_sha256 bytea CHECK (octet_length(current_control_sha256)=32),
    PRIMARY KEY (fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,generation),
    FOREIGN KEY (origin_node,tenant_id,stream_id,partition,authority_epoch,local_node)
        REFERENCES federation_replica_namespace
            (origin_node,tenant_id,stream_id,partition,authority_epoch,local_node)
        ON DELETE RESTRICT ON UPDATE RESTRICT,
    CHECK (generation = 1 OR phase NOT IN ('reconciled_closed','open') OR
        (reconciliation_sha256 IS NOT NULL AND final_prefix IS NOT NULL AND
         proofs_sha256 IS NOT NULL AND current_control_sha256 IS NOT NULL)),
    CHECK (generation = 1 OR phase <> 'open' OR authorization_sha256 IS NOT NULL)
);

CREATE FUNCTION federation_restore_local_terminal() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'estado de geração não pode ser excluído' USING ERRCODE = '23514';
    END IF;
    IF (OLD.phase = 'retired_closed' AND NEW.phase <> 'retired_closed')
       OR (OLD.fixture_id,OLD.cluster,OLD.local_node,OLD.origin_node,OLD.tenant_id,
           OLD.stream_id,OLD.partition,OLD.authority_epoch,OLD.generation,OLD.target_db_id,
           OLD.high_water_sha256,OLD.fence_sha256)
          IS DISTINCT FROM
          (NEW.fixture_id,NEW.cluster,NEW.local_node,NEW.origin_node,NEW.tenant_id,
           NEW.stream_id,NEW.partition,NEW.authority_epoch,NEW.generation,NEW.target_db_id,
           NEW.high_water_sha256,NEW.fence_sha256) THEN
        RAISE EXCEPTION 'geração encerrada ou vínculo imutável' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER federation_restore_local_terminal
    BEFORE UPDATE OR DELETE ON federation_restore_local
    FOR EACH ROW EXECUTE FUNCTION federation_restore_local_terminal();
