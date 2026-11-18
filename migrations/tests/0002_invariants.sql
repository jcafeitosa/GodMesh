-- Verificação de invariantes da migração 0002 (ADR-062/065/066).
-- Executar com: psql -v ON_ERROR_STOP=1 -f migrations/tests/0002_invariants.sql
-- Esperado: 7 blocos com "OK". Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) Incidente sem classe de retenção é recusado (fail-closed, ADR-062).
DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state)
    VALUES ('a2-sem-politica','t1','p1','persisted_local');
    RAISE EXCEPTION 'FALHA: incidente sem classe de retenção foi aceito';
EXCEPTION
    WHEN not_null_violation THEN RAISE NOTICE 'OK 1: incidente sem classe de retenção recusado';
END $$;

-- 2) Incidente com política inexistente é recusado pela FK composta.
DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES ('a2-classe-invalida','t1','p1','persisted_local','incidente_sos','AR');
    RAISE EXCEPTION 'FALHA: jurisdição fora do conjunto aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 2a: jurisdição fora de BR/CO recusada';
END $$;

DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES ('a2-classe-inexistente','t1','p1','persisted_local','classe_inexistente','BR');
    RAISE EXCEPTION 'FALHA: incidente sem política de retenção foi aceito';
EXCEPTION
    WHEN foreign_key_violation THEN RAISE NOTICE 'OK 2b: incidente sem política recusado (FK)';
END $$;

-- 3) Política da Colômbia semeada, classe **e** cópia primária (ADR-066).
DO $$
DECLARE
    classes integer;
    copies  integer;
BEGIN
    SELECT count(*) INTO classes FROM retention_class
     WHERE class = 'incidente_sos' AND jurisdiction = 'CO';
    SELECT count(*) INTO copies FROM retention_copy
     WHERE class = 'incidente_sos' AND jurisdiction = 'CO' AND copy_scope = 'primary';
    IF classes = 1 AND copies = 1 THEN
        RAISE NOTICE 'OK 3: incidente_sos semeado para CO com cópia primária';
    ELSE
        RAISE EXCEPTION 'FALHA: semente de CO incompleta (classes=%, copies=%)', classes, copies;
    END IF;
END $$;

-- 4) Incidente com política válida é aceito e registra qual política autorizou.
-- `ON CONFLICT` mantém o arquivo reexecutável: o mesmo id não pode ser gravado duas vezes.
INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
VALUES ('a2-valido','t1','p1','persisted_local','incidente_sos','BR')
ON CONFLICT DO NOTHING;

DO $$
DECLARE
    autorizou text;
BEGIN
    SELECT retention_policy_id INTO autorizou FROM sos_incident WHERE id = 'a2-valido';
    IF autorizou = 'incidente_sos@BR' THEN
        RAISE NOTICE 'OK 4: incidente registra a política aplicada (%)', autorizou;
    ELSE
        RAISE EXCEPTION 'FALHA: política aplicada não registrada (%)', coalesce(autorizou,'NULL');
    END IF;
END $$;

-- 5) Chave do incidente é por tenant: o mesmo id não é tomado por outro tenant.
DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES ('a2-valido','t2','p2','persisted_local','incidente_sos','BR');
    RAISE EXCEPTION 'FALHA: id de um tenant aceito em outro tenant';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 5: colisão de id entre tenants recusada';
END $$;

-- 6) PK é composta por (tenant_id, id).
DO $$
DECLARE
    colunas text;
BEGIN
    SELECT string_agg(a.attname, ',' ORDER BY k.ord) INTO colunas
      FROM pg_constraint c
      CROSS JOIN LATERAL unnest(c.conkey) WITH ORDINALITY AS k(attnum, ord)
      JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum
     WHERE c.conname = 'sos_incident_pkey';
    IF colunas = 'tenant_id,id' THEN
        RAISE NOTICE 'OK 6: PK de sos_incident é (tenant_id, id)';
    ELSE
        RAISE EXCEPTION 'FALHA: PK inesperada (%)', coalesce(colunas,'NULL');
    END IF;
END $$;

-- 7) Remover a política referenciada por um incidente é recusado (sem órfão).
DO $$
BEGIN
    DELETE FROM retention_class
     WHERE class = 'incidente_sos' AND jurisdiction = 'BR';
    RAISE EXCEPTION 'FALHA: política usada por incidente foi removida';
EXCEPTION
    WHEN foreign_key_violation THEN RAISE NOTICE 'OK 7: política em uso não pode ser removida (FK)';
END $$;

SELECT 'verificação concluída' AS resultado;