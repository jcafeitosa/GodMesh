-- Migração 0009 — federação: aprovação assinada por nó autorizado (ADR-071,
-- emenda §2/§5; fatia M1b).
--
-- A emenda mudou a natureza da evidência: aprovar um `join-request` deixa de
-- ser "um humano com escopo `federation:admin` apertou o botão" e passa a ser
-- "um **nó autorizado** assinou a decisão". O escopo humano continua exigido —
-- ele autoriza a *operação* — mas sozinho não prova autoridade de ingresso.
-- Estas colunas são a segunda prova, e são verificáveis por qualquer nó que
-- conheça a chave pública do aprovador (`godmesh_federation::ApprovalCertificate`).
--
-- Decisões de esquema:
--
-- * **Quatro colunas, todas ligadas ao mesmo fato.** `approved_by_node` (quem
--   assinou), `approval_signature` (a prova), `approval_epoch` (geração da
--   identidade do aprovador, anti-rollback) e `approval_expires_at` (até quando
--   a prova vale). CHECK coerente: `approved` exige as quatro presentes;
--   qualquer outro estado exige as quatro **nulas** — rejeição não fabrica
--   prova de aprovação, e aprovação sem prova não entra no banco.
-- * **Sem FK para `federation_peer(node_id)`, deliberadamente.** A emenda §5
--   autoriza nós genesis *por configuração*, fora de banda; eles são aprovadores
--   legítimos sem linha em `federation_peer`, e uma FK tornaria o bootstrap
--   impossível ou obrigaria o servidor a escrever pares a partir de config (uma
--   segunda fonte de verdade). A verificação de que o aprovador está
--   `authorized` acontece **na transação de decisão**
--   (`godmesh_server::federation::approve_join_request`), contra
--   `federation_peer` **ou** o genesis declarado, e é coberta por teste de
--   integração no fio. Um FK também não expressaria `trust_state='authorized'`:
--   a constraint necessária não existe em SQL puro sem trigger.
-- * `approved_by` (principal do grant humano) permanece: são duas autoridades
--   compostas, não uma substituindo a outra.
-- * Formato de `approved_by_node` e `approval_signature` é CHECK, não aplicação:
--   identificador ou assinatura malformados são rejeitados pelo banco, e não
--   pelo código que pode ter um caminho esquecido.
--
-- Migração de dados: **nenhuma linha aprovada antes desta fatia tem prova
-- assinada**, e o CHECK não admite aprovação sem prova. O bloco abaixo falha
-- alto com instrução acionável em vez de apagar ou reescrever decisão alheia
-- silenciosamente: aprovação anterior à emenda é re-considerada pelo operador,
-- não convertida por script.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM federation_join_request WHERE state = 'approved') THEN
        RAISE EXCEPTION
            'há pedidos de ingresso aprovados sem prova assinada de nó: a migração 0009 exige '
            'approved_by_node, approval_signature, approval_epoch e approval_expires_at. '
            'Nenhuma aprovação anterior à emenda da ADR-071 é verificável por outro nó. '
            'Ação do operador: registrar nova decisão aprovada por nó autorizado para cada '
            'pedido afetado. Nenhuma linha é apagada nem reescrita por esta migração.';
    END IF;
END $$;

ALTER TABLE federation_join_request
    ADD COLUMN approved_by_node     text,
    ADD COLUMN approval_signature   text,
    ADD COLUMN approval_epoch       bigint,
    ADD COLUMN approval_expires_at  timestamptz;

ALTER TABLE federation_join_request
    -- Aprovação é um fato composto: com `state='approved'` as quatro provas
    -- existem juntas...
    ADD CONSTRAINT federation_join_request_approval_complete_when_approved CHECK (
        state <> 'approved' OR (
            approved_by_node IS NOT NULL
            AND approval_signature IS NOT NULL
            AND approval_epoch IS NOT NULL
            AND approval_expires_at IS NOT NULL
        )
    ),
    -- ...e em qualquer outro estado nenhuma delas existe. As duas direções são
    -- CHECKs separados de propósito: uma única equivalência `(a) = (b)` deixaria
    -- `rejected` com `approved_by_node` preenchido passar (falso nos dois lados).
    ADD CONSTRAINT federation_join_request_approval_absent_when_not_approved CHECK (
        state = 'approved' OR (
            approved_by_node IS NULL
            AND approval_signature IS NULL
            AND approval_epoch IS NULL
            AND approval_expires_at IS NULL
        )
    ),
    -- Identificador do aprovador é `node_id` no formato canônico: nada "quase
    -- igual" pode virar autoridade de ingresso.
    ADD CONSTRAINT federation_join_request_approved_by_node_format CHECK (
        approved_by_node IS NULL OR approved_by_node ~ '^gmn_[0-9a-f]{64}$'
    ),
    -- Assinatura Ed25519 em hex minúsculo (64 bytes).
    ADD CONSTRAINT federation_join_request_approval_signature_format CHECK (
        approval_signature IS NULL OR approval_signature ~ '^[0-9a-f]{128}$'
    ),
    ADD CONSTRAINT federation_join_request_approval_epoch_nonnegative CHECK (
        approval_epoch IS NULL OR approval_epoch >= 0
    ),
    -- A prova precisa ter validade depois do instante em que foi registrada:
    -- gravar aprovação com certificado já vencido é contradição de persisted.
    ADD CONSTRAINT federation_join_request_approval_not_expired_on_write CHECK (
        approval_expires_at IS NULL OR approved_at IS NULL
        OR approval_expires_at > approved_at
    );

-- Anti-rollback: a decisão "epoch não regressivo" consulta o histórico por
-- aprovador. Sem índice, o custo cresce com o número de pedidos; com ele, é
-- uma leitura proporcional às decisões daquele nó.
CREATE INDEX federation_join_request_approved_by_node_idx
    ON federation_join_request (approved_by_node, approval_epoch DESC)
    WHERE approved_by_node IS NOT NULL;

COMMENT ON COLUMN federation_join_request.approved_by_node IS
    'node_id do nó que assinou a aprovação (ADR-071 emenda §2). Autorizado por aprovação anterior ou por genesis declarado; a verificação é transacional.';

COMMENT ON COLUMN federation_join_request.approval_signature IS
    'Assinatura Ed25519 (hex minúsculo) sobre os bytes canônicos do ApprovalCertificate. Verificável por qualquer nó que conheça a chave do aprovador.';

COMMENT ON COLUMN federation_join_request.approval_epoch IS
    'Geração da identidade de federação do aprovador no momento da decisão. Usada como piso anti-rollback: epoch menor que o histórico do nó é recusada.';

COMMENT ON COLUMN federation_join_request.approval_expires_at IS
    'Fim da validade do certificado de aprovação. Reproduzido do certificado para que a expiração seja consultável sem re-verificar assinatura.';

COMMENT ON CONSTRAINT federation_join_request_approval_absent_when_not_approved ON federation_join_request IS
    'Em pending/rejected/expired nenhuma das quatro colunas de prova existe; em approved todas existem (ver o CHECK completo_when_approved).';
