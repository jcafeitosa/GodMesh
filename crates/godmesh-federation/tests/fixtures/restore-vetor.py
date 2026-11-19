"""Vetor independente M4-B2-C v1.4. Python 3 e cryptography 50.0.2.

Executar: python3 restore-vetor.py > restore-vetor.json
"""

import hashlib
import json
import struct
from pathlib import Path

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


DOMAIN = b"godmesh.federation.restore."
A = "gmn_" + "a" * 64
B = "gmn_" + "b" * 64
KEY = {
    "fixture_id": "11" * 16,
    "cluster": "c",
    "local_node": B,
    "origin_node": A,
    "tenant_id": "t",
    "stream_id": "m4a.metadata.v",
    "partition": 0,
    "authority_epoch": 7,
}
KEY_ORDER = "fixture_id:T cluster:T local_node:T origin_node:T tenant_id:T stream_id:T partition:P authority_epoch:U"


def u32(value):
    return struct.pack(">I", value)


def u64(value):
    return struct.pack(">Q", value)


def tlv(tag, value):
    tag = tag.encode("ascii")
    return u32(len(tag)) + tag + u32(len(value)) + value


def body(suffix, order, fields):
    raw = DOMAIN + suffix.encode("ascii")
    for field in order.split():
        tag, kind = field.split(":")
        value = fields[tag]
        raw += tlv(tag, encode(kind, value))
    return raw


def encode(kind, value):
    if kind == "T":
        return value.encode("utf-8")
    if kind == "U":
        return u64(value)
    if kind == "I":
        return struct.pack(">q", value)
    if kind == "P":
        return u32(value)
    if kind in ("B", "E"):
        return bytes([value])
    if kind in ("D", "S"):
        return bytes.fromhex(value)
    if kind == "set":
        digests = sorted(bytes.fromhex(digest) for digest in value)
        return u64(len(digests)) + b"".join(digests)
    raise ValueError(kind)


def record(out, name, suffix, order, fields):
    raw = body(suffix, order, fields)
    digest = hashlib.sha256(raw).hexdigest()
    out[name] = {"canonical_hex": raw.hex(), "sha256": digest}
    return raw, digest


