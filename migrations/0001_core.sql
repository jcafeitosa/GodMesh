-- Migração inicial do núcleo — ADR-057 (posição), ADR-062/065 (retenção),
-- ADR-058/064 (ACK do incidente SOS).
--
-- Princípio: o banco também é fronteira. Nada de evento sem classe de retenção;
-- nada de ACK reescrito depois de gravado.

-- PostGIS: adiado até haver imagem multi-arch homologada (arm64).
-- CREATE EXTENSION IF NOT EXISTS postgis;

-- Classe de retenção: finalidade, jurisdição, prazo e gatilho (ADR-065).
CREATE TABLE retention_class (
    class           text        NOT NULL,
    jurisdiction    text        NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    finality        text        NOT NULL,
    months          integer     NOT NULL CHECK (months > 0),
    discard_trigger text        NOT NULL,
    PRIMARY KEY (class, jurisdiction)
);

-- Cópia autorizada por classe/jurisdição (ADR-066).
CREATE TABLE retention_copy (
    class        text NOT NULL,
    jurisdiction text NOT NULL,
    copy_scope   text NOT NULL CHECK (copy_scope IN ('primary','secondary','backup')),
    PRIMARY KEY (class, jurisdiction, copy_scope),
    FOREIGN KEY (class, jurisdiction) REFERENCES retention_class (class, jurisdiction)
);

-- Posição canônica de evento (ADR-057).
CREATE TABLE event_position (
    origin_node     text        NOT NULL,
    tenant_id       text        NOT NULL,
    stream_id       text        NOT NULL,
    partition       integer     NOT NULL CHECK (partition >= 0),
    authority_epoch bigint      NOT NULL CHECK (authority_epoch > 0),
    seq             bigint      NOT NULL CHECK (seq > 0),
    retention_class text        NOT NULL,
    jurisdiction    text        NOT NULL CHECK (jurisdiction IN ('BR','CO')),
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq),
    FOREIGN KEY (retention_class, jurisdiction) REFERENCES retention_class (class, jurisdiction)
);

-- Incidente SOS: estado é a única fonte do rótulo.
CREATE TABLE sos_incident (
    id          text        PRIMARY KEY,
    tenant_id   text        NOT NULL,
    principal   text        NOT NULL,
    state       text        NOT NULL CHECK (state IN (
                    'triggered_locally','persisted_local','replicated_to_authority',
                    'acknowledged_technical','acknowledged_human','outcome_unknown',
                    'not_persisted','closed')),
    reason      text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

-- Trilha de ACK: append-only.
CREATE TABLE sos_ack (
    incident_id text        NOT NULL REFERENCES sos_incident (id),
    ack_class   text        NOT NULL CHECK (ack_class IN (
                    'transport_accepted','persisted_local','replicated_to_authority',
                    'materialized','delivered_to_device','acknowledged_human')),
    source      text        NOT NULL,
    at_unix     bigint      NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (incident_id, ack_class, recorded_at)
);

CREATE OR REPLACE FUNCTION forbid_ack_rewrite() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'sos_ack é append-only (ADR-058)';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sos_ack_append_only
    BEFORE UPDATE OR DELETE ON sos_ack
    FOR EACH ROW EXECUTE FUNCTION forbid_ack_rewrite();

-- Outbox durável (ADR-061): publicar não é confirmar.
CREATE TABLE outbox_entry (
    origin_node     text        NOT NULL,
    tenant_id       text        NOT NULL,
    stream_id       text        NOT NULL,
    partition       integer     NOT NULL,
    authority_epoch bigint      NOT NULL,
    seq             bigint      NOT NULL,
    effect_kind     text        NOT NULL CHECK (effect_kind IN ('event','irreversible')),
    published_at    timestamptz,
    confirmed_at    timestamptz,
    PRIMARY KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, seq)
);

-- Semente mínima de retenção conforme ADR-065 (revisão jurídica pendente).
INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) VALUES
    ('mensagem',        'BR', 'historico',            24, 'expiry'),
    ('transcricao_asr', 'BR', 'funcao_do_canal',      12, 'expiry'),
    ('posicao',         'BR', 'mapa_e_suporte',        1, 'expiry'),
    ('incidente_sos',   'BR', 'dever_de_cuidado',     12, 'incident_closed + 12m'),
    ('auditoria',       'BR', 'deteccao_de_abuso',    12, 'expiry'),
    ('telemetria',      'BR', 'operacao',              1, 'rollup_complete'),
    ('crowdfind',       'BR', 'resolucao_de_avistamento', 1, 'token_rotate')
ON CONFLICT DO NOTHING;

INSERT INTO retention_copy (class, jurisdiction, copy_scope) VALUES
    ('mensagem','BR','primary'), ('posicao','BR','primary'),
    ('incidente_sos','BR','primary'), ('auditoria','BR','primary'),
    ('telemetria','BR','primary'), ('telemetria','BR','secondary'),
    ('crowdfind','BR','primary'), ('transcricao_asr','BR','primary')
ON CONFLICT DO NOTHING;
