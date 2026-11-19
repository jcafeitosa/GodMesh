BEGIN;
DO $$
DECLARE
    a text := 'gmn_' || repeat('a',64);
    b text := 'gmn_' || repeat('b',64);
    digest bytea := decode(repeat('ab',32),'hex');
BEGIN
    IF EXISTS (SELECT FROM federation_snapshot_receipt_policy)
       OR EXISTS (SELECT FROM federation_snapshot_receipt) THEN
        RAISE EXCEPTION '0016 semeou política ou comprovante';
    END IF;
    RAISE NOTICE 'OK 1: 0016 semeia zero políticas e comprovantes';
    INSERT INTO federation_replica_namespace
        VALUES (a,'synthetic','m4a.metadata.snapshot',0,7,b,'metadata',1,'synthetic',1,'BR','fixture',1);
    INSERT INTO federation_snapshot_receipt_policy
        VALUES ('fixture',1,b,a,'synthetic','m4a.metadata.snapshot',0,7,'BR','snapshot_receipt',50,true);
    INSERT INTO federation_snapshot_receipt
        VALUES (b,a,'synthetic','m4a.metadata.snapshot',0,7,3,0,3,digest,digest,'fixture',1,10,80,60,'BR');
    BEGIN
        INSERT INTO federation_snapshot_receipt
            VALUES (b,a,'synthetic','m4a.metadata.snapshot',0,7,3,1,2,digest,digest,'fixture',1,10,80,60,'BR');
        RAISE EXCEPTION 'segundo comprovante no mesmo corte';
    EXCEPTION WHEN unique_violation THEN
        RAISE NOTICE 'OK 2: mesmo corte não pode ganhar segundo comprovante';
    END;
    BEGIN
        INSERT INTO federation_snapshot_receipt
            VALUES (a,a,'synthetic','m4a.metadata.snapshot',0,7,3,0,3,digest,digest,'fixture',1,10,80,60,'BR');
        RAISE EXCEPTION 'comprovante aceitou destino divergente';
    EXCEPTION WHEN foreign_key_violation THEN
        RAISE NOTICE 'OK 3: comprovante vincula identidade local exata';
    END;
    UPDATE federation_snapshot_receipt_policy SET active=false;
    BEGIN
        UPDATE federation_snapshot_receipt_policy SET duration_seconds=600;
        RAISE EXCEPTION 'alterou política histórica';
    EXCEPTION WHEN check_violation THEN
        RAISE NOTICE 'OK 4: versão histórica de comprovante imutável';
    END;
    BEGIN
        DELETE FROM federation_snapshot_receipt_policy WHERE policy_id='fixture' AND policy_version=1;
        INSERT INTO federation_snapshot_receipt_policy
            VALUES ('fixture',1,b,a,'synthetic','m4a.metadata.snapshot',0,7,'BR','snapshot_receipt',900,false);
        RAISE EXCEPTION 'exclusão e recriação da versão referenciada foram aceitas';
    EXCEPTION
        WHEN foreign_key_violation THEN
            RAISE NOTICE 'OK 5: referência bloqueia exclusão e recriação da versão histórica';
        WHEN raise_exception THEN
            RAISE WARNING '%', SQLERRM;
    END;

    INSERT INTO federation_replica_namespace
        VALUES (a,'synthetic','m4a.metadata.other',0,7,b,'metadata',1,'synthetic',1,'BR','fixture',1);
    INSERT INTO federation_snapshot_receipt_policy
        VALUES ('other',1,b,a,'synthetic','m4a.metadata.other',0,7,'BR','snapshot_receipt',50,true);
    BEGIN
        UPDATE federation_snapshot_receipt SET policy_id='other';
        RAISE EXCEPTION 'chave válida de outra série foi aceita';
    EXCEPTION
        WHEN foreign_key_violation THEN
            RAISE NOTICE 'OK 6: comprovante recusa chave de política de outra série';
        WHEN raise_exception THEN
            RAISE WARNING '%', SQLERRM;
    END;

END
$$;
ROLLBACK;
