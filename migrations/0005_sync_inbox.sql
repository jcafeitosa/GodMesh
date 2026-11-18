-- Migração 0005 — checkpoint do Inbox de sync (ADR-057/061).
--
-- Numeração: a fatia de sync chegou depois de 0003_outbox_publisher e
-- 0004_grant_revocation; o conteúdo é o mesmo, o nome não colide.
--
-- `event_position` já é a posição durável do evento (0001). O que faltava era o
-- registro explícito do **maior prefixo contíguo** aceito por namespace de
-- Inbox: sem ele, cada leitura teria de reconstruir o prefixo e não existiria
-- ponto único para a atualização atômica que acompanha a inserção dos eventos.
--
-- Namespace de Inbox = `(tenant_id, origin_node, stream_id, partition,
-- authority_epoch)`. `partition` e `authority_epoch` são obrigatórios e fazem
-- parte da chave (ADR-057); não existe prefixo "global" de stream.
--
-- O prefixo só avança dentro da mesma transação que insere as posições
-- aceitas (`POST /v1/sync/inbox`). Esta tabela não é cache descartável: ela é o
-- checkpoint persistido que sobrevive ao reinício do processo.

-- Sem BEGIN/COMMIT: o runner é dono da transação (contrato de migrations).

CREATE TABLE sync_inbox_state (
    tenant_id         text        NOT NULL,
    origin_node       text        NOT NULL,
    stream_id         text        NOT NULL,
    partition         integer     NOT NULL CHECK (partition >= 0),
    authority_epoch   bigint      NOT NULL CHECK (authority_epoch > 0),
    contiguous_prefix bigint      NOT NULL CHECK (contiguous_prefix >= 0),
    updated_at        timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT sync_inbox_state_pkey PRIMARY KEY (
        tenant_id, origin_node, stream_id, partition, authority_epoch
    ),
    CONSTRAINT sync_inbox_state_nonempty_text CHECK (
        tenant_id <> '' AND origin_node <> '' AND stream_id <> ''
    )
);

COMMENT ON TABLE sync_inbox_state IS
    'ADR-057/061: maior prefixo contíguo aceito por namespace de Inbox (tenant, origem, stream, partição, epoch). Prefixo 0 = nenhum evento durável; a tabela sem linha é tratada como prefixo 0, não como erro.';

COMMENT ON COLUMN sync_inbox_state.contiguous_prefix IS
    'Maior `p` tal que todo `1..=p` está durável em `event_position` para o namespace. Nunca decresce.';
