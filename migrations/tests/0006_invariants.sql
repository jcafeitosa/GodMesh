-- Verificação de invariantes da migração 0006 (chave de paginação estável).
-- Requer o esquema vigente: `0001_core.sql` … `0006_incident_listing.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável,
-- aplica todas as migrações e falha com exit != 0 se algum bloco `OK` faltar.
-- Rodar `psql` direto não basta: com `ON_ERROR_STOP off` o psql devolve 0 mesmo
-- depois de um ERROR, então a contagem dos `NOTICE: OK` é a prova.
--
-- Esperado: 3 blocos `OK` (1, 2, 3). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) `list_seq` é atribuída pelo banco e cresce na ordem de inserção — é isso
-- que faz `list_seq DESC` ser paginação sem empate.
DO $$
DECLARE
    primeiro bigint;
    segundo  bigint;
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES ('a6-ordem-1','t6','p6','persisted_local','incidente_sos','BR')
    RETURNING list_seq INTO primeiro;
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES ('a6-ordem-2','t6','p6','persisted_local','incidente_sos','BR')
    RETURNING list_seq INTO segundo;

    IF primeiro IS NULL OR segundo IS NULL OR segundo <= primeiro THEN
        RAISE EXCEPTION 'FALHA: list_seq não cresce na inserção (% -> %)', primeiro, segundo;
    END IF;
    RAISE NOTICE 'OK 1: list_seq atribuída pelo banco cresce na inserção (% -> %)', primeiro, segundo;
END $$;

-- 2) O cliente não escreve `list_seq`: valor explícito é recusado pelo banco.
-- A ordem da paginação não pode ser escolhida (nem forjada) por quem chama.
DO $$
BEGIN
    INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction, list_seq)
    VALUES ('a6-list-seq-forjada','t6','p6','persisted_local','incidente_sos','BR', 999999);
    RAISE EXCEPTION 'FALHA: list_seq aceitou valor explícito do cliente';
EXCEPTION
    WHEN generated_always THEN RAISE NOTICE 'OK 2: list_seq não aceita escrita explícita';
END $$;

-- 3) Índice da varredura da listagem por tenant existe e está na ordem publicada.
DO $$
DECLARE
    definicao text;
BEGIN
    SELECT indexdef INTO definicao
      FROM pg_indexes
     WHERE schemaname = current_schema()
       AND indexname = 'sos_incident_listagem';
    IF definicao IS NULL THEN
        RAISE EXCEPTION 'FALHA: índice sos_incident_listagem ausente';
    END IF;
    IF definicao NOT LIKE '%(tenant_id, list_seq DESC)%' THEN
        RAISE EXCEPTION 'FALHA: índice sos_incident_listagem com ordem inesperada: %', definicao;
    END IF;
    RAISE NOTICE 'OK 3: índice sos_incident_listagem (tenant_id, list_seq DESC) presente';
END $$;