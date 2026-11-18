-- Migração 0008 — federação: registro de pares e pedidos de ingresso.
--
-- Primeira fatia da malha (ADR-071): identidade, manifesto assinado e ingresso
-- por API. Não há transporte aqui — QUIC/mTLS é a fatia seguinte (M2). O que
-- estas tabelas guardam é o que o servidor pode afirmar hoje: quem pediu
-- ingresso, com qual manifesto assinado, e qual foi a decisão administrativa.
--
-- Decisões de esquema:
--
-- * `federation_peer.node_id` é a PK porque é o identificador derivado da chave
--   pública (`gmn_` + sha256 hex). Um nó tem **um** registro de par; a coluna
--   `peer_id` existe separada para que a fatia de transporte possa distinguir
--   registros do mesmo nó (endpoint/relay) sem migrar dado.
-- * `public_key` é dado público: fica em claro e em hex minúsculo, é o que
--   permite verificar manifestos. **Nenhuma** coluna guarda chave privada.
-- * `trust_state` é fechado por CHECK: estado fora do contrato não entra no
--   banco, e quem lê trata valor desconhecido como erro, nunca como autorizado.
-- * `federation_join_request` é idempotente por `(node_id, epoch)`: reenviar o
--   mesmo manifesto não cria segundo pedido e não reescreve `requested_at`.
-- * `approved_at`/`approved_by` só existem junto de `state = 'approved'`: a
--   coerência é do banco, não da aplicação.
--
-- Sem BEGIN/COMMIT: o runner é dono da transação (contrato de migrations).

CREATE TABLE federation_peer (
    -- Identificador derivado da chave pública do nó.
    node_id          text        PRIMARY KEY CHECK (btrim(node_id) <> ''),
    -- Identificador do registro de par. Nesta fatia é igual a `node_id`.
    peer_id          text        NOT NULL UNIQUE CHECK (btrim(peer_id) <> ''),
    -- Chave pública Ed25519 em hex minúsculo (32 bytes). Dado público.
    public_key       text        NOT NULL CHECK (public_key ~ '^[0-9a-f]{64}$'),
    cluster          text        NOT NULL CHECK (btrim(cluster) <> ''),
    region           text        NOT NULL CHECK (btrim(region) <> ''),
    -- Estado de confiança; a máquina de estados é validada na aplicação
    -- (godmesh_federation::peer::TrustState) e o domínio de valores aqui.
    trust_state      text        NOT NULL
                     CHECK (trust_state IN ('pending', 'authorized', 'revoked', 'unknown')),
    endpoints        text[]      NOT NULL DEFAULT '{}',
    software_version text        NOT NULL CHECK (btrim(software_version) <> ''),
    capabilities     text[]      NOT NULL DEFAULT '{}',
    epoch            bigint      NOT NULL CHECK (epoch >= 0),
    last_seen_at     timestamptz NOT NULL DEFAULT now(),
    updated_at       timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE federation_peer IS
    'Registro de pares da federação (ADR-071). Guarda apenas identidade pública e estado de confiança; nenhuma chave privada.';

COMMENT ON COLUMN federation_peer.node_id IS
    'Identificador derivado da chave pública: gmn_ + hex de sha256(pubkey). Só entra no registro depois de manifesto verificado.';

COMMENT ON COLUMN federation_peer.peer_id IS
    'Identificador do registro de par. Nesta fatia igual a node_id; separado para a fatia de transporte (M2) distinguir endpoints do mesmo nó.';

COMMENT ON COLUMN federation_peer.trust_state IS
    'pending | authorized | revoked | unknown. Apenas authorized permite receber dado privado; a transição é validada na aplicação.';

COMMENT ON COLUMN federation_peer.last_seen_at IS
    'Último contato comprovado (segundos Unix no contrato da API). Não é prova de frescor: quem decide isso é a janela de issued_at do manifesto.';

CREATE INDEX federation_peer_trust_state_idx ON federation_peer (trust_state);

CREATE TABLE federation_join_request (
    -- Identificador público do pedido: fjr_ + 32 hex.
    join_request_id text        PRIMARY KEY
                    CHECK (join_request_id ~ '^fjr_[0-9a-f]{32}$'),
    node_id         text        NOT NULL CHECK (btrim(node_id) <> ''),
    -- Geração da identidade de federação. Parte da chave de idempotência.
    epoch           bigint      NOT NULL CHECK (epoch >= 0),
    -- Manifesto assinado como chegou. Serve de evidência do que foi pedido;
    -- a assinatura é reverificável a partir dele.
    manifest        jsonb       NOT NULL CHECK (jsonb_typeof(manifest) = 'object'),
    requested_at    timestamptz NOT NULL DEFAULT now(),
    approved_at     timestamptz,
    -- Quem aprovou (principal do grant com escopo federation:admin).
    approved_by     text,
    state           text        NOT NULL DEFAULT 'pending'
                    CHECK (state IN ('pending', 'approved', 'rejected', 'expired')),

    -- Idempotência do ingresso: um pedido por (nó, geração).
    CONSTRAINT federation_join_request_node_epoch UNIQUE (node_id, epoch),
    -- Aprovação e seus carimbos existem juntos, ou não existem.
    CONSTRAINT federation_join_request_approved_at_coherent
        CHECK ((state = 'approved') = (approved_at IS NOT NULL)),
    CONSTRAINT federation_join_request_approved_by_coherent
        CHECK ((state = 'approved') = (approved_by IS NOT NULL)),
    -- Carimbo só é coerente com o estado: `approved_by` sem aprovação nem
    -- aprovação sem autor é recusado acima; aqui o aprovador não pode ser vazio.
    CONSTRAINT federation_join_request_approved_by_nonempty
        CHECK (approved_by IS NULL OR btrim(approved_by) <> '')
);

COMMENT ON TABLE federation_join_request IS
    'Pedidos de ingresso na federação (ADR-071). Idempotente por (node_id, epoch); aprovação cria/atualiza federation_peer, rejeição não.';

COMMENT ON COLUMN federation_join_request.manifest IS
    'Manifesto assinado (jsonb) exatamente como recebido. A chave privada nunca está aqui: o manifesto só carrega a chave pública.';

COMMENT ON COLUMN federation_join_request.state IS
    'pending | approved | rejected | expired. approved_at/approved_by só podem existir em approved (CHECKs de coerência).';

CREATE INDEX federation_join_request_state_idx
    ON federation_join_request (state, requested_at DESC);