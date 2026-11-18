-- Verificação de invariantes da migração 0008 (federação: pares e ingressos).
-- Requer o esquema vigente: `0001_core.sql` ... `0008_federation_peers.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável e
-- falha com exit != 0 se algum bloco `OK` faltar.
--
-- Esperado: 6 blocos `OK` (1, 2, 3, 4, 5, 6). Qualquer outro resultado é regressão.

\set ON_ERROR_STOP off

-- 1) `approved_at` sem `state = 'approved'` é incoerência: pedido rejeitado não
--    tem instante de aprovação.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        approved_at, state)
    VALUES ('fjr_00000000000000000000000000000001', 'gmn_no1', 1, '{}'::jsonb,
            now(), 'rejected');
    RAISE EXCEPTION 'FALHA: approved_at em pedido não aprovado foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 1: approved_at em pedido não aprovado recusado (CHECK)';
END $$;

-- 2) `approved_by` sem aprovação também é incoerência (quem aprovou o quê?).
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        approved_by, state)
    VALUES ('fjr_00000000000000000000000000000002', 'gmn_no2', 1, '{}'::jsonb,
            'operador', 'pending');
    RAISE EXCEPTION 'FALHA: approved_by em pedido pendente foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 2: approved_by em pedido não aprovado recusado (CHECK)';
END $$;

-- 3) Estado fora do contrato não entra no banco.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest, state)
    VALUES ('fjr_00000000000000000000000000000003', 'gmn_no3', 1, '{}'::jsonb, 'aprovado');
    RAISE EXCEPTION 'FALHA: estado inventado em join request foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 3: estado inventado em join request recusado (CHECK)';
END $$;

-- 4) Idempotência por (node_id, epoch): o mesmo nó não acumula pedidos da mesma
--    geração.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest)
    VALUES ('fjr_00000000000000000000000000000004', 'gmn_no4', 7, '{}'::jsonb);
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest)
    VALUES ('fjr_00000000000000000000000000000005', 'gmn_no4', 7, '{}'::jsonb);
    RAISE EXCEPTION 'FALHA: pedido duplicado por (node_id, epoch) foi aceito';
EXCEPTION
    WHEN unique_violation THEN RAISE NOTICE 'OK 4: pedido duplicado por (node_id, epoch) recusado (UNIQUE)';
END $$;

-- 5) `trust_state` é fechado: par com estado fora do contrato não existe.
DO $$
BEGIN
    INSERT INTO federation_peer (node_id, peer_id, public_key, cluster, region,
                                 trust_state, software_version, epoch)
    VALUES ('gmn_no5', 'gmn_no5', repeat('a', 64), 'c', 'r',
            'trusted', '0.1.0', 1);
    RAISE EXCEPTION 'FALHA: trust_state inventado foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 5: trust_state inventado recusado (CHECK)';
END $$;

-- 6) Nenhuma coluna de chave privada (nem qualquer segredo) nas tabelas de
--    federação: a chave privada vive só no arquivo do nó.
DO $$
DECLARE
    colunas text;
    esperado constant text := 'capabilities,cluster,endpoints,epoch,last_seen_at,node_id,peer_id,public_key,region,software_version,trust_state,updated_at';
BEGIN
    SELECT string_agg(column_name, ',' ORDER BY column_name) INTO colunas
      FROM information_schema.columns
     WHERE table_name = 'federation_peer';
    IF colunas IS DISTINCT FROM esperado THEN
        RAISE EXCEPTION 'FALHA: colunas de federation_peer não batem com o contrato: %', colunas;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name IN ('federation_peer', 'federation_join_request')
           AND (column_name LIKE '%secret%' OR column_name LIKE '%private%'
                OR column_name LIKE '%seed%')
    ) THEN
        RAISE EXCEPTION 'FALHA: tabela de federação expõe coluna de segredo';
    END IF;
    RAISE NOTICE 'OK 6: tabelas de federação guardam apenas chave pública';
END $$;