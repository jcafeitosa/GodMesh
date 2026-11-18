-- Migração 0006 — chave de paginação estável da listagem de incidentes.
--
-- Motivo do delta: `GET /v1/incidents` precisa de paginação por cursor que não
-- repita nem perca linha entre páginas. `sos_incident` não tinha nenhuma coluna
-- estritamente monotônica: `created_at` empata (o `now()` é o instante da
-- transação, e dois incidentes da mesma transação têm o mesmo valor) e `id` é
-- texto, então `(created_at, id)` não cabe numa posição numérica de cursor.
-- Paginação por `OFFSET` foi recusada: com inserção concorrente ela repete e
-- perde linhas.
--
-- `list_seq` é atribuída pelo banco no INSERT (identity), nunca pelo cliente.
-- Ela é a ordem de inserção — e, como `created_at` também é atribuído no INSERT,
-- listar `list_seq DESC` é "mais recente primeiro" sem empate. O cursor carrega
-- apenas o último `list_seq` entregue; a página seguinte é `list_seq < cursor`.
--
-- Transação: o arquivo NÃO abre `BEGIN`/`COMMIT`. Quem é dono da transação é o
-- runner (`crates/godmesh-server/src/migrate.rs`), que envia o arquivo + o
-- registro de controle em um único script, executado pelo Postgres como bloco
-- transacional implícito.

ALTER TABLE sos_incident
    ADD COLUMN list_seq bigint GENERATED ALWAYS AS IDENTITY;

COMMENT ON COLUMN sos_incident.list_seq IS
    'Ordem de inserção atribuída pelo banco. Chave de paginação estável de GET /v1/incidents (nunca vem do cliente).';

-- Varredura da listagem por tenant na ordem da paginação.
CREATE INDEX sos_incident_listagem
    ON sos_incident (tenant_id, list_seq DESC);