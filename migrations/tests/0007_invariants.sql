-- Verificação de invariantes da migração 0007 (emissão de grant).
-- Requer o esquema vigente: `0001_core.sql` ... `0007_grant_issuance.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável e
-- falha com exit != 0 se algum bloco `OK` faltar.
--
-- Esperado: 5 blocos `OK` (1, 2, 3, 4, 5). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) O hash é a única forma de identificar o token: formato sha256 hex exigido.
DO $$
BEGIN
    INSERT INTO grant_issued (token_id, tenant_id, principal, token_hash, scopes,
                              not_before, expires_at, authority_epoch)
    VALUES ('g-hash-curto', 't', 'p', 'nao-e-sha256', ARRAY['incident:read'],
            now(), now() + interval '1 hour', 1);
    RAISE EXCEPTION 'FALHA: token_hash fora do formato sha256 hex foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 1: token_hash fora do formato sha256 hex recusado (CHECK)';
END $$;

-- 2) Dois grants não podem compartilhar o mesmo hash (o token é a credencial).
DO $$
BEGIN
    INSERT INTO grant_issued (token_id, tenant_id, principal, token_hash, scopes,
                              not_before, expires_at, authority_epoch)
    VALUES ('g-h1', 't', 'p', repeat('a', 64), ARRAY['incident:read'],
            now(), now() + interval '1 hour', 1);
    INSERT INTO grant_issued (token_id, tenant_id, principal, token_hash, scopes,
                              not_before, expires_at, authority_epoch)
    VALUES ('g-h2', 't', 'p', repeat('a', 64), ARRAY['incident:read'],
            now(), now() + interval '1 hour', 1);
    RAISE EXCEPTION 'FALHA: hash de token duplicado foi aceito';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 2: hash de token duplicado recusado (UNIQUE)';
END $$;

-- 3) Grant sem escopo não autoriza nada e não é emitido.
DO $$
BEGIN
    INSERT INTO grant_issued (token_id, tenant_id, principal, token_hash, scopes,
                              not_before, expires_at, authority_epoch)
    VALUES ('g-sem-escopo', 't', 'p', repeat('b', 64), ARRAY[]::text[],
            now(), now() + interval '1 hour', 1);
    RAISE EXCEPTION 'FALHA: grant sem escopo foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 3: grant sem escopo recusado (CHECK)';
END $$;

-- 4) Janela invertida não é grant: `expires_at` precisa ser posterior.
DO $$
BEGIN
    INSERT INTO grant_issued (token_id, tenant_id, principal, token_hash, scopes,
                              not_before, expires_at, authority_epoch)
    VALUES ('g-janela', 't', 'p', repeat('c', 64), ARRAY['incident:read'],
            now() + interval '1 hour', now(), 1);
    RAISE EXCEPTION 'FALHA: janela invertida foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 4: janela invertida recusada (CHECK)';
END $$;

-- 5) O token não tem coluna: a tabela só oferece hash. A consulta que o
--    servidor usa devolve metadados e hash, nunca o token.
DO $$
DECLARE
    colunas text;
    esperado constant text := 'authority_epoch,created_at,expires_at,not_before,principal,scopes,tenant_id,token_hash,token_id';
BEGIN
    SELECT string_agg(column_name, ',' ORDER BY column_name) INTO colunas
      FROM information_schema.columns
     WHERE table_name = 'grant_issued';
    IF colunas IS DISTINCT FROM esperado THEN
        RAISE EXCEPTION 'FALHA: colunas de grant_issued não batem com o contrato: %', colunas;
    END IF;
    RAISE NOTICE 'OK 5: grant_issued guarda token_hash e nenhuma coluna de token';
END $$;