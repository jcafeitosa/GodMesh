use std::{fs, num::NonZeroUsize, os::unix::fs::OpenOptionsExt};

use godmesh_federation::{
    GenesisNode, NodeIdentity,
    join_authority::{
        JoinAuthorityError::*, JoinRevocationReceiptV1, JoinRevocationV1, JoinValidityV1,
    },
    join_chain::{
        ExpectedJoinIdentity, JoinChainPolicy, JoinChainV2, VerifiedJoinChain, verify_join_chain,
    },
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Reference<T> {
    artifact: T,
    canonical_hex: String,
}

#[derive(Deserialize)]
struct Vector {
    validity: Reference<JoinValidityV1>,
    revocation: Reference<JoinRevocationV1>,
    receipt: Reference<JoinRevocationReceiptV1>,
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/m1c-b1-vetor.json")).unwrap()
}

fn vector() -> Vector {
    serde_json::from_value(fixture()).unwrap()
}

fn chain(depth: usize, window: Option<(i64, i64)>) -> VerifiedJoinChain {
    let source: Value = serde_json::from_str(include_str!("fixtures/m1c-a-vetor.json")).unwrap();
    let mut chain: JoinChainV2 = serde_json::from_value(source["chain"].clone()).unwrap();
    chain.links.truncate(depth);
    if let Some((start, end)) = window {
        let cert = &mut chain.links[0].certificate;
        cert.not_before = start;
        cert.expires_at = end;
        cert.sign(&signer(1));
    }
    let leaf = &chain.links.last().unwrap().certificate;
    let root = GenesisNode::new(key()["node_id"].as_str().unwrap(), &root_key()).unwrap();
    verify_join_chain(
        &serde_json::to_vec(&chain).unwrap(),
        &ExpectedJoinIdentity {
            node_id: leaf.candidate_node_id.clone(),
            public_key: leaf.candidate_public_key.clone(),
        },
        150,
        &JoinChainPolicy {
            cluster: leaf.cluster.clone(),
            roots: vec![root],
            max_depth: NonZeroUsize::new(2).unwrap(),
            max_frame_bytes: NonZeroUsize::new(32768).unwrap(),
        },
    )
    .unwrap()
}

fn key() -> Value {
    fixture()["root"].clone()
}

fn root_key() -> String {
    key()["public_key"].as_str().unwrap().into()
}

// Semente pública exclusiva do vetor de teste.
fn signer(seed: u8) -> NodeIdentity {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!(
        "godmesh-m1c-{}-{}",
        std::process::id(),
        hex::encode(
            NodeIdentity::generate_in_memory()
                .unwrap()
                .public_key_bytes()
        )
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    writeln!(file, "{}", hex::encode([seed; 32])).unwrap();
    drop(file);
    let identity = NodeIdentity::load(&path);
    fs::remove_file(path).unwrap();
    identity.unwrap()
}

#[test]
fn vetor_python_confere_bytes_assinaturas_digest_e_vinculos() {
    let Vector {
        validity,
        revocation,
        receipt,
    } = vector();
    let root = signer(1);
    let mut status = validity.artifact.clone();
    status.root_node_id.clear();
    status.signature.clear();
    status.sign(&root);
    assert_eq!(status, validity.artifact);
    assert_eq!(
        hex::encode(status.canonical_bytes()),
        validity.canonical_hex
    );
    status
        .verify_for_chain(&root_key(), &chain(2, None), 150, 10)
        .unwrap();

    let mut record = revocation.artifact.clone();
    record.revoker_node_id.clear();
    record.signature.clear();
    record.sign(&root);
    assert_eq!(record, revocation.artifact);
    assert_eq!(
        hex::encode(record.canonical_bytes()),
        revocation.canonical_hex
    );
    assert_eq!(record.digest().unwrap(), fixture()["revocation"]["digest"]);
    let evidence = record
        .verify_for_chain(&root_key(), &chain(1, None))
        .unwrap();

    let mut ack = receipt.artifact.clone();
    ack.root_node_id.clear();
    ack.signature.clear();
    ack.sign(&root);
    assert_eq!(ack, receipt.artifact);
    assert_eq!(hex::encode(ack.canonical_bytes()), receipt.canonical_hex);
    ack.verify_for_revocation(&root_key(), &evidence).unwrap();
}

fn strict_json<T: DeserializeOwned>(name: &str) {
    let mut data = fixture()[name]["artifact"].clone();
    let raw = data.to_string();
    assert!(serde_json::from_str::<T>(&raw).is_ok());
    let duplicate = format!(r#"{{"cluster":"duplicado",{}"#, &raw[1..]);
    assert!(serde_json::from_str::<T>(&duplicate).is_err());
    data["extra"] = json!(0);
    assert!(serde_json::from_value::<T>(data.clone()).is_err());
    let fields = data.as_object_mut().unwrap();
    fields.remove("extra");
    fields.remove("signature");
    assert!(serde_json::from_value::<T>(data).is_err());

    let order: &[&str] = match name {
        "validity" => &[
            "cluster",
            "root_node_id",
            "leaf_digest",
            "issued_at",
            "expires_at",
            "signature",
        ],
        "revocation" => &[
            "cluster",
            "root_node_id",
            "target_digest",
            "revoker_node_id",
            "epoch",
            "signature",
        ],
        "receipt" => &[
            "cluster",
            "root_node_id",
            "revocation_digest",
            "accepted_at",
            "signature",
        ],
        _ => unreachable!(),
    };
    let object = &fixture()[name]["artifact"];
    let positional = Value::Array(order.iter().map(|field| object[*field].clone()).collect());
    assert!(serde_json::from_str::<T>(&positional.to_string()).is_err());
    assert!(serde_json::from_value::<T>(positional).is_err());
}

#[test]
fn os_tres_formatos_exigem_objeto_estrito() {
    strict_json::<JoinValidityV1>("validity");
    strict_json::<JoinRevocationV1>("revocation");
    strict_json::<JoinRevocationReceiptV1>("receipt");
}

#[test]
fn vigencia_respeita_precedencia_e_intervalo_semiaberto() {
    let v = vector();
    let status = v.validity.artifact;
    let root = signer(1);
    let wrong_key = signer(2).public_key_hex();
    let chain = chain(2, None);
    let verify = |artifact: &JoinValidityV1, key: &str, now, max| {
        artifact.verify_for_chain(key, &chain, now, max)
    };
    let key = root_key();
    assert_eq!(verify(&status, &key, 149, 10), Err(NotYetValid));
    assert_eq!(verify(&status, &key, 150, 10), Ok(()));
    assert_eq!(verify(&status, &key, 159, 10), Ok(()));
    assert_eq!(verify(&status, &key, 160, 10), Err(Expired));
    assert_eq!(verify(&status, &key, 149, 9), Err(DurationExceeded));
    assert_eq!(verify(&status, &key, 150, 0), Err(InvalidField));

    let mut bad = status.clone();
    bad.leaf_digest = "F".repeat(64);
    assert_eq!(verify(&bad, &wrong_key, 150, 10), Err(InvalidField));
    bad.leaf_digest = "0".repeat(64);
    bad.signature = v.receipt.artifact.signature;
    assert_eq!(verify(&bad, &wrong_key, 150, 10), Err(KeyMismatch));
    assert_eq!(verify(&bad, &key, 150, 10), Err(InvalidSignature));
    bad.issued_at = 119;
    bad.sign(&root);
    assert_eq!(verify(&bad, &key, 118, 1), Err(ArtifactMismatch));
    bad.leaf_digest.clone_from(&status.leaf_digest);
    bad.sign(&root);
    assert_eq!(verify(&bad, &key, 118, 1), Err(WindowOutsideChain));
    bad.issued_at = status.issued_at;
    bad.expires_at = 181;
    bad.sign(&root);
    assert_eq!(verify(&bad, &key, 150, 100), Err(WindowOutsideChain));
    bad.issued_at = bad.expires_at + 1;
    bad.sign(&root);
    assert_eq!(verify(&bad, &key, 150, 10), Err(InvalidWindow));
    assert_eq!(verify(&status, &key, 150, 10), Ok(()));
}

#[test]
fn vigencia_calcula_duracao_nos_extremos_sem_overflow() {
    let chain = chain(1, Some((i64::MIN, i64::MAX)));
    let root = signer(1);
    let key = root_key();
    let mut status = vector().validity.artifact;
    status.leaf_digest = chain.leaf_digest().into();
    status.issued_at = i64::MIN;
    status.expires_at = i64::MAX;
    status.sign(&root);
    assert_eq!(
        status.verify_for_chain(&key, &chain, i64::MAX, i64::MAX),
        Err(DurationExceeded)
    );
    for (start, end) in [(i64::MIN, i64::MIN + 1), (i64::MAX - 1, i64::MAX)] {
        status.issued_at = start;
        status.expires_at = end;
        status.sign(&root);
        assert_eq!(status.verify_for_chain(&key, &chain, start, 1), Ok(()));
        assert_eq!(status.verify_for_chain(&key, &chain, end, 1), Err(Expired));
    }
}

#[test]
fn revogacao_verifica_formato_chave_assinatura_e_elo_nessa_ordem() {
    let v = vector();
    let root = signer(1);
    let delegate = signer(2);
    let key = root_key();
    let target = chain(1, None);
    assert_eq!(
        v.revocation
            .artifact
            .verify_for_chain(&key, &chain(2, None)),
        Err(ArtifactMismatch)
    );
    let mut record = v.revocation.artifact.clone();
    record.epoch = i64::MAX as u64 + 1;
    assert_eq!(
        record.verify_for_chain(&delegate.public_key_hex(), &target),
        Err(InvalidField)
    );
    record.epoch -= 1;
    record.target_digest = "0".repeat(64);
    assert_eq!(
        record.verify_for_chain(&delegate.public_key_hex(), &target),
        Err(KeyMismatch)
    );
    assert_eq!(
        record.verify_for_chain(&key, &target),
        Err(InvalidSignature)
    );
    record.sign(&delegate);
    assert_eq!(
        record.verify_for_chain(&delegate.public_key_hex(), &target),
        Err(ArtifactMismatch)
    );
    record.target_digest = target.leaf_digest().into();
    record.sign(&delegate);
    let mut other_cluster = record.clone();
    other_cluster.cluster = " cluster-teste".into();
    other_cluster.sign(&delegate);
    assert_eq!(
        other_cluster.verify_for_chain(&delegate.public_key_hex(), &target),
        Err(ArtifactMismatch)
    );
    let mut other_root = record.clone();
    other_root.root_node_id = delegate.node_id().into();
    other_root.sign(&delegate);
    assert_eq!(
        other_root.verify_for_chain(&delegate.public_key_hex(), &target),
        Err(ArtifactMismatch)
    );
    assert_eq!(record.root_node_id, root.node_id());
    assert_eq!(record.revoker_node_id, delegate.node_id());
    assert!(
        record
            .verify_for_chain(&delegate.public_key_hex(), &target)
            .is_ok()
    );
}

#[test]
fn recibo_autentico_de_revogacao_invalida_nao_produz_evidencia() {
    let v = vector();
    let target = chain(1, None);
    let key = root_key();
    let root = signer(1);
    let record = v.revocation.artifact;
    let ack = v.receipt.artifact;
    let mut invalid_record = record.clone();
    invalid_record.signature.clone_from(&ack.signature);
    let mut signed_ack = ack.clone();
    signed_ack.revocation_digest = invalid_record.digest().unwrap();
    signed_ack.sign(&root);
    assert_eq!(signed_ack.verify(&key), Ok(()));
    assert_eq!(
        invalid_record.verify_for_chain(&key, &target),
        Err(InvalidSignature)
    );

    let evidence = record.verify_for_chain(&key, &target).unwrap();
    assert_eq!(
        signed_ack.verify_for_revocation(&key, &evidence),
        Err(ArtifactMismatch)
    );
    signed_ack = ack.clone();
    signed_ack.cluster = "outro".into();
    assert_eq!(
        signed_ack.verify_for_revocation(&key, &evidence),
        Err(InvalidSignature)
    );
    signed_ack.sign(&root);
    assert_eq!(
        signed_ack.verify_for_revocation(&key, &evidence),
        Err(ArtifactMismatch)
    );
    let delegate = signer(2);
    let mut other_root = ack.clone();
    other_root.sign(&delegate);
    assert_eq!(
        other_root.verify_for_revocation(&key, &evidence),
        Err(KeyMismatch)
    );
    assert_eq!(
        other_root.verify_for_revocation(&delegate.public_key_hex(), &evidence),
        Err(ArtifactMismatch)
    );
    assert_eq!(ack.verify_for_revocation(&key, &evidence), Ok(()));
}
