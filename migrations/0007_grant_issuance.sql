-- Migração 0007 — grants emitidos pela API administrativa.
--
-- Motivo: até aqui o único caminho para existir uma credencial era editar
-- `GODMESH_GRANTS` e reiniciar. Isso impede um ciclo administrativo normal
-- (emitir, consultar, revogar) e força um reinício para toda credencial nova.
--
-- Decisão (ADR-026/ADR-031): nada de criptografia própria. O token continua
-- opaco e aleatório, gerado com RNG do sistema; o banco guarda **apenas** o
-- sha256 hex do token, nunca o token. O identificador público do grant
-- (`token_id`) é o que aparece em rota, auditoria e revogação.
--
-- O hash é comparado em tempo de requisição contra o hash calculado do token
-- apresentado; a revogação continua em `grant_revocation` (0004), consultada
-- pelo mesmo `token_id`. Emitir não exige reinício, revogar também não.
--
-- Sem BEGIN/COMMIT: o runner é dono da transação (contrato de migrations).

CREATE TABLE grant_issued (
    -- Identificador público do grant. É a chave usada em
    -- `grant_revocation.token_id` e o que aparece em URL/auditoria.
    token_id        text        PRIMARY KEY CHECK (btrim(token_id) <> ''),
    -- Sujeito do grant: tenancy e autoria nunca vêm do corpo da requisição.
    tenant_id       text        NOT NULL CHECK (btrim(tenant_id) <> ''),
    principal       text        NOT NULL CHECK (btrim(principal) <> ''),
    -- sha256 hex (64 caracteres) do token. O token em si não tem coluna aqui de
    -- propósito: não existe caminho de código que o persista.
    token_hash      text        NOT NULL UNIQUE CHECK (token_hash ~ '^[0-9a-f]{64}$'),
    -- Escopos concedidos, na forma do contrato (`incident:write`, ...).
    scopes          text[]      NOT NULL CHECK (cardinality(scopes) > 0),
    not_before      timestamptz NOT NULL,
    expires_at      timestamptz NOT NULL,
    authority_epoch bigint      NOT NULL CHECK (authority_epoch >= 0),
    created_at      timestamptz NOT NULL DEFAULT now(),
    -- Janela precisa ser positiva: grant sem prazo não é emitido.
    CONSTRAINT grant_issued_window_positive CHECK (expires_at > not_before)
);

COMMENT ON TABLE grant_issued IS
    'Grants emitidos pela API administrativa (ADR-026/ADR-031). Guarda apenas o sha256 hex do token; o token só existe na resposta da emissão.';

COMMENT ON COLUMN grant_issued.token_id IS
    'Identificador público do grant; chave de revogação em grant_revocation e de consulta administrativa.';

COMMENT ON COLUMN grant_issued.token_hash IS
    'sha256 hex do token opaco. Comparado em tempo de requisição; o token nunca é persistido.';

COMMENT ON COLUMN grant_issued.scopes IS
    'Escopos concedidos. Lista vazia é recusada: grant sem escopo não autoriza nada e não deve existir.';

COMMENT ON COLUMN grant_issued.expires_at IS
    'Fim da janela. O servidor recusa a credencial a partir deste instante (401 credential_expired).';