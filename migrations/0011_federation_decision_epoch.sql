-- SPEC M1B/I v1.3: piso de epoch de decisões confirmadas por aprovador.
-- Um genesis pode decidir sem linha em federation_peer.
CREATE TABLE federation_decision_epoch (
    approver_node_id text PRIMARY KEY CHECK (approver_node_id ~ '^gmn_[0-9a-f]{64}$'),
    decision_epoch bigint NOT NULL CHECK (decision_epoch >= 0)
);

-- As rejeições anteriores a esta migração não registravam epoch.
INSERT INTO federation_decision_epoch (approver_node_id, decision_epoch)
SELECT approved_by_node, max(approval_epoch)
  FROM federation_join_request
 WHERE state = 'approved' AND approved_by_node IS NOT NULL
 GROUP BY approved_by_node;
