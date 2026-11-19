BEGIN;
DO $$
DECLARE
    origin text := 'gmn_' || repeat('a', 64);
BEGIN
    IF EXISTS (SELECT FROM federation_replica_policy)
       OR EXISTS (SELECT FROM federation_replica_acl)
       OR EXISTS (SELECT FROM retention_class WHERE class = 'm4a_metadados_posicao') THEN
        RAISE EXCEPTION '0014 semeou política, ACL ou classe';
    END IF;
    RAISE NOTICE 'OK 1: 0014 sem política ou ACL implícita';

    INSERT INTO federation_replica_namespace
    VALUES (origin,'synthetic','m4a.metadata.invariant',0,7,origin,'metadata',1,'synthetic',1,'BR','fixture',1);
    BEGIN
        INSERT INTO federation_replica_namespace
        VALUES (origin,'synthetic','m4a.metadata.invariant',0,9,origin,'metadata',1,'synthetic',1,'BR','fixture',1);
        RAISE EXCEPTION 'epoch duplicou reserva';
    EXCEPTION WHEN unique_violation THEN
        RAISE NOTICE 'OK 2: epoch alternativo não duplica reserva';
    END;

    INSERT INTO federation_replica_policy
    VALUES ('fixture',1,'synthetic','synthetic',1,'BR',origin,'primary','replica_metadata','issued_at',10,true,'explicit',true);
    UPDATE federation_replica_policy SET active=false WHERE policy_id='fixture';
    BEGIN
        UPDATE federation_replica_policy SET duration_seconds=20 WHERE policy_id='fixture';
        RAISE EXCEPTION 'política histórica foi alterada';
    EXCEPTION WHEN check_violation THEN
        RAISE NOTICE 'OK 3: versão histórica não muda após retirada';
    END;
END
$$;
ROLLBACK;
