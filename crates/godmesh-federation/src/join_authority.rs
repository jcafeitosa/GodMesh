//! Formatos assinados de vigência, revogação e recibo da M1C-B1.

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use sha2::{Digest, Sha256};

use crate::{
    NodeIdentity,
    identity::{self, SIGNATURE_LEN},
    join_chain::VerifiedJoinChain,
    manifest::{MAX_FIELD_LEN, push_field},
};

const VALIDITY_DOMAIN: &[u8] = b"godmesh.federation.join.status.v1";
const REVOCATION_DOMAIN: &[u8] = b"godmesh.federation.join.revocation.v1";
const RECEIPT_DOMAIN: &[u8] = b"godmesh.federation.join.revocation-receipt.v1";
const REVOCATION_DIGEST_DOMAIN: &[u8] = b"godmesh.federation.join.revocation-digest.v1";

/// Falha de verificação de um artefato M1C-B1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinAuthorityError {
    InvalidField,
    InvalidWindow,
    KeyMismatch,
    InvalidSignature,
    ArtifactMismatch,
    WindowOutsideChain,
    DurationExceeded,
    NotYetValid,
    Expired,
}

impl fmt::Display for JoinAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for JoinAuthorityError {}

use JoinAuthorityError::{
    ArtifactMismatch, DurationExceeded, Expired, InvalidField, InvalidSignature, InvalidWindow,
    KeyMismatch, NotYetValid, WindowOutsideChain,
};

// Como na M1C-A, o visitante admite somente mapas e preserva as chaves
// duplicadas para que o esquema derivado possa recusá-las.
fn deserialize_object<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    struct ObjectVisitor<T>(PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("um objeto")
        }

        fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
            T::deserialize(MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_map(ObjectVisitor(PhantomData))
}

macro_rules! signed_object {
    ($(#[$doc:meta])* $name:ident { $($field:ident: $ty:ty),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct $name {
            $(pub $field: $ty,)+
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Wire {
                    $($field: $ty,)+
                }

                let Wire { $($field,)+ } = deserialize_object(deserializer)?;
                Ok(Self { $($field,)+ })
            }
        }
    };
}

signed_object! {
    /// Declaração de vigência de uma folha, assinada pela raiz.
    JoinValidityV1 {
        cluster: String,
        root_node_id: String,
        leaf_digest: String,
        issued_at: i64,
        expires_at: i64,
        signature: String,
    }
}

signed_object! {
    /// Declaração de revogação de uma prova específica.
    JoinRevocationV1 {
        cluster: String,
        root_node_id: String,
        target_digest: String,
        revoker_node_id: String,
        epoch: u64,
        signature: String,
    }
}

signed_object! {
    /// Recibo da raiz para os bytes exatos de uma revogação.
    JoinRevocationReceiptV1 {
        cluster: String,
        root_node_id: String,
        revocation_digest: String,
        accepted_at: i64,
        signature: String,
    }
}

/// Evidência produzida pela verificação da revogação contra a cadeia alvo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedJoinRevocation {
    cluster: String,
    root_node_id: String,
    digest: String,
}

fn canonical_prefix(domain: &[u8], cluster: &str, root_node_id: &str) -> Vec<u8> {
    let mut bytes = domain.to_vec();
    push_field(&mut bytes, "cluster", cluster.as_bytes());
    push_field(&mut bytes, "root_node_id", root_node_id.as_bytes());
    bytes
}

fn lower_hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_fields(
    cluster: &str,
    root_node_id: &str,
    digest: &str,
    signature: &str,
) -> Result<(), JoinAuthorityError> {
    if cluster.len() > MAX_FIELD_LEN
        || cluster.trim().is_empty()
        || !identity::is_node_id(root_node_id)
        || !lower_hex(digest, 64)
        || !lower_hex(signature, SIGNATURE_LEN * 2)
    {
        return Err(InvalidField);
    }
    Ok(())
}

