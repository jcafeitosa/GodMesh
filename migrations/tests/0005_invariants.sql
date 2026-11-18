-- Verificação de invariantes da migração 0005 (checkpoint do Inbox, ADR-057/061).
-- Requer o esquema vigente: `0001_core.sql`, `0002_incident_tenancy.sql`,
-- `0003_outbox_publisher.sql`, `0004_grant_revocation.sql` **e**
-- `0005_sync_inbox.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável,
-- aplica as migrações e falha com exit != 0 se algum bloco `OK` faltar.
--
-- Esperado: 6 blocos `OK` (1, 2, 3, 4, 5, 6). Qualquer outro resultado é
-- regressão.

\set ON_ERROR_STOP off

-- 1) Namespace duplicado é recusado: a chave é a tupla inteira.
INSERT INTO sync_inbox_state
    (tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix)
VALUES ('t1','node-a','stream-9',1,7,3)
ON CONFLICT DO NOTHING;

DO $$
BEGIN
    INSERT INTO sync_inbox_state
        (tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix)
    VALUES ('t1','node-a','stream-9',1,7,4);
    RAISE EXCEPTION 'FALHA: prefixo duplicado para o mesmo namespace foi aceito';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 1: namespace de Inbox duplicado recusado (PK)';
END $$;

-- 2) Prefixo negativo é recusado.
DO $$
BEGIN
    INSERT INTO sync_inbox_state
        (tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix)
    VALUES ('t1','node-a','stream-9',1,7,-1);
    RAISE EXCEPTION 'FALHA: prefixo negativo foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 2: contiguous_prefix negativo recusado';
END $$;

-- 3) Partição negativa e epoch zero são recusados (mesmas faixas de event_position).
DO $$
BEGIN
    INSERT INTO sync_inbox_state
        (tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix)
    VALUES ('t1','node-a','stream-9',-1,7,1);
    RAISE EXCEPTION 'FALHA: partição negativa foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 3: partição negativa recusada';
END $$;

DO $$
BEGIN
    INSERT INTO sync_inbox_state
        (tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix)
    VALUES ('t1','node-a','stream-9',1,0,1);
    RAISE EXCEPTION 'FALHA: authority_epoch zero foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 4: authority_epoch zero recusado';
END $$;

-- 5) Partição e epoch são dimensões distintas do namespace: mesma seq/stream não colide.
DO $$
DECLARE
    total integer;
BEGIN
    INSERT INTO sync_inbox_state
        (tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix)
    VALUES ('t1','node-a','stream-9',2,7,5),
           ('t1','node-a','stream-9',1,8,6),
           ('t1','node-b','stream-9',1,7,7);
    SELECT count(*) INTO total FROM sync_inbox_state WHERE stream_id = 'stream-9';
    IF total = 4 THEN
        RAISE NOTICE 'OK 5: partição, epoch e origem são dimensões do namespace (%)', total;
    ELSE
        RAISE EXCEPTION 'FALHA: namespace colapsou dimensões (linhas=%)', total;
    END IF;
END $$;

-- 6) Prefixo só avança por UPDATE explícito; a tupla do namespace não muda.
DO $$
DECLARE
    antes  bigint;
    depois bigint;
BEGIN
    SELECT contiguous_prefix INTO antes FROM sync_inbox_state
     WHERE tenant_id='t1' AND origin_node='node-a' AND stream_id='stream-9'
       AND partition=1 AND authority_epoch=7;
    UPDATE sync_inbox_state SET contiguous_prefix = antes + 1, updated_at = now()
     WHERE tenant_id='t1' AND origin_node='node-a' AND stream_id='stream-9'
       AND partition=1 AND authority_epoch=7;
    SELECT contiguous_prefix INTO depois FROM sync_inbox_state
     WHERE tenant_id='t1' AND origin_node='node-a' AND stream_id='stream-9'
       AND partition=1 AND authority_epoch=7;
    IF depois = antes + 1 THEN
        RAISE NOTICE 'OK 6: prefixo persiste a avançar no mesmo namespace (% -> %)', antes, depois;
    ELSE
        RAISE EXCEPTION 'FALHA: prefixo não avançou (% -> %)', antes, depois;
    END IF;
END $$;