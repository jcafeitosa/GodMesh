-- M4-A2. Suspensão de retomada independente do ciclo de vida do checkpoint.
CREATE TABLE federation_replica_resume_block (
    local_node text NOT NULL,
    origin_node text NOT NULL,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    partition integer NOT NULL,
    authority_epoch bigint NOT NULL,
    blocked_next_seq bigint NOT NULL CHECK (blocked_next_seq > 0),
    PRIMARY KEY (local_node, origin_node, tenant_id, stream_id, partition, authority_epoch),
    FOREIGN KEY (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
        REFERENCES federation_replica_namespace
            (origin_node, tenant_id, stream_id, partition, authority_epoch, local_node)
);
