-- Verificação de invariantes da migração 0003 (ADR-061: publicar ≠ confirmar).
--
-- Requer o esquema completo: `0001_core.sql`, `0002_incident_tenancy.sql` e
-- `0003_outbox_publisher.sql`. Execute com `migrations/tests/run_all.sh`, que
-- cria um banco descartável, aplica as três migrações e falha com exit != 0 se
-- algum bloco `OK` faltar.
--
-- Esperado: 6 blocos `OK` (1..6). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) Confirmação sem publicação é impossível no banco (regra dura (a)).
DO $$
BEGIN
    INSERT INTO outbox_entry (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind, confirmed_at)
    VALUES ('n3','t1','s1',1,7,1,'event', now());
    RAISE EXCEPTION 'FALHA: confirmação sem publicação foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 1: confirmed_at sem published_at recusado (CHECK)';
END $$;

-- 2) Publicação sem confirmação é o estado normal do append.
DO $$
BEGIN
    INSERT INTO outbox_entry (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind, published_at)
    VALUES ('n3','t1','s1',1,7,1,'event', now());
    RAISE NOTICE 'OK 2: publicado sem confirmado aceito';
END $$;

-- 3) Publicado e confirmado na mesma linha é aceito.
DO $$
BEGIN
    UPDATE outbox_entry SET confirmed_at = now()
     WHERE origin_node = 'n3' AND tenant_id = 't1' AND stream_id = 's1'
       AND partition = 1 AND authority_epoch = 7 AND seq = 1;
    RAISE NOTICE 'OK 3: publicado e então confirmado aceito';
END $$;

-- 4) Apagar a publicação de uma linha confirmada é recusado pelo mesmo CHECK.
DO $$
BEGIN
    UPDATE outbox_entry SET published_at = NULL
     WHERE origin_node = 'n3' AND tenant_id = 't1' AND stream_id = 's1'
       AND partition = 1 AND authority_epoch = 7 AND seq = 1;
    RAISE EXCEPTION 'FALHA: linha confirmada perdeu published_at';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 4: apagar published_at de linha confirmada recusado';
END $$;

-- 5) `attempts` nasce em 0 e recusa valor negativo.
DO $$
DECLARE
    v integer;
BEGIN
    INSERT INTO outbox_entry (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind)
    VALUES ('n3','t1','s1',1,7,2,'event');
    SELECT attempts INTO v FROM outbox_entry
     WHERE origin_node = 'n3' AND tenant_id = 't1' AND stream_id = 's1'
       AND partition = 1 AND authority_epoch = 7 AND seq = 2;
    IF v <> 0 THEN
        RAISE EXCEPTION 'FALHA: attempts inicial deveria ser 0, veio %', v;
    END IF;
    BEGIN
        UPDATE outbox_entry SET attempts = -1
         WHERE origin_node = 'n3' AND tenant_id = 't1' AND stream_id = 's1'
           AND partition = 1 AND authority_epoch = 7 AND seq = 2;
        RAISE EXCEPTION 'FALHA: attempts negativo aceito';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
    RAISE NOTICE 'OK 5: attempts inicia em 0 e recusa negativo';
END $$;

-- 6) A PK do sink recusa a mesma posição duas vezes (detector de duplicata).
DO $$
BEGIN
    INSERT INTO outbox_sink (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind)
    VALUES ('n3','t1','s1',1,7,1,'event');
    INSERT INTO outbox_sink (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind)
    VALUES ('n3','t1','s1',1,7,1,'event');
    RAISE EXCEPTION 'FALHA: entrega duplicada no sink foi aceita';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 6: entrega duplicada da mesma posição recusada';
END $$;

SELECT 'verificação concluída' AS resultado;