fn verify_signature(
    key: &str,
    signer: &str,
    canonical_bytes: impl FnOnce() -> Vec<u8>,
    signature: &str,
) -> Result<(), JoinAuthorityError> {
    if !identity::verify_node_id(signer, key) {
        return Err(KeyMismatch);
    }
    identity::verify_signature(key, &canonical_bytes(), signature).map_err(|_| InvalidSignature)
}

fn verify_chain_binding(
    chain: &VerifiedJoinChain,
    cluster: &str,
    root_node_id: &str,
    leaf_digest: &str,
) -> Result<(), JoinAuthorityError> {
    if cluster != chain.cluster()
        || root_node_id != chain.root().node_id
        || leaf_digest != chain.leaf_digest()
    {
        return Err(ArtifactMismatch);
    }
    Ok(())
}

impl JoinValidityV1 {
    /// Codifica os campos assinados na ordem da SPEC.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = canonical_prefix(VALIDITY_DOMAIN, &self.cluster, &self.root_node_id);
        push_field(&mut bytes, "leaf_digest", self.leaf_digest.as_bytes());
        push_field(&mut bytes, "issued_at", &self.issued_at.to_be_bytes());
        push_field(&mut bytes, "expires_at", &self.expires_at.to_be_bytes());
        bytes
    }

    /// Preenche a raiz com a identidade usada e assina a declaração.
    pub fn sign(&mut self, identity: &NodeIdentity) {
        self.root_node_id = identity.node_id().to_owned();
        self.signature = identity.sign_hex(&self.canonical_bytes());
    }

    /// Verifica estrutura, vínculo da chave e assinatura.
    ///
    /// # Errors
    /// Retorna a primeira falha nessa ordem.
    pub fn verify(&self, root_key: &str) -> Result<(), JoinAuthorityError> {
        validate_fields(
            &self.cluster,
            &self.root_node_id,
            &self.leaf_digest,
            &self.signature,
        )?;
        if self.issued_at >= self.expires_at {
            return Err(InvalidWindow);
        }
        verify_signature(
            root_key,
            &self.root_node_id,
            || self.canonical_bytes(),
            &self.signature,
        )
    }

    /// Verifica a vigência para a cadeia, o instante e o limite fornecidos.
    ///
    /// # Errors
    /// Verifica assinatura, vínculo, contenção, duração e instante nessa ordem.
    /// Um limite de duração não positivo retorna [`JoinAuthorityError::InvalidField`].
    pub fn verify_for_chain(
        &self,
        root_key: &str,
        chain: &VerifiedJoinChain,
        now: i64,
        max_validity_seconds: i64,
    ) -> Result<(), JoinAuthorityError> {
        self.verify(root_key)?;
        verify_chain_binding(chain, &self.cluster, &self.root_node_id, &self.leaf_digest)?;
        let (start, end) = chain.window();
        if self.issued_at < start || self.expires_at > end {
            return Err(WindowOutsideChain);
        }
        if max_validity_seconds <= 0 {
            return Err(InvalidField);
        }
        if i128::from(self.expires_at) - i128::from(self.issued_at)
            > i128::from(max_validity_seconds)
        {
            return Err(DurationExceeded);
        }
        if now < self.issued_at {
            return Err(NotYetValid);
        }
        if now >= self.expires_at {
            return Err(Expired);
        }
        Ok(())
    }
}

