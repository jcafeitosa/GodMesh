-- Verificação de invariantes no banco (ADR-057/058/061/062).
-- Executar com: psql -v ON_ERROR_STOP=1 -f migrations/tests/0001_invariants.sql
-- Esperado: 4 blocos com "OK". Qualquer outro resultado é regressão.

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

-- 4) Trilha de ACK é append-only.
INSERT INTO sos_incident (id, tenant_id, principal, state) VALUES ('inc-1','t1','p1','persisted_local');
INSERT INTO sos_ack (incident_id, ack_class, source, at_unix) VALUES ('inc-1','persisted_local','inbox:1',0);
DO $$
BEGIN
    UPDATE sos_ack SET source = 'adulterado' WHERE incident_id = 'inc-1';
    RAISE EXCEPTION 'FALHA: ACK foi reescrito';
EXCEPTION
    WHEN raise_exception THEN
        IF SQLERRM LIKE 'FALHA:%' THEN RAISE; END IF;
        RAISE NOTICE 'OK 4: ACK append-only (reescrita bloqueada)';
END $$;

-- 5) State do incidente é conjunto fechado.
DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state) VALUES ('inc-2','t1','p1','enviado');
    RAISE EXCEPTION 'FALHA: estado inventado aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 5: estado inventado recusado';
END $$;

SELECT 'verificação concluída' AS resultado;
