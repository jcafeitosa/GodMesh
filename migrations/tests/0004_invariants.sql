-- Verificação de invariantes da migração 0004 (revogação persistente).
-- Requer o esquema vigente: `0001_core.sql` + `0002_incident_tenancy.sql` +
-- `0004_grant_revocation.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável e
-- falha com exit != 0 se algum bloco `OK` faltar.
--
-- Esperado: 4 blocos `OK` (1, 2, 3, 4). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) A PK protege contra duplicação: dois grants não podem ser o mesmo token_id.
INSERT INTO grant_revocation (token_id, reason) VALUES ('g-1', 'rotacao');
DO $$
BEGIN
    INSERT INTO grant_revocation (token_id, reason) VALUES ('g-1', 'duplicado');
    RAISE EXCEPTION 'FALHA: revogação duplicada de token_id foi aceita';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 1: token_id duplicado recusado (PK)';
END $$;

-- 2) Motivo vazio é recusado: revogação sem justificativa não é auditável.
DO $$
BEGIN
    INSERT INTO grant_revocation (token_id, reason) VALUES ('g-2', '   ');
    RAISE EXCEPTION 'FALHA: revogação com motivo vazio foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 2: motivo vazio recusado (CHECK)';
END $$;

-- 3) `revoked_at` tem default: a revogação registra o instante sem depender do
--    cliente.
DO $$
DECLARE
    momento timestamptz;
BEGIN
    INSERT INTO grant_revocation (token_id, reason) VALUES ('g-3', 'incidente');
    SELECT revoked_at INTO momento FROM grant_revocation WHERE token_id = 'g-3';
    IF momento IS NULL THEN
        RAISE EXCEPTION 'FALHA: revoked_at sem default';
    END IF;
    RAISE NOTICE 'OK 3: revoked_at preenchido por default';
END $$;

-- 4) A leitura usada pelo servidor devolve o instante em segundos Unix e
--    distingue "não existe revogação" de "revogação no instante zero".
DO $$
DECLARE
    ausente  bigint;
    presente bigint;
BEGIN
    SELECT (EXTRACT(EPOCH FROM revoked_at))::bigint INTO ausente
      FROM grant_revocation WHERE token_id = 'g-nao-revogado';
    IF ausente IS NOT NULL THEN
        RAISE EXCEPTION 'FALHA: token não revogado devolveu instante';
    END IF;
    SELECT (EXTRACT(EPOCH FROM revoked_at))::bigint INTO presente
      FROM grant_revocation WHERE token_id = 'g-3';
    IF presente IS NULL OR presente <= 0 THEN
        RAISE EXCEPTION 'FALHA: token revogado sem instante Unix';
    END IF;
    RAISE NOTICE 'OK 4: consulta distingue ausente de revogado';
END $$;