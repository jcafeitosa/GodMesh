-- Migração 0002 — tenancy do incidente SOS e política de retenção aplicada.
-- ADR-062 (nada persistido sem política), ADR-065 (prazos por classe) e
-- ADR-066 (residência declarada por tenant; BR primária, CO secundária).
--
-- Motivo do delta, achados do gate wfr_414e32e9:
--   * `sos_incident` não registrava qual política autorizou a persistência e
--     aceitava INSERT direto sem classe de retenção;
--   * PK global de `id` fazia um tenant bloquear o `id` de outro (squatting);
--   * não existia fonte de jurisdição por tenant para resolver a política.

BEGIN;

-- Jurisdição declarada por tenant (ADR-066). A jurisdição é dado do servidor,
-- nunca do corpo do pedido: campo enviado pelo cliente é seleção sujeita a
-- autorização, não prova de pertencimento (SPEC-005 §3). Sem linha aqui, a API
-- falha fechada (`retention_scope_unresolved`).
CREATE TABLE tenant_region (
    tenant_id    text        NOT NULL,
    jurisdiction text        NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    declared_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id)
);

COMMENT ON TABLE tenant_region IS
    'ADR-066: jurisdição declarada por tenant. Ausência de linha bloqueia persistência (ADR-062).';

-- Colômbia é região secundária da ADR-066 e a classe `incidente_sos` está
-- declarada para BR/CO na ADR-065 com a mesma finalidade, prazo e gatilho.
-- As demais classes continuam sem política em CO e seguem falhando fechado.
INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) VALUES
    ('incidente_sos', 'CO', 'dever_de_cuidado', 12, 'incident_closed + 12m')
ON CONFLICT DO NOTHING;

-- Cópia primária autorizada: sem ela a política não autoriza persistência
-- (ADR-062 exige classe + finalidade + cópia + jurisdição + prazo + gatilho).
INSERT INTO retention_copy (class, jurisdiction, copy_scope) VALUES
    ('incidente_sos', 'CO', 'primary')
ON CONFLICT DO NOTHING;

-- Retenção aplicada ao incidente. `retention_policy_id` é derivado da chave
-- composta porque a política neste esquema é identificada por
-- (class, jurisdiction) — uma coluna única não pode referenciar chave de duas
-- colunas; a integridade é garantida pela FK composta abaixo.
ALTER TABLE sos_incident ADD COLUMN retention_class text;
ALTER TABLE sos_incident ADD COLUMN retention_jurisdiction text;
ALTER TABLE sos_incident ADD COLUMN retention_policy_id text
    GENERATED ALWAYS AS (retention_class || '@' || retention_jurisdiction) STORED;

-- Backfill dos registros anteriores à política vinculada: a única classe que o
-- endpoint grava é `incidente_sos`, e a migração 0001 só semeou BR.
UPDATE sos_incident
   SET retention_class = 'incidente_sos',
       retention_jurisdiction = 'BR'
 WHERE retention_class IS NULL
    OR retention_jurisdiction IS NULL;

ALTER TABLE sos_incident
    ALTER COLUMN retention_class SET NOT NULL,
    ALTER COLUMN retention_jurisdiction SET NOT NULL;

ALTER TABLE sos_incident
    ADD CONSTRAINT sos_incident_retention_jurisdiction_check
    CHECK (retention_jurisdiction IN ('BR','CO'));

-- FK imediata (não adiável): no INSERT o PostgreSQL toma KEY SHARE na linha da
-- política, então apagar a política concorrentemente não deixa incidente órfão.
ALTER TABLE sos_incident
    ADD CONSTRAINT sos_incident_retention_class_fkey
    FOREIGN KEY (retention_class, retention_jurisdiction)
    REFERENCES retention_class (class, jurisdiction);

-- Tenancy explícita no agregado: a chave do incidente passa a ser
-- (tenant_id, id). `id` continua único no banco inteiro, e isso é proposital:
--  * a trilha `sos_ack(incident_id)` referencia `sos_incident(id)` e não ganha
--    `tenant_id` nesta migração, então precisa de chave única em `id`;
--  * colisão de `id` entre tenants deixa de virar 503 opaco e passa a ser
--    detectada e devolvida como 409 `incident_id_conflict`, sem criar um segundo
--    incidente com o mesmo identificador.
ALTER TABLE sos_ack DROP CONSTRAINT sos_ack_incident_id_fkey;
ALTER TABLE sos_incident DROP CONSTRAINT sos_incident_pkey;
ALTER TABLE sos_incident ADD CONSTRAINT sos_incident_pkey PRIMARY KEY (tenant_id, id);
ALTER TABLE sos_incident ADD CONSTRAINT sos_incident_id_key UNIQUE (id);
ALTER TABLE sos_ack ADD CONSTRAINT sos_ack_incident_id_fkey
    FOREIGN KEY (incident_id) REFERENCES sos_incident (id);

COMMIT;