-- Migração 0004 — revogação persistente de grant.
--
-- Motivo: hoje "autenticado" significa apenas "está em GODMESH_GRANTS". Sem uma
-- fonte de revogação durável, revogar exigiria reiniciar o processo e reeditar a
-- configuração — e o servidor não teria como provar que um token deixou de valer.
--
-- Decisão (ADR-026/ADR-031): o token continua opaco; não há JWT nem cifra
-- própria. A revogação é uma linha de banco consultada em tempo de requisição,
-- com cache curto no processo (5 s, `REVOCATION_CACHE_TTL_SECONDS`) e invalidação
-- explícita. O `token_id` identifica o grant; o token em si nunca é persistido
-- aqui.
--
-- Fail-closed: se esta consulta falhar, a requisição é negada (503
-- `revocation_unavailable`), nunca aceita por omissão.

-- Sem BEGIN/COMMIT: o runner é dono da transação (contrato de migrations).

CREATE TABLE grant_revocation (
    -- Identificador público do grant (`GrantRecord.token_id`). É a mesma chave
    -- usada no registro de configuração; o token secreto não entra no banco.
    token_id   text        PRIMARY KEY,
    revoked_at timestamptz NOT NULL DEFAULT now(),
    -- Motivo obrigatório e não vazio: revogação sem justificativa não é
    -- auditável e, portanto, não é aceita.
    reason     text        NOT NULL CHECK (btrim(reason) <> '')
);

COMMENT ON TABLE grant_revocation IS
    'Revogação persistente por token_id (ADR-026/ADR-031). Consultada em tempo de requisição, cache curto de 5 s; ausência de linha significa não revogado.';

COMMENT ON COLUMN grant_revocation.revoked_at IS
    'Instante da revogação. O servidor nega a credencial a partir deste instante, sem reiniciar.';

COMMENT ON COLUMN grant_revocation.reason IS
    'Justificativa operacional obrigatória; vazia é recusada pelo CHECK.';
