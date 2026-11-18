-- Verificação de invariantes do banco (ADR-057/058/061/062/065).
--
-- Requer o esquema vigente: `0001_core.sql` **e** `0002_incident_tenancy.sql`.
-- Sobre o esquema só com 0001 este arquivo não é aplicável: os blocos 4, 5 e 6
-- usam `sos_incident.retention_class/retention_jurisdiction`, criadas na 0002,
-- e por isso falham com "column does not exist" (não com um invariante violado).
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável,
-- aplica 0001+0002 e falha com exit != 0 se algum bloco `OK` faltar. Rodar
-- `psql` direto não basta: com `ON_ERROR_STOP off` o psql devolve 0 mesmo
-- depois de um ERROR, então a contagem dos `NOTICE: OK` é a prova.
--
-- Esperado: 6 blocos `OK` (1, 2, 3, 4, 5, 6). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) Evento sem classe de retenção deve falhar (fail-closed, ADR-062).
DO $$
BEGIN
    INSERT INTO event_position (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, retention_class, jurisdiction)
    VALUES ('node-a','t1','s1',1,7,1,'classe_inexistente','BR');
    RAISE EXCEPTION 'FALHA: evento sem classe de retenção foi aceito';
EXCEPTION
    WHEN foreign_key_violation THEN RAISE NOTICE 'OK 1: evento sem política recusado';
END $$;

-- 2) Evento com classe válida deve ser aceito.
DO $$
BEGIN
    INSERT INTO event_position (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, retention_class, jurisdiction)
    VALUES ('node-a','t1','s1',1,7,1,'posicao','BR');
    RAISE NOTICE 'OK 2: evento com política aceito';
END $$;

-- 3) Posição duplicada deve falhar.
DO $$
BEGIN
    INSERT INTO event_position (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, retention_class, jurisdiction)
    VALUES ('node-a','t1','s1',1,7,1,'posicao','BR');
    RAISE EXCEPTION 'FALHA: posição duplicada aceita';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 3: posição duplicada recusada';
END $$;

-- Fixture dos blocos 4 e 5. A partir da migração 0002 o incidente exige a
-- política que autorizou a persistência (ADR-062), então o insert declara
-- classe e jurisdição; `ON CONFLICT DO NOTHING` mantém o arquivo reexecutável.
INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
VALUES ('inc-1','t1','p1','persisted_local','incidente_sos','BR')
ON CONFLICT DO NOTHING;
INSERT INTO sos_ack (incident_id, ack_class, source, at_unix) VALUES ('inc-1','persisted_local','inbox:1',0)
ON CONFLICT DO NOTHING;

-- 4) Trilha de ACK é append-only contra UPDATE.
DO $$
DECLARE
    linhas integer;
BEGIN
    SELECT count(*) INTO linhas FROM sos_ack WHERE incident_id = 'inc-1';
    IF linhas <> 1 THEN
        RAISE EXCEPTION 'FALHA: fixture de ACK ausente (linhas=%)', linhas;
    END IF;
    BEGIN
        UPDATE sos_ack SET source = 'adulterado' WHERE incident_id = 'inc-1';
        RAISE EXCEPTION 'FALHA: ACK foi reescrito';
    EXCEPTION
        WHEN raise_exception THEN
            IF SQLERRM LIKE 'FALHA:%' THEN RAISE; END IF;
            RAISE NOTICE 'OK 4: ACK append-only (UPDATE bloqueado pelo trigger)';
    END;
END $$;

-- 5) Trilha de ACK é append-only contra DELETE.
DO $$
DECLARE
    linhas integer;
BEGIN
    BEGIN
        DELETE FROM sos_ack WHERE incident_id = 'inc-1';
        RAISE EXCEPTION 'FALHA: ACK foi apagado';
    EXCEPTION
        WHEN raise_exception THEN
            IF SQLERRM LIKE 'FALHA:%' THEN RAISE; END IF;
            RAISE NOTICE 'OK 5: ACK append-only (DELETE bloqueado pelo trigger)';
    END;
    SELECT count(*) INTO linhas FROM sos_ack WHERE incident_id = 'inc-1';
    IF linhas <> 1 THEN
        RAISE EXCEPTION 'FALHA: ACK desapareceu da trilha (linhas=%)', linhas;
    END IF;
END $$;

-- 6) State do incidente é conjunto fechado.
-- Id fixo é seguro: o INSERT nunca cria linha (o CHECK falha antes), então a
-- reprovação é idempotente e não depende de `random()`.
DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES ('inc-check-estado', 't1', 'p1', 'enviado', 'incidente_sos', 'BR');
    RAISE EXCEPTION 'FALHA: estado inventado aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 6: estado inventado recusado';
END $$;

SELECT 'verificação concluída' AS resultado;