-- Verificação de invariantes da migração 0009 (aprovação assinada por nó).
-- Requer o esquema vigente: `0001_core.sql` ... `0009_federation_approval.sql`.
--
-- Execute com `migrations/tests/run_all.sh`, que cria um banco descartável e
-- falha com exit != 0 se algum bloco `OK` faltar.
--
-- Esperado: 7 blocos `OK` (1..7). Qualquer outro resultado é regressão.
--
-- O que estes blocos provam é a **forma** da prova de aprovação (o banco recusa
-- aprovação sem certificado e recusa certificado em pedido não aprovado). Que
-- o aprovador está `authorized` não é verificável por constraint — depende de
-- `federation_peer` e do genesis declarado, e é provado na transação
-- (`crates/godmesh-server/tests/federation.rs`).

\set ON_ERROR_STOP off

-- 1) `approved` sem as colunas de prova é recusa do banco, não da aplicação.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_at, approved_by)
    VALUES ('fjr_00000000000000000000000000000101', 'gmn_no101', 1, '{}'::jsonb,
            'approved', now(), 'operador');
    RAISE EXCEPTION 'FALHA: aprovação sem certificado de nó foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 1: approved sem prova assinada recusado (CHECK)';
END $$;

-- 2) Prova de aprovação em pedido **não aprovado** é incoerência: rejeição não
--    fabrica certificado.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_by_node)
    VALUES ('fjr_00000000000000000000000000000102', 'gmn_no102', 1, '{}'::jsonb,
            'rejected', 'gmn_' || repeat('a', 64));
    RAISE EXCEPTION 'FALHA: approved_by_node em pedido rejeitado foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 2: certificado de aprovação em pedido não aprovado recusado (CHECK)';
END $$;

-- 3) Aprovação incompleta (assinatura ausente) não vale como evidência.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_at, approved_by,
                                        approved_by_node, approval_epoch, approval_expires_at)
    VALUES ('fjr_00000000000000000000000000000103', 'gmn_no103', 1, '{}'::jsonb,
            'approved', now(), 'operador',
            'gmn_' || repeat('b', 64), 4, now() + interval '1 hour');
    RAISE EXCEPTION 'FALHA: aprovação sem approval_signature foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 3: aprovação incompleta (sem assinatura) recusada (CHECK)';
END $$;

-- 4) Aprovador com identificador fora da forma canônica é recusado: node_id
--    "quase igual" não pode virar autoridade.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_at, approved_by,
                                        approved_by_node, approval_signature,
                                        approval_epoch, approval_expires_at)
    VALUES ('fjr_00000000000000000000000000000104', 'gmn_no104', 1, '{}'::jsonb,
            'approved', now(), 'operador',
            'gmn_quase_igual', repeat('c', 128), 4, now() + interval '1 hour');
    RAISE EXCEPTION 'FALHA: approved_by_node fora do formato gmn_+64hex foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 4: node_id do aprovador fora do formato recusado (CHECK)';
END $$;

-- 5) Assinatura com tamanho/alfabeto errados é recusada antes de qualquer
--    verificação criptográfica: 128 hex minúsculos ou nada.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_at, approved_by,
                                        approved_by_node, approval_signature,
                                        approval_epoch, approval_expires_at)
    VALUES ('fjr_00000000000000000000000000000105', 'gmn_no105', 1, '{}'::jsonb,
            'approved', now(), 'operador',
            'gmn_' || repeat('d', 64), upper(repeat('e', 128)), 4,
            now() + interval '1 hour');
    RAISE EXCEPTION 'FALHA: assinatura em hex maiúsculo (fora do contrato) foi aceita';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 5: approval_signature fora do formato recusada (CHECK)';
END $$;

-- 6) `epoch` negativo não existe: o piso anti-rollback começa em zero.
DO $$
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_at, approved_by,
                                        approved_by_node, approval_signature,
                                        approval_epoch, approval_expires_at)
    VALUES ('fjr_00000000000000000000000000000106', 'gmn_no106', 1, '{}'::jsonb,
            'approved', now(), 'operador',
            'gmn_' || repeat('f', 64), repeat('0', 128), -1,
            now() + interval '1 hour');
    RAISE EXCEPTION 'FALHA: approval_epoch negativo foi aceito';
EXCEPTION
    WHEN check_violation THEN RAISE NOTICE 'OK 6: approval_epoch negativo recusado (CHECK)';
END $$;

-- 7) O caminho positivo: aprovação com o certificado completo entra, e o
--    banco **não** guarda nada que se pareça com chave privada. Sem este
--    bloco, os seis anteriores poderiam ser satisfeitos por um CHECK que
--    recusasse tudo.
DO $$
DECLARE
    colunas text;
    total bigint;
BEGIN
    INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest,
                                        state, approved_at, approved_by,
                                        approved_by_node, approval_signature,
                                        approval_epoch, approval_expires_at)
    VALUES ('fjr_00000000000000000000000000000107', 'gmn_no107', 1, '{}'::jsonb,
            'approved', now(), 'operador',
            'gmn_' || repeat('1', 64), repeat('2', 128), 7,
            now() + interval '1 hour');

    SELECT count(*) INTO total FROM federation_join_request
     WHERE join_request_id = 'fjr_00000000000000000000000000000107'
       AND approval_epoch = 7;
    IF total <> 1 THEN
        RAISE EXCEPTION 'FALHA: aprovação completa não foi persistida como gravada';
    END IF;

    SELECT string_agg(column_name, ',' ORDER BY column_name) INTO colunas
      FROM information_schema.columns
     WHERE table_name = 'federation_join_request';
    IF colunas IS DISTINCT FROM
       'approval_epoch,approval_expires_at,approval_signature,approved_at,approved_by,'
       || 'approved_by_node,epoch,join_request_id,manifest,node_id,requested_at,state' THEN
        RAISE EXCEPTION 'FALHA: colunas de federation_join_request divergem do contrato: %', colunas;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name IN ('federation_peer', 'federation_join_request')
           AND (column_name LIKE '%secret%' OR column_name LIKE '%private%'
                OR column_name LIKE '%seed%')
    ) THEN
        RAISE EXCEPTION 'FALHA: tabela de federação expõe coluna de segredo';
    END IF;
    RAISE NOTICE 'OK 7: aprovação com certificado completo persiste e nenhuma coluna guarda segredo';
END $$;
