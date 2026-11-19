-- M3-B1 profile 1: uma série federada e um papel local por banco.
CREATE TABLE federation_config_policy (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    profile bigint NOT NULL CHECK (profile = 1),
    cluster text NOT NULL CHECK (btrim(cluster) <> ''),
    namespace text NOT NULL CHECK (namespace = 'federation'),
    scope_id text NOT NULL CHECK (scope_id = ''),
    epoch bigint NOT NULL CHECK (epoch >= 0),
    role text NOT NULL CHECK (role IN ('publisher', 'receiver')),
    voters text[] NOT NULL CHECK (cardinality(voters) IN (3, 5) AND array_position(voters, NULL) IS NULL)
);

-- Cada hash tem uma representação original. Um bundle incompatível pode
-- receber aceitação posterior, sem reescrever o frame ou apagar a evidência.
CREATE TABLE federation_config_frame (
    hash text PRIMARY KEY CHECK (hash ~ '^[0-9a-f]{64}$'),
    series boolean NOT NULL DEFAULT true REFERENCES federation_config_policy,
    version bigint NOT NULL CHECK (version > 0),
    frame bytea NOT NULL CHECK (octet_length(frame) > 0),
    applies_at bigint NOT NULL,
    expires_at bigint NOT NULL CHECK (expires_at > applies_at),
    effective_after bigint NOT NULL CHECK (effective_after < expires_at),
    floor bigint NOT NULL CHECK (floor > 0),
    software bigint[] NOT NULL CHECK (
        cardinality(software) = 3 AND array_position(software, NULL) IS NULL AND 0 <= ALL(software)
    ),
    signers text[] NOT NULL CHECK (cardinality(signers) IN (3, 5) AND array_position(signers, NULL) IS NULL),
    accepted boolean NOT NULL,
    incompatible boolean NOT NULL,
    resolution boolean NOT NULL CHECK (NOT resolution OR accepted)
);

CREATE INDEX federation_config_frame_version ON federation_config_frame (version);
CREATE UNIQUE INDEX federation_config_frame_one_primary_per_version
    ON federation_config_frame (version) WHERE accepted OR incompatible;

CREATE TABLE federation_config_state (
    singleton boolean PRIMARY KEY DEFAULT true REFERENCES federation_config_policy,
    accepted text REFERENCES federation_config_frame,
    incompatible text REFERENCES federation_config_frame,
    pending text REFERENCES federation_config_frame,
    effective text REFERENCES federation_config_frame,
    published text REFERENCES federation_config_frame,
    conflict text REFERENCES federation_config_frame,
    recovering boolean NOT NULL DEFAULT false,
    CHECK (pending IS NULL OR pending IS NOT DISTINCT FROM accepted),
    CHECK (recovering IS FALSE OR pending IS NOT NULL),
    CHECK (accepted IS NOT DISTINCT FROM COALESCE(pending, effective))
);

CREATE TABLE federation_config_audit (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    hash text NOT NULL REFERENCES federation_config_frame,
    happened_at bigint NOT NULL,
    transition text NOT NULL CHECK (transition IN ('accepted', 'incompatible', 'conflict', 'resolved', 'activated')),
    actor text,
    CHECK ((transition = 'resolved') = (actor IS NOT NULL)),
    CHECK (actor IS NULL OR btrim(actor) <> '')
);
