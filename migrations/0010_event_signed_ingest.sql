-- Migração 0010 — S-sec-2: escopo de ingest por nó (evento assinado).
--
-- Fatia S-sec-2 de `docs/04-seguranca/abuso-e-malha-maligna.md` §5/§8: evento
-- sem assinatura de nó autorizado **não entra no log durável**. A assinatura é
-- verificada contra a chave registrada em `federation_peer` (0008); esta
-- migração cria o degrau que faltava entre "o nó é autorizado" e "o nó pode
-- escrever ESTA stream DESTE tenant": `federation_peer_scope`.
--
-- A numeração segue a reserva do CTO: **0009 pertence à M1b** (aprovação de
-- ingresso assinada por nó autorizado — emenda ADR-071), que ainda não tem
-- arquivo neste checkout. O runner ordena por nome e `0009…` vem antes de
-- `0010…` mesmo quando criado depois, então nenhuma reordenação é necessária.
--
-- Decisões de esquema:
--
-- * **tabela, não `text[]` em `federation_peer`**: o escopo é o controle que
--   um operador concede e revoga item por item (§6 pede auditoria de "negação
--   de escopo"). Em array, revogar um par remove todos; em tabela, cada linha
--   é uma concessão com FK que o banco confere, indexável no caminho quente
--   do ingest por `(tenant_id, stream_id)`, e pronta para ganhar `expires_at`
--   quando a S-sec-3 (validade/provação) chegar. Um `text[]` também aceitaria
--   grafia errada de tenant em silêncio — FK não.
-- * **granularidade exata `(node_id, tenant_id, stream_id)`**, sem curinga:
--   §5 da matriz de ameaças nomeia essa tupla. Curinga (`*`) é afrouxamento
--   de escopo e exigiria ADR própria; enquanto não existe, escopo é literal.
-- * FK **RESTRICT**: apagar um par que ainda tem escopos concedidos exige
--   revogar os escopos primeiro — o histórico de concessões não desaparece por
--   cascade de uma limpeza de registro.
-- * `federation_peer_scope` não guarda chave nem segredo; só nomes.
--
-- Sem BEGIN/COMMIT: o runner é dono da transação (contrato de migrations).

CREATE TABLE federation_peer_scope (
    node_id    text        NOT NULL REFERENCES federation_peer (node_id),
    tenant_id  text        NOT NULL CHECK (btrim(tenant_id) <> ''),
    stream_id  text        NOT NULL CHECK (btrim(stream_id) <> ''),
    granted_at timestamptz NOT NULL DEFAULT now(),

    -- Uma concessão por tupla: reexecutar a concessão é no-op estrutural,
    -- nunca "segunda cópia" do mesmo escopo.
    PRIMARY KEY (node_id, tenant_id, stream_id)
);

COMMENT ON TABLE federation_peer_scope IS
    'Escopo de ingest por nó (S-sec-2): tupla exata (nó, tenant, stream) que o nó assinante pode escrever. Ausência de linha é negação (falha fechada).';

COMMENT ON COLUMN federation_peer_scope.node_id IS
    'Nó da federação já registrado em federation_peer. A FK RESTRICT impede apagar par com escopo ainda concedido.';

COMMENT ON COLUMN federation_peer_scope.granted_at IS
    'Instante da concessão. Não é validade: expiração/renovação é S-sec-3 e exige coluna nova, não reinterpretação desta.';

-- Lookup do ingest: "este nó pode escrever esta stream deste tenant?".
CREATE INDEX federation_peer_scope_tenant_stream_idx
    ON federation_peer_scope (tenant_id, stream_id, node_id);
