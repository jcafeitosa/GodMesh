-- M1C-B: dependências de pins e estado portátil, separados da prova e do piso v1.
CREATE TABLE federation_portable_roots (
    cluster text NOT NULL CHECK (btrim(cluster) <> ''),
    root_node_id text NOT NULL CHECK (root_node_id ~ '^gmn_[0-9a-f]{64}$'),
    public_key text NOT NULL CHECK (public_key ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY (cluster, root_node_id)
);

CREATE TABLE federation_portable_decisions (
    join_request_id text PRIMARY KEY REFERENCES federation_join_request,
    cluster text NOT NULL,
    root_node_id text NOT NULL,
    signer_node_id text NOT NULL,
    leaf_digest text NOT NULL CHECK (leaf_digest ~ '^[0-9a-f]{64}$'),
    digests text[] NOT NULL CHECK (cardinality(digests) > 0 AND leaf_digest = digests[cardinality(digests)]),
    chain_json text NOT NULL CHECK (chain_json <> ''),
    decided_at bigint NOT NULL,
    approved_by text NOT NULL CHECK (btrim(approved_by) <> ''),
    UNIQUE (cluster, root_node_id, leaf_digest),
    FOREIGN KEY (cluster, root_node_id) REFERENCES federation_portable_roots
);

CREATE TABLE federation_portable_epoch (
    cluster text NOT NULL,
    root_node_id text NOT NULL,
    signer_node_id text NOT NULL,
    epoch bigint NOT NULL CHECK (epoch >= 0),
    PRIMARY KEY (cluster, root_node_id, signer_node_id),
    FOREIGN KEY (cluster, root_node_id) REFERENCES federation_portable_roots
);

CREATE TABLE federation_portable_validity (
    cluster text NOT NULL,
    root_node_id text NOT NULL,
    leaf_digest text NOT NULL,
    issued_at bigint NOT NULL,
    expires_at bigint NOT NULL CHECK (expires_at > issued_at),
    validity jsonb NOT NULL CHECK (jsonb_typeof(validity) = 'object'),
    PRIMARY KEY (cluster, root_node_id, leaf_digest, issued_at),
    FOREIGN KEY (cluster, root_node_id, leaf_digest) REFERENCES federation_portable_decisions (cluster, root_node_id, leaf_digest)
);

CREATE TABLE federation_portable_imports (
    cluster text NOT NULL,
    node_id text NOT NULL CHECK (node_id ~ '^gmn_[0-9a-f]{64}$'),
    public_key text NOT NULL CHECK (public_key ~ '^[0-9a-f]{64}$'),
    root_node_id text NOT NULL,
    origin text NOT NULL,
    peer_node_id text UNIQUE REFERENCES federation_peer,
    manifest jsonb NOT NULL CHECK (jsonb_typeof(manifest) = 'object'),
    chain_json text,
    digests text[],
    validity jsonb,
    issued_at bigint NOT NULL,
    expires_at bigint,
    PRIMARY KEY (cluster, node_id),
    FOREIGN KEY (cluster, root_node_id) REFERENCES federation_portable_roots,
    CHECK (peer_node_id IS NULL OR peer_node_id = node_id),
    CHECK (
        (origin = 'pinned_genesis' AND root_node_id = node_id AND peer_node_id IS NOT NULL
            AND chain_json IS NULL AND digests IS NULL AND validity IS NULL AND expires_at IS NULL)
        OR
        (origin = 'portable_chain' AND chain_json IS NOT NULL AND digests IS NOT NULL
            AND cardinality(digests) > 0 AND validity IS NOT NULL AND expires_at IS NOT NULL
            AND expires_at > issued_at)
    )
);

CREATE TABLE federation_portable_revocations (
    cluster text NOT NULL,
    root_node_id text NOT NULL,
    target_digest text NOT NULL CHECK (target_digest ~ '^[0-9a-f]{64}$'),
    revocation_digest text NOT NULL CHECK (revocation_digest ~ '^[0-9a-f]{64}$'),
    target_chain_json text NOT NULL,
    revocation jsonb NOT NULL CHECK (jsonb_typeof(revocation) = 'object'),
    receipt jsonb NOT NULL CHECK (jsonb_typeof(receipt) = 'object'),
    PRIMARY KEY (cluster, root_node_id, target_digest),
    FOREIGN KEY (cluster, root_node_id) REFERENCES federation_portable_roots
);