impl JoinRevocationV1 {
    /// Codifica os campos assinados na ordem da SPEC.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = canonical_prefix(REVOCATION_DOMAIN, &self.cluster, &self.root_node_id);
        push_field(&mut bytes, "target_digest", self.target_digest.as_bytes());
        push_field(
            &mut bytes,
            "revoker_node_id",
            self.revoker_node_id.as_bytes(),
        );
        push_field(&mut bytes, "epoch", &self.epoch.to_be_bytes());
        bytes
    }

    /// Preenche o revogador com a identidade usada e assina a declaração.
    pub fn sign(&mut self, identity: &NodeIdentity) {
        self.revoker_node_id = identity.node_id().to_owned();
        self.signature = identity.sign_hex(&self.canonical_bytes());
    }

    /// Verifica formato, vínculo da chave e assinatura.
    ///
    /// # Errors
    /// Retorna a primeira falha nessa ordem.
    pub fn verify(&self, revoker_key: &str) -> Result<(), JoinAuthorityError> {
        validate_fields(
            &self.cluster,
            &self.root_node_id,
            &self.target_digest,
            &self.signature,
        )?;
        if !identity::is_node_id(&self.revoker_node_id) || self.epoch > i64::MAX as u64 {
            return Err(InvalidField);
        }
        verify_signature(
            revoker_key,
            &self.revoker_node_id,
            || self.canonical_bytes(),
            &self.signature,
        )
    }

    /// Identifica os bytes da declaração e da assinatura.
    ///
    /// # Errors
    /// Retorna erro se a assinatura não puder ser decodificada em 64 bytes.
    pub fn digest(&self) -> Result<String, hex::FromHexError> {
        let mut signature = [0; SIGNATURE_LEN];
        hex::decode_to_slice(&self.signature, &mut signature)?;
        let mut bytes = REVOCATION_DIGEST_DOMAIN.to_vec();
        push_field(&mut bytes, "body", &self.canonical_bytes());
        push_field(&mut bytes, "signature", &signature);
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    /// Produz evidência após verificar a assinatura e o vínculo à folha alvo.
    /// O consumidor fornece a chave autorizada para revogar.
    ///
    /// # Errors
    /// Retorna a falha de verificação da assinatura ou do vínculo à cadeia.
    pub fn verify_for_chain(
        &self,
        revoker_key: &str,
        target: &VerifiedJoinChain,
    ) -> Result<VerifiedJoinRevocation, JoinAuthorityError> {
        self.verify(revoker_key)?;
        verify_chain_binding(
            target,
            &self.cluster,
            &self.root_node_id,
            &self.target_digest,
        )?;
        Ok(VerifiedJoinRevocation {
            cluster: self.cluster.clone(),
            root_node_id: self.root_node_id.clone(),
            digest: self.digest().map_err(|_| InvalidField)?,
        })
    }
}

impl JoinRevocationReceiptV1 {
    /// Codifica os campos assinados na ordem da SPEC.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = canonical_prefix(RECEIPT_DOMAIN, &self.cluster, &self.root_node_id);
        push_field(
            &mut bytes,
            "revocation_digest",
            self.revocation_digest.as_bytes(),
        );
        push_field(&mut bytes, "accepted_at", &self.accepted_at.to_be_bytes());
        bytes
    }

    /// Preenche a raiz com a identidade usada e assina o recibo.
    pub fn sign(&mut self, identity: &NodeIdentity) {
        self.root_node_id = identity.node_id().to_owned();
        self.signature = identity.sign_hex(&self.canonical_bytes());
    }

    /// Verifica formato, vínculo da chave e assinatura.
    ///
    /// # Errors
    /// Retorna a primeira falha nessa ordem.
    pub fn verify(&self, root_key: &str) -> Result<(), JoinAuthorityError> {
        validate_fields(
            &self.cluster,
            &self.root_node_id,
            &self.revocation_digest,
            &self.signature,
        )?;
        verify_signature(
            root_key,
            &self.root_node_id,
            || self.canonical_bytes(),
            &self.signature,
        )
    }

    /// Verifica o recibo com a chave confiável da raiz e a evidência da revogação.
    ///
    /// # Errors
    /// Retorna a falha de verificação da assinatura ou do vínculo à revogação.
    pub fn verify_for_revocation(
        &self,
        root_key: &str,
        revocation: &VerifiedJoinRevocation,
    ) -> Result<(), JoinAuthorityError> {
        self.verify(root_key)?;
        if self.cluster != revocation.cluster
            || self.root_node_id != revocation.root_node_id
            || self.revocation_digest != revocation.digest
        {
            return Err(ArtifactMismatch);
        }
        Ok(())
    }
}
