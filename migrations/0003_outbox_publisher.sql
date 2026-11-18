-- Migração 0003 — publicador durável do outbox (ADR-061).
--
-- Motivo do delta:
--   * `outbox_entry` (0001) não registrava tentativa de efeito; sem `attempts`
--     uma falha do efeito era indistinguível de "nunca tentado";
--   * nada impedia no banco um `confirmed_at` sem `published_at` — a regra
--     "publicar não é confirmar" só existia em código;
--   * não havia efeito de teste para a publicação, então o publicador só podia
--     ser exercitado contra uma integração externa inexistente.
--
-- O que esta migração NÃO faz: não cria broker, fila, bridge, retry infinito
-- nem garantia de entrega. `outbox_sink` é um sink de TESTE local (tabela).
--
-- Transação: o arquivo NÃO abre `BEGIN`/`COMMIT`. Quem é dono da transação é o
-- runner (`src/migrate.rs`), que envia o arquivo + o registro de controle em um
-- único script, executado pelo Postgres como bloco transacional implícito. Um
-- `BEGIN` aqui aninharia/quebraria esse bloco.

-- Contador de tentativas de execução do efeito. Fica em 0 até a primeira falha.
ALTER TABLE outbox_entry
    ADD COLUMN attempts integer NOT NULL DEFAULT 0
        CONSTRAINT outbox_entry_attempts_nonnegative CHECK (attempts >= 0);

COMMENT ON COLUMN outbox_entry.attempts IS
    'ADR-061: tentativas de efeito que falharam. published_at é a reserva de publicação; confirmed_at só existe depois do efeito (regra do CHECK abaixo).';

-- Regra dura (a) no próprio banco: confirmação sem publicação é impossível.
-- O código também recusa, mas a constraint garante que nenhum caminho — script,
-- migração futura ou escrita manual — produza a linha proibida.
ALTER TABLE outbox_entry
    ADD CONSTRAINT outbox_entry_confirmed_requires_published
        CHECK (confirmed_at IS NULL OR published_at IS NOT NULL);

-- Índice do publicador: só o que ainda não foi confirmado, na ordem de sequência
-- dentro do grupo `(origin, tenant, stream, partition, epoch)`.
CREATE INDEX outbox_entry_pendente
    ON outbox_entry (origin_node, tenant_id, stream_id, partition, authority_epoch, seq)
    WHERE confirmed_at IS NULL;

COMMENT ON INDEX outbox_entry_pendente IS
    'ADR-061: varredura do publicador por grupo e sequência, restrita a pendentes.';

-- Sink de TESTE (não é integração externa). Grava o que o publicador entregou,
-- para que a publicação e a confirmação sejam observáveis por teste local sem
-- inventar bridge, rádio ou notificação. A PK impede entrega duplicada da mesma
-- posição: é o detector de publicação repetida.
CREATE TABLE outbox_sink (
    origin_node     text        NOT NULL,
    tenant_id       text        NOT NULL,
    stream_id       text        NOT NULL,
    partition       integer     NOT NULL,
    authority_epoch bigint      NOT NULL,
    seq             bigint      NOT NULL,
    effect_kind     text        NOT NULL CHECK (effect_kind IN ('event','irreversible')),
    delivered_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq)
);

COMMENT ON TABLE outbox_sink IS
    'Sink de TESTE do publicador (ADR-061). Não representa integração externa e não prova entrega a bridge, rádio ou notificação.';