def main():
    out = {}
    policy_fields = {
        "policy_id": "p",
        "policy_version": 1,
        "tenant_id": "t",
        "retention_class": "meta",
        "class_version": 1,
        "jurisdiction": "BR",
        "copy_node": A,
        "copy_kind": "primary",
        "purpose": "replica_metadata",
        "trigger_event": "issued_at",
        "duration_seconds": 100,
        "allow_discard": True,
        "hold_mode": "explicit",
        "active": True,
    }
    policy_order = (
        "policy_id:T policy_version:U tenant_id:T retention_class:T class_version:U "
        "jurisdiction:T copy_node:T copy_kind:T purpose:T trigger_event:T duration_seconds:U "
        "allow_discard:B hold_mode:T active:B"
    )
    _, primary = record(out, "primary_policy", "copy-policy.v1", policy_order, policy_fields)
    secondary = dict(policy_fields, policy_id="s", policy_version=2, copy_node=B, copy_kind="secondary")
    _, export_policy = record(out, "export_policy", "copy-policy.v1", policy_order, secondary)
    _, import_policy = record(out, "import_policy", "copy-policy.v1", policy_order, dict(secondary, policy_version=3))

    acl_fields = {
        "origin_node": A, "tenant_id": "t", "stream_id": KEY["stream_id"], "partition": 0,
        "authority_epoch": 7, "destination": B, "direction": "export", "retention_class": "meta",
        "class_version": 1, "jurisdiction": "BR", "copy_kind": "secondary", "policy_id": "s",
        "policy_version": 2, "not_before": 0, "expires_at": 200, "revoked": False,
    }
    acl_order = (
        "origin_node:T tenant_id:T stream_id:T partition:P authority_epoch:U destination:T direction:T "
        "retention_class:T class_version:U jurisdiction:T copy_kind:T policy_id:T policy_version:U "
        "not_before:I expires_at:I revoked:B"
    )
    _, export_acl = record(out, "export_acl", "copy-acl.v1", acl_order, acl_fields)
    _, import_acl = record(out, "import_acl", "copy-acl.v1", acl_order, dict(acl_fields, direction="import", policy_version=3))
    _, revocation = record(
        out, "revocation", "revocation.v1",
        "cluster:T root_node_id:T target_digest:D revocation_digest:D accepted_at:I receipt_signature:S",
        {"cluster": "c", "root_node_id": A, "target_digest": "22" * 32, "revocation_digest": "33" * 32,
         "accepted_at": 50, "receipt_signature": "44" * 64},
    )
    _, control = record(
        out, "control", "controls.v1",
        KEY_ORDER + " generation:U root_node_id:T authority_kind:E authority_ref:D origin_revocations:set "
        "destination_revocations:set m3_epoch:U m3_version:U m3_hash:D m3_floor:U primary_policy_sha256:D "
        "export_secondary_policy_sha256:D import_secondary_policy_sha256:D export_acl_sha256:D "
        "import_acl_sha256:D blocked_next_seq:U intent_state:E attempt_id:U prefix:U",
        dict(KEY, generation=2, root_node_id=A, authority_kind=0, authority_ref="55" * 32,
             origin_revocations=[revocation], destination_revocations=[], m3_epoch=1, m3_version=2,
             m3_hash="66" * 32, m3_floor=1, primary_policy_sha256=primary,
             export_secondary_policy_sha256=export_policy, import_secondary_policy_sha256=import_policy,
             export_acl_sha256=export_acl, import_acl_sha256=import_acl,
             blocked_next_seq=0, intent_state=0, attempt_id=0, prefix=0),
    )
    _, fence = record(
        out, "fence", "fence.v1",
        KEY_ORDER + " fenced_generation:U cut_db_id:T target_db_id:T old_login:T admission_closed:B "
        "nologin_confirmed:B grants_removed:B remaining_sessions:U unknown_effects:U fenced_at:I",
        dict(KEY, fenced_generation=1, cut_db_id="cut", target_db_id="target", old_login="old",
             admission_closed=True, nologin_confirmed=True, grants_removed=True, remaining_sessions=0,
             unknown_effects=0, fenced_at=100),
    )

    snapshot = json.loads(Path(__file__).with_name("m4-b1-vetor.json").read_text())
    proof_frames = [bytes.fromhex(frame) for frame in snapshot["events"]]
    for label, frames in (("empty_proofs", []), ("two_proofs", proof_frames)):
        raw = DOMAIN + b"proofs.v1" + u64(len(frames))
        raw += b"".join(u32(len(frame)) + frame for frame in frames)
        out[label] = {"frames": [f.hex() for f in frames], "canonical_hex": raw.hex(),
                      "sha256": hashlib.sha256(raw).hexdigest()}

    reconciliation = dict(KEY, target_generation=2, high_water_sha256="77" * 32, final_prefix=0,
                          proofs_sha256=out["empty_proofs"]["sha256"], current_control_sha256=control,
                          blocked_next_seq=0, intent_state=0, attempt_id=0)
    reconciliation_order = (
        KEY_ORDER + " target_generation:U high_water_sha256:D final_prefix:U proofs_sha256:D "
        "current_control_sha256:D blocked_next_seq:U intent_state:E attempt_id:U"
    )
    _, reconciled = record(out, "reconciliation", "reconciled.v1", reconciliation_order, reconciliation)
    assert control == "76cfb322a57d85d0997cb367e1670a8c2fce520c886ba27a7000627c4b7462ad"
    assert fence == "dd435217351771e0e64c792846739ac7af03b91796965b233667c68069fe84f1"
    assert reconciled == "5edea42cb064886242c8e2f98e9a8787e59cc7c6353419dc906247f5e76fc7b8"

    signer = Ed25519PrivateKey.from_private_bytes(bytes([0x19]) * 32)
    out["harness_public_key"] = signer.public_key().public_bytes_raw().hex()
    backup = b"synthetic dump bytes\r\n"
    out["backup_hex"] = backup.hex()
    high = {
        "version": 1, "key": KEY, "backup_db_id": "cut", "target_db_id": "target",
        "backup_generation": 1, "fenced_generation": 1, "target_generation": 2, "backup_kind": "at_cut",
        "backup_bytes": len(backup), "backup_sha256": hashlib.sha256(backup).hexdigest(),
        "backup_prefix": 0, "cut_prefix": 0, "backup_proof_count": 0,
        "backup_proofs_sha256": out["empty_proofs"]["sha256"], "gap_proof_count": 0,
        "gap_proofs_sha256": out["empty_proofs"]["sha256"], "control_state_sha256": control,
        "blocked_next_seq": 0, "intent_state": 0, "attempt_id": 0, "fence_sha256": fence, "cut_at": 100,
    }
    raw, high_digest = record(
        out, "high_water", "high-water.v1",
        "version:U " + KEY_ORDER + " backup_db_id:T target_db_id:T backup_generation:U "
        "fenced_generation:U target_generation:U backup_kind:T backup_bytes:U backup_sha256:D "
        "backup_prefix:U cut_prefix:U backup_proof_count:U backup_proofs_sha256:D gap_proof_count:U "
        "gap_proofs_sha256:D control_state_sha256:D blocked_next_seq:U intent_state:E attempt_id:U "
        "fence_sha256:D cut_at:I",
        dict(high, **KEY),
    )
    out["high_water"]["wire"] = dict(high, signature=signer.sign(raw).hex())

    _, reconciled = record(out, "bound_reconciliation", "reconciled.v1", reconciliation_order,
                           dict(reconciliation, high_water_sha256=high_digest))
    promotion = {
        "version": 1, "key": KEY, "target_db_id": "target", "fenced_generation": 1,
        "target_generation": 2, "high_water_sha256": high_digest, "reconciliation_sha256": reconciled,
        "fence_sha256": fence, "not_before": 110, "expires_at": 120,
    }
    raw, _ = record(
        out, "promotion", "promotion.v1", "version:U " + KEY_ORDER + " target_db_id:T fenced_generation:U "
        "target_generation:U high_water_sha256:D reconciliation_sha256:D fence_sha256:D not_before:I expires_at:I",
        dict(promotion, **KEY),
    )
    out["promotion"]["wire"] = dict(promotion, signature=signer.sign(raw).hex())
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
