BEGIN;
DO $$
DECLARE
    a text := 'gmn_' || repeat('a',64);
    b text := 'gmn_' || repeat('b',64);
    first_token bigint;
    second_token bigint;
    digest bytea := decode(repeat('ab',32),'hex');
BEGIN
    IF EXISTS (SELECT FROM federation_snapshot_intent_marker)
       OR EXISTS (SELECT FROM federation_snapshot_intent_detail)
       OR EXISTS (SELECT FROM federation_snapshot_intent_policy) THEN
        RAISE EXCEPTION '0017 semeou intenção ou política';
    END IF;
    RAISE NOTICE 'OK 1: 0017 sem seed';

    INSERT INTO federation_replica_namespace VALUES
        (a,'synthetic','m4a.metadata.intent',0,7,b,'metadata',1,'synthetic',1,'BR','fixture',1),
        (a,'synthetic','m4a.metadata.other',0,7,b,'metadata',1,'synthetic',1,'BR','fixture',1);
    INSERT INTO federation_snapshot_intent_policy VALUES
        ('fixture',1,b,a,'synthetic','m4a.metadata.intent',0,7,'BR','snapshot_intent',50,true,true),
        ('other',1,b,a,'synthetic','m4a.metadata.other',0,7,'BR','snapshot_intent',50,true,true);

    INSERT INTO federation_snapshot_intent_marker
        (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)
        VALUES (b,a,'synthetic','m4a.metadata.intent',0,7)
        RETURNING attempt_id INTO first_token;
    INSERT INTO federation_snapshot_intent_detail
        VALUES (b,a,'synthetic','m4a.metadata.intent',0,7,first_token,0,3,3,
                digest,digest,digest,1,100,10,60,'fixture',1,'BR');

    UPDATE federation_snapshot_intent_policy SET active=false WHERE policy_id='fixture';
    BEGIN
        DELETE FROM federation_snapshot_intent_policy WHERE policy_id='fixture';
        RAISE EXCEPTION 'excluiu política histórica referenciada';
    EXCEPTION WHEN foreign_key_violation THEN
        RAISE NOTICE 'OK 2: detalhe preserva versão histórica inativa';
    END;
    BEGIN
        UPDATE federation_snapshot_intent_detail SET policy_id='other';
        RAISE EXCEPTION 'detalhe aceitou política de outra série';
    EXCEPTION WHEN foreign_key_violation THEN
        RAISE NOTICE 'OK 3: detalhe recusa política de outra série';
    END;
    BEGIN
        DELETE FROM federation_snapshot_intent_marker;
        RAISE EXCEPTION 'removeu marcador com detalhe ativo';
    EXCEPTION WHEN foreign_key_violation THEN
        RAISE NOTICE 'OK 4: detalhe vincula token e marcador ativos';
    END;

    DELETE FROM federation_snapshot_intent_detail;
    IF (SELECT attempt_id FROM federation_snapshot_intent_marker) IS DISTINCT FROM first_token THEN
        RAISE EXCEPTION 'redução apagou ou alterou marcador';
    END IF;
    RAISE NOTICE 'OK 5: redução mantém o marcador e seu token';

    DELETE FROM federation_snapshot_intent_marker;
    INSERT INTO federation_snapshot_intent_marker
        (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)
        VALUES (b,a,'synthetic','m4a.metadata.intent',0,7)
        RETURNING attempt_id INTO second_token;
    IF second_token <= first_token THEN
        RAISE EXCEPTION 'token não avançou';
    END IF;
    BEGIN
        INSERT INTO federation_snapshot_intent_detail
            VALUES (b,a,'synthetic','m4a.metadata.intent',0,7,first_token,0,3,3,
                    digest,digest,digest,1,100,10,60,'fixture',1,'BR');
        RAISE EXCEPTION 'reutilizou token antigo';
    EXCEPTION WHEN foreign_key_violation THEN
        RAISE NOTICE 'OK 6: token antigo não vincula detalhe a nova intenção';
    END;
END
$$;
ROLLBACK;
