use std::{fs, num::NonZeroUsize, os::unix::fs::OpenOptionsExt};

use godmesh_federation::{
    ApprovalCertificate, DecisionAction, GenesisNode, NodeIdentity,
    join_chain::{
        ExpectedJoinIdentity,
        JoinChainError::{self, *},
        JoinChainPolicy, JoinChainV2, VerifiedJoinChain, manifest_digest, verify_join_chain,
    },
};
use serde_json::{Value, json};

fn vector() -> Value {
    serde_json::from_str(include_str!("fixtures/m1c-a-vetor.json")).unwrap()
}

fn chain() -> JoinChainV2 {
    serde_json::from_value(vector()["chain"].clone()).unwrap()
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn policy() -> JoinChainPolicy {
    let root = &vector()["root"];
    JoinChainPolicy {
        cluster: "cluster-teste".into(),
        roots: vec![
            GenesisNode::new(
                root["node_id"].as_str().unwrap(),
                root["public_key"].as_str().unwrap(),
            )
            .unwrap(),
        ],
        max_depth: nz(2),
        max_frame_bytes: nz(32768),
    }
}

fn expected(index: usize) -> ExpectedJoinIdentity {
    let cert = &vector()["chain"]["links"][index]["certificate"];
    ExpectedJoinIdentity {
        node_id: cert["candidate_node_id"].as_str().unwrap().into(),
        public_key: cert["candidate_public_key"].as_str().unwrap().into(),
    }
}

fn verify(chain: &JoinChainV2, now: i64) -> Result<VerifiedJoinChain, JoinChainError> {
    verify_join_chain(
        &serde_json::to_vec(chain).unwrap(),
        &expected(1),
        now,
        &policy(),
    )
}

fn baseline() -> VerifiedJoinChain {
    verify(&chain(), 150).unwrap()
}

// As sementes são públicas e exclusivas do vetor de teste.
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

fn resign(chain: &mut JoinChainV2) {
    chain.links[0].certificate.sign(&signer(1));
    chain.links[1].certificate.issuer = godmesh_federation::join_chain::JoinIssuerV2::Parent {
        digest: chain.links[0].certificate.digest().unwrap(),
    };
    chain.links[1].certificate.sign(&signer(2));
}

#[test]
fn p_independent_vector_and_one_or_two_links() {
    let fixture = vector();
    let mut chain = chain();
    for (i, link) in chain.links.iter_mut().enumerate() {
        let reference = &fixture["expected"][i];
        assert_eq!(
            hex::encode(link.certificate.canonical_bytes()),
            reference["canonical_hex"]
        );
        assert_eq!(
            manifest_digest(&link.manifest).unwrap(),
            fixture["chain"]["links"][i]["certificate"]["manifest_digest"]
        );
        assert_eq!(link.certificate.digest().unwrap(), reference["link_digest"]);
        link.certificate.signature.clear();
        link.certificate.sign(&signer((i + 1).try_into().unwrap()));
        assert_eq!(
            link.certificate.signature,
            fixture["chain"]["links"][i]["certificate"]["signature"]
        );
    }
    let result = verify(&chain, 150).unwrap();
    assert_eq!(result, baseline());
    assert_eq!(result.cluster(), "cluster-teste");
    assert_eq!(result.root(), &policy().roots[0]);
    assert_eq!(result.leaf(), &expected(1));
    assert_eq!(result.leaf_digest(), fixture["expected"][1]["link_digest"]);
    assert_eq!(result.depth(), 2);
    assert_eq!(result.window(), (120, 180));
    assert!(!result.can_approve());

    chain.links.truncate(1);
    let result = verify_join_chain(
        &serde_json::to_vec(&chain).unwrap(),
        &expected(0),
        150,
        &policy(),
    )
    .unwrap();
    assert_eq!(result.leaf(), &expected(0));
    assert_eq!(result.window(), (100, 200));
    assert_eq!(result.depth(), 1);
    assert!(result.can_approve());
}

#[test]
fn p_equivalent_json_and_nonempty_manifest_lists() {
    let mut chain = chain();
    let link = &mut chain.links[1];
    link.manifest.capabilities = vec!["a".into(), "b".into()];
    link.manifest.endpoints = vec!["node-a:1".into(), "node-b:2".into()];
    link.manifest.sign(&signer(3));
    link.certificate.manifest_digest = manifest_digest(&link.manifest).unwrap();
    link.certificate.sign(&signer(2));
    let result = verify(&chain, 150).unwrap();
    let link = &mut chain.links[1];
    let canonical = link.manifest.canonical_bytes();
    link.manifest.capabilities.reverse();
    link.manifest.endpoints.reverse();
    assert_eq!(link.manifest.canonical_bytes(), canonical);
    assert_eq!(
        manifest_digest(&link.manifest).unwrap(),
        link.certificate.manifest_digest
    );
    let reordered = format!(
        "{{ \"links\" : {}, \"\\u0076ersion\" : 2 }}",
        serde_json::to_string_pretty(&json!(chain.links)).unwrap()
    );
    assert_eq!(
        verify_join_chain(reordered.as_bytes(), &expected(1), 150, &policy()),
        Ok(result)
    );
}

#[test]
fn n_topology_and_fields() {
    let genesis = |node_id: String| json!({"kind":"genesis","node_id":node_id});
    let parent = |digest| json!({"kind":"parent","digest":digest});
    for (index, field, value, error) in [
        (
            0,
            "issuer",
            genesis(format!("gmn_{}", "0".repeat(64))),
            UnknownRoot(0),
        ),
        (
            0,
            "issuer",
            parent("0".repeat(64)),
            InvalidIssuerPosition(0),
        ),
        (
            1,
            "issuer",
            genesis(policy().roots[0].node_id.clone()),
            InvalidIssuerPosition(1),
        ),
        (1, "issuer", parent("0".repeat(64)), ParentDigestMismatch(1)),
        (
            1,
            "candidate_public_key",
            json!(expected(0).public_key),
            InvalidField(1),
        ),
        (
            1,
            "candidate_node_id",
            json!(expected(0).node_id),
            InvalidField(1),
        ),
        (1, "cluster", json!("outro"), ClusterMismatch(1)),
        (1, "epoch", json!(i64::MAX as u64 + 1), InvalidField(1)),
        (1, "not_before", json!(180), InvalidWindow(1)),
    ] {
        let mut frame = vector()["chain"].clone();
        frame["links"][index]["certificate"][field] = value;
        assert_eq!(
            verify_join_chain(
                &serde_json::to_vec(&frame).unwrap(),
                &expected(1),
                150,
                &policy()
            ),
            Err(error)
        );
    }
    let mut value = chain();
    value.links.swap(0, 1);
    assert_eq!(verify(&value, 150), Err(InvalidIssuerPosition(0)));
    let mut value = chain();
    value.links.remove(0);
    assert_eq!(verify(&value, 150), Err(InvalidIssuerPosition(0)));

    let mut value = chain();
    value.links[0].certificate.can_approve = false;
    resign(&mut value);
    assert_eq!(verify(&value, 150), Err(DelegationDenied(1)));

    for (index, repeated) in [(0, signer(1)), (1, signer(2))] {
        let mut value = chain();
        let link = &mut value.links[index];
        link.manifest.sign(&repeated);
        link.certificate
            .candidate_node_id
            .clone_from(&link.manifest.node_id);
        link.certificate
            .candidate_public_key
            .clone_from(&link.manifest.public_key);
        link.certificate.manifest_digest = manifest_digest(&link.manifest).unwrap();
        resign(&mut value);
        assert_eq!(verify(&value, 150), Err(RepeatedIdentity(index)));
    }
    assert_eq!(
        verify_join_chain(
            &serde_json::to_vec(&chain()).unwrap(),
            &expected(0),
            150,
            &policy()
        ),
        Err(CandidateMismatch)
    );
}

#[test]
fn n_manifest_binding_digest_and_precedence() {
    let mut value = chain();
    value.links[1].manifest.signature = "00".repeat(64);
    assert_eq!(verify(&value, 150), Err(InvalidManifest(1)));

    let mut value = chain();
    value.links[1].manifest.capabilities = vec!["a".repeat(256); 40];
    value.links[1].manifest.sign(&signer(3));
    value.links[1].certificate.manifest_digest = manifest_digest(&value.links[1].manifest).unwrap();
    resign(&mut value);
    assert_eq!(verify(&value, 150), Err(InvalidManifest(1)));

    let mut value = chain();
    value.links[1].manifest = value.links[0].manifest.clone();
    value.links[1].certificate.manifest_digest = manifest_digest(&value.links[1].manifest).unwrap();
    resign(&mut value);
    assert_eq!(verify(&value, 150), Err(ManifestBindingMismatch(1)));

    let mut value = chain();
    value.links[1].manifest.region = "outro".into();
    value.links[1].manifest.sign(&signer(3));
    assert_eq!(verify(&value, 150), Err(ManifestDigestMismatch(1)));

    let mut value = chain();
    value.links[1].certificate.signature = "00".repeat(64);
    assert_eq!(verify(&value, 180), Err(InvalidSignature(1)));
    assert_eq!(verify(&value, 119), Err(InvalidSignature(1)));
    value.links[0].manifest.signature = "00".repeat(64);
    assert_eq!(verify(&value, 180), Err(InvalidManifest(0)));

    let mut value = chain();
    value.links[1].certificate.not_before = 99;
    resign(&mut value);
    assert_eq!(verify(&value, 150), Err(ChildWindowOutsideParent(1)));
    assert_eq!(verify(&value, 180), Err(Expired(1)));
}

#[test]
fn b_exact_frame_depth_and_window_limits() {
    let mut policy = policy();
    let frame = serde_json::to_vec(&chain()).unwrap();
    policy.max_frame_bytes = nz(frame.len());
    assert_eq!(
        verify_join_chain(&frame, &expected(1), 150, &policy),
        Ok(baseline())
    );
    let mut larger = frame.clone();
    larger.push(b' ');
    assert_eq!(
        verify_join_chain(&larger, &expected(1), 150, &policy),
        Err(FrameTooLarge)
    );
    policy.max_depth = nz(1);
    assert_eq!(
        verify_join_chain(&frame, &expected(1), 150, &policy),
        Err(DepthExceeded)
    );
    assert_eq!(
        verify_join_chain(br#"{"version":2,"links":[]}"#, &expected(1), 150, &policy),
        Err(EmptyChain)
    );
    for now in [120, 179] {
        assert_eq!(verify(&chain(), now), Ok(baseline()));
    }
    assert_eq!(verify(&chain(), 119), Err(NotYetValid(1)));
    assert_eq!(verify(&chain(), 180), Err(Expired(1)));

    let mut value = chain();
    for link in &mut value.links {
        link.certificate.not_before = i64::MIN;
        link.certificate.expires_at = i64::MAX;
        link.certificate.epoch = i64::MAX as u64;
    }
    resign(&mut value);
    for now in [i64::MIN, i64::MAX - 1] {
        assert_eq!(verify(&value, now).unwrap().window(), (i64::MIN, i64::MAX));
    }
    assert_eq!(verify(&value, i64::MAX), Err(Expired(0)));
}

#[test]
fn f_version_probe_strict_objects_and_recovery() {
    let valid = serde_json::to_vec(&chain()).unwrap();
    let result = baseline();
    let mut frames = vec![
        (valid[..valid.len() - 1].to_vec(), MalformedFrame),
        (b"not json".to_vec(), MalformedFrame),
        (
            br#"{"version":3,"links":{"other":[true,null]}}"#.to_vec(),
            UnsupportedVersion,
        ),
        (
            br#"{"version":18446744073709551615}"#.to_vec(),
            UnsupportedVersion,
        ),
        (br#"{"version":3,"version":2}"#.to_vec(), MalformedFrame),
        (br#"{"version":3,"links":[]"#.to_vec(), MalformedFrame),
        (br#"{"version":3} null"#.to_vec(), MalformedFrame),
        (br#"{"version":2.0,"links":[]}"#.to_vec(), MalformedFrame),
        (br#"{"version":-1}"#.to_vec(), MalformedFrame),
        (br#"{"version":"2"}"#.to_vec(), MalformedFrame),
        (br#"{"links":[]}"#.to_vec(), MalformedFrame),
        (
            br#"{"version":2,"links":[],"extra":null}"#.to_vec(),
            MalformedFrame,
        ),
        (br#"[2,[]]"#.to_vec(), MalformedFrame),
    ];

    let frame = vector()["chain"].clone();
    let link = &frame["links"][0];
    let cert = &link["certificate"];
    let mut fields = vec![];
    for field in [
        "join_request_id",
        "cluster",
        "candidate_node_id",
        "candidate_public_key",
        "manifest_digest",
        "issuer",
        "can_approve",
        "not_before",
        "expires_at",
        "epoch",
        "signature",
    ] {
        fields.push(cert[field].clone());
    }
    for path in ["link", "certificate", "issuer", "extra", "missing"] {
        let mut changed = frame.clone();
        let link = &mut changed["links"][0];
        match path {
            "link" => *link = json!([link["certificate"], link["manifest"]]),
            "certificate" => link["certificate"] = json!(fields),
            "issuer" => {
                link["certificate"]["issuer"] = json!(["genesis", policy().roots[0].node_id])
            }
            "extra" => link["certificate"]["extra"] = json!(true),
            "missing" => {
                link["certificate"]
                    .as_object_mut()
                    .unwrap()
                    .remove("can_approve");
            }
            _ => unreachable!(),
        }
        frames.push((serde_json::to_vec(&changed).unwrap(), MalformedFrame));
    }
    for (needle, replacement) in [
        (
            "\"can_approve\":true",
            "\"can_approve\":true,\"can_approve\":true",
        ),
        (
            "\"kind\":\"genesis\"",
            "\"kind\":\"genesis\",\"kind\":\"genesis\"",
        ),
        ("\"kind\":\"genesis\"", "\"kind\":\"genesis\",\"extra\":0"),
        ("\"links\":[", "\"links\":[],\"links\":["),
        ("\"certificate\":{", "\"extra\":0,\"certificate\":{"),
    ] {
        let raw = String::from_utf8(valid.clone()).unwrap();
        assert!(raw.contains(needle));
        frames.push((
            raw.replacen(needle, replacement, 1).into_bytes(),
            MalformedFrame,
        ));
    }
    for (frame, error) in frames {
        assert_eq!(
            verify_join_chain(&frame, &expected(1), 150, &policy()),
            Err(error),
            "{}",
            String::from_utf8_lossy(&frame)
        );
        assert_eq!(verify(&chain(), 150), Ok(result.clone()));
    }

    let mut duplicate = policy();
    duplicate.roots.push(duplicate.roots[0].clone());
    let mut empty = policy();
    empty.roots.clear();
    let mut wrong_key = policy();
    wrong_key.roots[0].public_key = expected(0).public_key;
    let mut invalid_identity = expected(1);
    invalid_identity.public_key = expected(0).public_key;
    for invalid in [duplicate, empty, wrong_key] {
        assert_eq!(
            verify_join_chain(b"invalid", &invalid_identity, 150, &invalid),
            Err(InvalidPolicy)
        );
        assert_eq!(verify(&chain(), 150), Ok(result.clone()));
    }
    assert_eq!(
        verify_join_chain(b"invalid", &invalid_identity, 150, &policy()),
        Err(InvalidExpectedIdentity)
    );
    assert_eq!(verify(&chain(), 150), Ok(result));
}

#[test]
fn n_v1_approval_and_rejection_remain_domain_separated() {
    let root = signer(1);
    let mut v1 = ApprovalCertificate {
        join_request_id: format!("fjr_{}", "c".repeat(32)),
        node_id: String::new(),
        public_key: String::new(),
        cluster: "cluster-teste".into(),
        scopes: vec!["join".into()],
        not_before: 120,
        expires_at: 180,
        epoch: 1,
        signature: String::new(),
    };
    let mut value = chain();
    for (action, other, domain) in [
        (
            DecisionAction::Approve,
            DecisionAction::Reject,
            b"godmesh.federation.approval.v1".as_slice(),
        ),
        (
            DecisionAction::Reject,
            DecisionAction::Approve,
            b"godmesh.federation.rejection.v1".as_slice(),
        ),
    ] {
        v1.sign_for(&root, action);
        assert!(v1.canonical_bytes_for(action).starts_with(domain));
        assert!(v1.verify_for(&root.public_key_hex(), action).is_ok());
        assert!(v1.verify_for(&root.public_key_hex(), other).is_err());

        let parent = signer(2);
        v1.sign_for(&parent, action);
        value.links[1]
            .certificate
            .signature
            .clone_from(&v1.signature);
        assert_eq!(verify(&value, 150), Err(InvalidSignature(1)));
        let canonical = value.links[1].certificate.canonical_bytes();
        let mut wrong_domain = domain.to_vec();
        wrong_domain.extend_from_slice(&canonical[b"godmesh.federation.join.approval.v2".len()..]);
        value.links[1].certificate.signature = parent.sign_hex(&wrong_domain);
        assert_eq!(verify(&value, 150), Err(InvalidSignature(1)));
    }
}
