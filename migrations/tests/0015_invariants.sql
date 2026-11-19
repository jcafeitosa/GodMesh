BEGIN;
DO $$
DECLARE
    origin text := 'gmn_' || repeat('a', 64);
    receiver text := 'gmn_' || repeat('b', 64);
BEGIN
    IF EXISTS (SELECT FROM federation_replica_resume_block) THEN
        RAISE EXCEPTION '0015 semeou suspensão';
    END IF;
    RAISE NOTICE 'OK 1: 0015 semeia zero suspensões';
    INSERT INTO federation_replica_namespace
    VALUES (origin,'synthetic','m4a.metadata.suspension',0,7,receiver,'metadata',1,'synthetic',1,'BR','fixture',1);
    INSERT INTO federation_replica_resume_block
    VALUES (receiver,origin,'synthetic','m4a.metadata.suspension',0,7,1);
    IF EXISTS (SELECT FROM federation_replica_checkpoint WHERE origin_node=origin AND tenant_id='synthetic') THEN
        RAISE EXCEPTION 'suspensão criou checkpoint';
    END IF;
    RAISE NOTICE 'OK 2: suspensão referencia reserva sem criar checkpoint';

    BEGIN
        INSERT INTO federation_replica_resume_block
        VALUES (origin,origin,'synthetic','m4a.metadata.suspension',0,7,1);
        RAISE EXCEPTION 'suspensão aceitou destino divergente';
    EXCEPTION WHEN foreign_key_violation THEN
        RAISE NOTICE 'OK 3: suspensão exige destino exato da reserva';
    END;
    BEGIN
        UPDATE federation_replica_resume_block SET blocked_next_seq=0 WHERE local_node=receiver;
        RAISE EXCEPTION 'suspensão aceitou sequência zero';
    EXCEPTION WHEN check_violation THEN
        RAISE NOTICE 'OK 4: suspensão exige sequência positiva';
    END;
END
$$;
ROLLBACK;
