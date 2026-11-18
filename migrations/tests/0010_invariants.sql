-- Verificação de invariantes da migração 0010 (S-sec-2: escopo de ingest por nó).
-- Requer o esquema vigente: `0001_core.sql` ... `0010_event_signed_ingest.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável e
-- falha com exit != 0 se algum bloco `OK` faltar.
--
-- Esperado: 6 blocos `OK` (1, 2, 3, 4, 5, 6). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- Sem par registrado, nada a escopar: os blocos usam um nó sintético válido.
DO $$
BEGIN
    INSERT INTO federation_peer (node_id, peer_id, public_key, cluster, region,
                                 trust_state, software_version, epoch)
    VALUES ('gmn_escopo_base', 'gmn_escopo_base', repeat('a', 64), 'c', 'r',
            'pending', '0.1.0', 1)
    ON CONFLICT DO NOTHING;
END $$;

-- 1) Concessão duplicada da mesma tupla é recusada pela PK: escopo existe uma
--    vez, e "reexecutar a concessão" não fabrica segunda evidência.
DO $$
BEGIN
    INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id)
    VALUES ('gmn_escopo_base', 'tenant-x', 'stream-1');
    INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id)
    VALUES ('gmn_escopo_base', 'tenant-x', 'stream-1');
    RAISE EXCEPTION 'FALHA: escopo duplicado (node, tenant, stream) foi aceito';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 1: escopo duplicado recusado (PK)';
END $$;

-- 2) Escopo de nó inexistente é recuso pela FK: não existe concessão órfã —
--    quem recebe escopo tem de ter passado pelo registro de pares.
DO $$
BEGIN
    INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id)
    VALUES ('gmn_fantasma', 'tenant-x', 'stream-1');
    RAISE EXCEPTION 'FALHA: escopo para nó sem registro foi aceito';
EXCEPTION
    WHEN foreign_key_violation THEN RAISE NOTICE 'OK 2: escopo de nó não registrado recusado (FK)';
END $$;

-- 3) Tenant ou stream vazios não entram: escopo vazio seria curinga acidental
--    ("nó pode escrever qualquer stream") — exatamente o que a fatia nega.
DO $$
BEGIN
    INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id)
    VALUES ('gmn_escopo_base', '  ', 'stream-3');
    RAISE EXCEPTION 'FALHA: escopo com tenant vazio foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 3: escopo com tenant/stream vazio recusado (CHECK)';
END $$;

-- 4) RESTRICT, não CASCADE: apagar par com escopo ainda concedido falha. A
--    revogação explícita (apagar os escopos) vem antes da limpeza do registro,
--    senão o histórico de concessões some junto com o par.
DO $$
BEGIN
    INSERT INTO federation_peer (node_id, peer_id, public_key, cluster, region,
                                 trust_state, software_version, epoch)
    VALUES ('gmn_escopo_restrito', 'gmn_escopo_restrito', repeat('b', 64), 'c', 'r',
            'revoked', '0.1.0', 1)
    ON CONFLICT DO NOTHING;
    INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id)
    VALUES ('gmn_escopo_restrito', 'tenant-x', 'stream-4');
    DELETE FROM federation_peer WHERE node_id = 'gmn_escopo_restrito';
    RAISE EXCEPTION 'FALHA: par com escopo concedido foi apagado por cascade';
EXCEPTION
    WHEN foreign_key_violation THEN
        DELETE FROM federation_peer_scope WHERE node_id = 'gmn_escopo_restrito';
        DELETE FROM federation_peer WHERE node_id = 'gmn_escopo_restrito';
        RAISE NOTICE 'OK 4: apagar par com escopo ativo recusado (FK RESTRICT)';
END $$;

-- 5) O lookup do ingest existe como índice: a negativa é consultada no caminho
--    quente (antes de qualquer INSERT) e não pode virar scan crescente.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
         WHERE tablename = 'federation_peer_scope'
           AND indexdef LIKE '%(tenant_id, stream_id, node_id)%'
    ) THEN
        RAISE EXCEPTION 'FALHA: índice de lookup do ingest (tenant, stream, node) ausente';
    END IF;
    RAISE NOTICE 'OK 5: índice de lookup do ingest presente';
END $$;

-- 6) A tabela de escopo guarda apenas nomes: nenhuma coluna de chave/segredo
--    entra na malha por esta migração (a chave privada vive só no arquivo do nó).
DO $$
DECLARE
    colunas text;
    esperado constant text := 'granted_at,node_id,stream_id,tenant_id';
BEGIN
    SELECT string_agg(column_name, ',' ORDER BY column_name) INTO colunas
      FROM information_schema.columns
     WHERE table_name = 'federation_peer_scope';
    IF colunas IS DISTINCT FROM esperado THEN
        RAISE EXCEPTION 'FALHA: colunas de federation_peer_scope não batem com o contrato: %', colunas;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'federation_peer_scope'
           AND (column_name LIKE '%secret%' OR column_name LIKE '%private%'
                OR column_name LIKE '%seed%' OR column_name LIKE '%key%')
    ) THEN
        RAISE EXCEPTION 'FALHA: tabela de escopo expõe coluna de segredo';
    END IF;
    RAISE NOTICE 'OK 6: escopo por nó guarda apenas nomes, nenhum segredo';
END $$;
