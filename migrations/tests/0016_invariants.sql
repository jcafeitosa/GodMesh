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
        VALUES (b,a,'synthetic','m4a.metadata.snapshot',0,7,3,0,3,digest,digest,'fixture',1,10,80,60);
    BEGIN
        INSERT INTO federation_snapshot_receipt
            VALUES (b,a,'synthetic','m4a.metadata.snapshot',0,7,3,1,2,digest,digest,'fixture',1,10,80,60);
        RAISE EXCEPTION 'segundo comprovante no mesmo corte';
    EXCEPTION WHEN unique_violation THEN
        RAISE NOTICE 'OK 2: mesmo corte não pode ganhar segundo comprovante';
    END;
    BEGIN
        INSERT INTO federation_snapshot_receipt
            VALUES (a,a,'synthetic','m4a.metadata.snapshot',0,7,3,0,3,digest,digest,'fixture',1,10,80,60);
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
END
$$;
ROLLBACK;
