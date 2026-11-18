//! Manifesto de nó assinado.
//!
//! O manifesto é a carta de apresentação de um nó ao cluster: identidade
//! pública, cluster/região, versão, capacidades, endpoints e `epoch`. Ele chega
//! por uma rota **pública** (`POST /v1/federation/join-requests`), então a única
//! coisa que o servidor pode afirmar sobre o conteúdo é o que a assinatura prova.
//!
//! ## Serialização canônica (definida aqui, testada aqui)
//!
//! Assinar JSON seria frágil: ordem de chaves, espaçamento e normalização mudam
//! os bytes. A forma assinada é explícita:
//!
//! ```text
//! "godmesh.federation.manifest.v1"        -- domínio, separa de outros usos da chave
//! "node_id"  <u32 BE len><bytes>
//! "public_key" <u32 BE len><bytes>
//! "cluster"  <u32 BE len><bytes>
//! "region"   <u32 BE len><bytes>
//! "software_version" <u32 BE len><bytes>
//! "alpn"     <u32 BE len><bytes>
//! "capabilities" <u32 BE n> (<u32 BE len><bytes>)*   -- ordenado e sem repetição
//! "endpoints"    <u32 BE n> (<u32 BE len><bytes>)*   -- ordenado e sem repetição
//! "epoch"    <u64 BE>
//! "issued_at" <i64 BE>
//! ```
//!
//! Cada campo carrega o próprio nome antes do valor, então dois campos não se
//! confundem por acidente de posição. `capabilities` e `endpoints` são
//! normalizados (ordenados, sem duplicata) **antes** de assinar e antes de
//! verificar: são conjuntos, e a ordem de envio não deve mudar a assinatura.
//! `signature` não entra nos bytes assinados, obviamente.
//!
//! Consequência testada: mesmos campos → mesmos bytes → mesma assinatura, mesmo
//! que `capabilities` chegue em ordem diferente. Alterar um único byte de
//! qualquer campo invalida a assinatura.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::identity::{self, IdentityError, NodeIdentity};

/// Domínio da assinatura do manifesto (versão 1).
pub const MANIFEST_DOMAIN: &[u8] = b"godmesh.federation.manifest.v1";

/// Janela padrão de frescor de `issued_at`: 5 minutos.
pub const DEFAULT_MAX_AGE_SECONDS: i64 = 300;

/// Máximo de itens em `capabilities`/`endpoints`.
pub const MAX_LIST_ITEMS: usize = 128;

/// Máximo de bytes de um campo textual.
pub const MAX_FIELD_LEN: usize = 256;

/// Teto de um frame de manifesto na rede, em bytes.
///
/// Recusa **antes** de alocar/parsear: um frame anunciado como gigante não pode
/// consumir memória do processo que o recebe.
pub const MAX_MANIFEST_FRAME_BYTES: usize = 8 * 1024;

/// Manifesto assinado por um nó.
///
/// `deny_unknown_fields` é deliberado: o corpo vem da rede e um campo
/// desconhecido não é ignorado em silêncio — o pedido é recusado na fronteira.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedManifest {
    /// Identificador público do nó: `gmn_` + hex de `sha256(public_key)`.
    pub node_id: String,
    /// Chave pública Ed25519 em hex minúsculo (64 caracteres).
    pub public_key: String,
    /// Cluster a que o nó pede ingresso.
    pub cluster: String,
    /// Região declarada pelo nó.
    pub region: String,
    /// Versão do software do nó.
    pub software_version: String,
    /// ALPN declarado pelo nó (versão do protocolo de federação que ele fala).
    ///
    /// Vazio significa **não declarado** — é o caso do ingresso por API (M1),
    /// onde não há sessão TLS para casar. O transporte (M2) exige que o valor
    /// declarado seja exatamente o ALPN negociado: a versão do protocolo está
    /// no conteúdo assinado, não só no handshake. Campo estendido pela fatia
    /// M2; a serialização é retrocompatível porque `serde(default)` aceita
    /// manifesto antigo sem a chave.
    #[serde(default)]
    pub alpn: String,
    /// Capacidades declaradas (conjunto; a ordem não é assinada).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Endpoints declarados (conjunto; a ordem não é assinada).
    #[serde(default)]
    pub endpoints: Vec<String>,
    /// Geração da identidade de federação do nó.
    pub epoch: u64,
    /// Instante de emissão em segundos Unix (UTC).
    pub issued_at: i64,
    /// Assinatura Ed25519 (hex minúsculo, 128 caracteres) sobre os bytes canônicos.
    pub signature: String,
}

/// Falhas de validação do manifesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Campo textual obrigatório vazio.
    EmptyField(&'static str),
    /// Campo textual acima de [`MAX_FIELD_LEN`].
    FieldTooLong { field: &'static str, len: usize },
    /// Lista acima de [`MAX_LIST_ITEMS`].
    TooManyItems { field: &'static str, len: usize },
    /// `node_id` fora do formato `gmn_` + 64 hex.
    InvalidNodeId(String),
    /// Chave pública fora do formato esperado.
    InvalidPublicKey(String),
    /// Assinatura fora do formato esperado.
    InvalidSignatureEncoding(String),
    /// Assinatura não confere com os bytes canônicos.
    SignatureInvalid,
    /// `node_id` declarado não é o derivado da chave pública apresentada.
    NodeIdMismatch,
    /// `issued_at` velho demais para a janela configurada.
    Expired {
        issued_at: i64,
        now: i64,
        max_age_seconds: i64,
    },
    /// `issued_at` no futuro além da tolerância de relógio.
    NotYetValid { issued_at: i64, now: i64 },
    /// Frame acima de [`MAX_MANIFEST_FRAME_BYTES`].
    FrameTooLarge { len: usize },
    /// Frame ilegível como manifesto desta versão (JSON inválido, campo
    /// desconhecido ou byte sobrando).
    FrameMalformed(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "campo obrigatório vazio: {field}"),
            Self::FieldTooLong { field, len } => {
                write!(f, "campo {field} com {len} bytes; máximo {}", MAX_FIELD_LEN)
            }
            Self::TooManyItems { field, len } => {
                write!(f, "lista {field} com {len} itens; máximo {MAX_LIST_ITEMS}")
            }
            Self::InvalidNodeId(detail) => write!(f, "node_id inválido: {detail}"),
            Self::InvalidPublicKey(detail) => write!(f, "chave pública inválida: {detail}"),
            Self::InvalidSignatureEncoding(detail) => {
                write!(f, "assinatura inválida: {detail}")
            }
            Self::SignatureInvalid => write!(f, "assinatura não confere com o manifesto"),
            Self::NodeIdMismatch => {
                write!(f, "node_id não é o derivado da chave pública apresentada")
            }
            Self::Expired {
                issued_at,
                now,
                max_age_seconds,
            } => write!(
                f,
                "manifesto expirado: issued_at={issued_at}, now={now}, janela={max_age_seconds}s"
            ),
            Self::NotYetValid { issued_at, now } => write!(
                f,
                "manifesto emitido no futuro: issued_at={issued_at}, now={now}"
            ),
            Self::FrameTooLarge { len } => write!(
                f,
                "frame de manifesto com {len} bytes; máximo {MAX_MANIFEST_FRAME_BYTES}"
            ),
            Self::FrameMalformed(detail) => write!(f, "frame de manifesto ilegível: {detail}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<IdentityError> for ManifestError {
    fn from(e: IdentityError) -> Self {
        match e {
            IdentityError::InvalidPublicKey(detail) => Self::InvalidPublicKey(detail),
            IdentityError::InvalidSignatureEncoding(detail) => {
                Self::InvalidSignatureEncoding(detail)
            }
            IdentityError::SignatureInvalid => Self::SignatureInvalid,
            other => Self::InvalidPublicKey(other.to_string()),
        }
    }
}

pub(crate) fn push_field(out: &mut Vec<u8>, tag: &str, value: &[u8]) {
    out.extend_from_slice(&(tag.len() as u32).to_be_bytes());
    out.extend_from_slice(tag.as_bytes());
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}

pub(crate) fn normalise(items: &[String]) -> Vec<&str> {
    let mut items: Vec<&str> = items.iter().map(String::as_str).collect();
    items.sort_unstable();
    items.dedup();
    items
}

impl SignedManifest {
    /// Bytes canônicos assinados (sem `signature`).
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(MANIFEST_DOMAIN);
        push_field(&mut out, "node_id", self.node_id.as_bytes());
        push_field(&mut out, "public_key", self.public_key.as_bytes());
        push_field(&mut out, "cluster", self.cluster.as_bytes());
        push_field(&mut out, "region", self.region.as_bytes());
        push_field(
            &mut out,
            "software_version",
            self.software_version.as_bytes(),
        );
        push_field(&mut out, "alpn", self.alpn.as_bytes());
        let capabilities = normalise(&self.capabilities);
        push_field(
            &mut out,
            "capabilities",
            &(capabilities.len() as u32).to_be_bytes(),
        );
        for item in capabilities {
            push_field(&mut out, "capability", item.as_bytes());
        }
        let endpoints = normalise(&self.endpoints);
        push_field(
            &mut out,
            "endpoints",
            &(endpoints.len() as u32).to_be_bytes(),
        );
        for item in endpoints {
            push_field(&mut out, "endpoint", item.as_bytes());
        }
        push_field(&mut out, "epoch", &self.epoch.to_be_bytes());
        push_field(&mut out, "issued_at", &self.issued_at.to_be_bytes());
        out
    }

    /// Assina o manifesto com a identidade do nó.
    ///
    /// Preenche `node_id`, `public_key` e `signature` a partir da chave. O
    /// chamador não escolhe o `node_id`: ele é sempre o derivado da chave que
    /// assina, o que elimina a possibilidade de assinar em nome de outro nó.
    pub fn sign(&mut self, identity: &NodeIdentity) {
        self.node_id = identity.node_id().to_owned();
        self.public_key = identity.public_key_hex();
        self.signature = identity.sign_hex(&self.canonical_bytes());
    }

    /// Verifica formato dos campos, ligação `node_id`↔chave e assinatura.
    ///
    /// # Errors
    /// [`ManifestError`] para campo inválido, `node_id` não derivado da chave ou
    /// assinatura que não confere.
    pub fn verify(&self) -> Result<(), ManifestError> {
        self.validate_fields()?;
        if !identity::is_node_id(&self.node_id) {
            return Err(ManifestError::InvalidNodeId(self.node_id.clone()));
        }
        // A chave é decodificada antes da assinatura para que um hex inválido
        // seja reportado como tal, e não como "assinatura inválida".
        identity::parse_public_key(&self.public_key)?;
        if !identity::verify_node_id(&self.node_id, &self.public_key) {
            return Err(ManifestError::NodeIdMismatch);
        }
        identity::verify_signature(&self.public_key, &self.canonical_bytes(), &self.signature)?;
        Ok(())
    }

    /// Serializa o manifesto para transporte em stream: o JSON do contrato (o
    /// mesmo corpo do ingresso por API), com teto de tamanho.
    ///
    /// O frame **não** é a forma assinada — a assinatura cobre
    /// [`Self::canonical_bytes`], que é independente de serializador. O frame é
    /// só o envelope de fio: um único formato para o mesmo dado evita um segundo
    /// codec com as mesmas regras duplicadas.
    ///
    /// # Errors
    /// [`ManifestError::FrameTooLarge`] se o resultado passar do teto.
    pub fn to_frame(&self) -> Result<Vec<u8>, ManifestError> {
        let bytes =
            serde_json::to_vec(self).map_err(|e| ManifestError::FrameMalformed(e.to_string()))?;
        if bytes.len() > MAX_MANIFEST_FRAME_BYTES {
            return Err(ManifestError::FrameTooLarge { len: bytes.len() });
        }
        Ok(bytes)
    }

    /// Lê um frame **sem** verificar a assinatura.
    ///
    /// `verify`/`verify_fresh` continuam obrigatórios antes de qualquer decisão;
    /// esta separação existe para que erro de formato e erro de assinatura sejam
    /// distinguíveis (códigos de fechamento diferentes).
    ///
    /// # Errors
    /// [`ManifestError::FrameTooLarge`] sem parsear; [`ManifestError::FrameMalformed`]
    /// para JSON inválido, campo desconhecido ou byte sobrando.
    pub fn from_frame(bytes: &[u8]) -> Result<Self, ManifestError> {
        if bytes.len() > MAX_MANIFEST_FRAME_BYTES {
            return Err(ManifestError::FrameTooLarge { len: bytes.len() });
        }
        serde_json::from_slice(bytes).map_err(|e| ManifestError::FrameMalformed(e.to_string()))
    }

    /// Verifica a assinatura **e** o frescor de `issued_at`.
    ///
    /// Rejeita manifesto velho (`now - issued_at > max_age_seconds`) e manifesto
    /// emitido no futuro além da mesma janela (relógio adiantado não pode
    /// esticar a validade). `max_age_seconds` é a janela configurada pelo
    /// servidor; [`DEFAULT_MAX_AGE_SECONDS`] é o padrão.
    ///
    /// # Errors
    /// Erros de [`Self::verify`] e [`ManifestError::Expired`] /
    /// [`ManifestError::NotYetValid`].
    pub fn verify_fresh(&self, now: i64, max_age_seconds: i64) -> Result<(), ManifestError> {
        self.verify()?;
        let age = now.saturating_sub(self.issued_at);
        if age > max_age_seconds {
            return Err(ManifestError::Expired {
                issued_at: self.issued_at,
                now,
                max_age_seconds,
            });
        }
        if self.issued_at.saturating_sub(now) > max_age_seconds {
            return Err(ManifestError::NotYetValid {
                issued_at: self.issued_at,
                now,
            });
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<(), ManifestError> {
        for (field, value) in [
            ("node_id", &self.node_id),
            ("public_key", &self.public_key),
            ("cluster", &self.cluster),
            ("region", &self.region),
            ("software_version", &self.software_version),
            ("signature", &self.signature),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::EmptyField(field));
            }
            if value.len() > MAX_FIELD_LEN.max(128) {
                return Err(ManifestError::FieldTooLong {
                    field,
                    len: value.len(),
                });
            }
        }
        // `alpn` é opcional no contrato (ingresso por API não tem sessão TLS),
        // mas quando declarado obedece ao mesmo teto de campo.
        if self.alpn.len() > MAX_FIELD_LEN {
            return Err(ManifestError::FieldTooLong {
                field: "alpn",
                len: self.alpn.len(),
            });
        }
        for (field, items) in [
            ("capabilities", &self.capabilities),
            ("endpoints", &self.endpoints),
        ] {
            if items.len() > MAX_LIST_ITEMS {
                return Err(ManifestError::TooManyItems {
                    field,
                    len: items.len(),
                });
            }
            for item in items {
                if item.trim().is_empty() {
                    return Err(ManifestError::EmptyField(field));
                }
                if item.len() > MAX_FIELD_LEN {
                    return Err(ManifestError::FieldTooLong {
                        field,
                        len: item.len(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(identity: &NodeIdentity) -> SignedManifest {
        let mut manifest = SignedManifest {
            node_id: String::new(),
            public_key: String::new(),
            cluster: "cluster-br-sul".to_owned(),
            region: "br-sul".to_owned(),
            software_version: "0.1.0".to_owned(),
            alpn: "godmesh/1".to_owned(),
            capabilities: vec!["sync".to_owned(), "events".to_owned()],
            endpoints: vec!["quic://10.0.0.1:7000".to_owned()],
            epoch: 3,
            issued_at: 1_700_000_000,
            signature: String::new(),
        };
        manifest.sign(identity);
        manifest
    }

    #[test]
    fn mesmos_campos_mesmos_bytes_mesma_assinatura() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let first = manifest(&identity);
        let second = manifest(&identity);
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.signature, second.signature);
        first.verify().expect("assinatura válida");
    }

    #[test]
    fn ordem_de_capabilities_nao_muda_a_assinatura() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let first = manifest(&identity);
        let mut reordered = first.clone();
        reordered.capabilities = vec!["events".to_owned(), "sync".to_owned()];
        assert_eq!(first.canonical_bytes(), reordered.canonical_bytes());
        reordered.verify().expect("ordem de conjunto não importa");
    }

    #[test]
    fn um_byte_alterado_invalida_a_assinatura() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let original = manifest(&identity);

        let mut outro_epoch = original.clone();
        outro_epoch.epoch += 1;
        assert_eq!(outro_epoch.verify(), Err(ManifestError::SignatureInvalid));

        let mut outro_cluster = original.clone();
        outro_cluster.cluster = "cluster-outro".to_owned();
        assert_eq!(outro_cluster.verify(), Err(ManifestError::SignatureInvalid));

        let mut outra_capacidade = original.clone();
        outra_capacidade.capabilities.push("metrics".to_owned());
        assert_eq!(
            outra_capacidade.verify(),
            Err(ManifestError::SignatureInvalid)
        );

        let mut outro_instante = original.clone();
        outro_instante.issued_at += 1;
        assert_eq!(
            outro_instante.verify(),
            Err(ManifestError::SignatureInvalid)
        );
    }

    #[test]
    fn node_id_de_outro_no_e_recusado() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let other = NodeIdentity::generate_in_memory().expect("outra identidade");
        let mut forjado = manifest(&identity);
        // Assinatura continua válida para a chave do nó, mas o `node_id`
        // declarado não é o derivado dela.
        forjado.node_id = other.node_id().to_owned();
        assert_eq!(forjado.verify(), Err(ManifestError::NodeIdMismatch));
    }

    #[test]
    fn manifesto_exige_campos_obrigatorios() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let mut vazio = manifest(&identity);
        vazio.cluster = "   ".to_owned();
        assert_eq!(vazio.verify(), Err(ManifestError::EmptyField("cluster")));
    }

    #[test]
    fn frescor_recusa_manifesto_velho_e_futuro() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let manifest = manifest(&identity);
        manifest
            .verify_fresh(manifest.issued_at + 10, DEFAULT_MAX_AGE_SECONDS)
            .expect("10 s de idade está dentro da janela");

        assert!(matches!(
            manifest.verify_fresh(manifest.issued_at + 301, DEFAULT_MAX_AGE_SECONDS),
            Err(ManifestError::Expired { .. })
        ));
        assert!(matches!(
            manifest.verify_fresh(manifest.issued_at - 301, DEFAULT_MAX_AGE_SECONDS),
            Err(ManifestError::NotYetValid { .. })
        ));
        // Janela configurável: o mesmo manifesto passa com janela maior.
        manifest
            .verify_fresh(manifest.issued_at + 301, 3_600)
            .expect("janela configurada aceita");
    }

    #[test]
    fn alpn_declarado_entra_nos_bytes_assinados() {
        use crate::identity::NodeIdentity;
        use crate::manifest::SignedManifest;
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let mut base = SignedManifest {
            node_id: String::new(),
            public_key: String::new(),
            cluster: "cluster-br-sul".to_owned(),
            region: "br-sul".to_owned(),
            software_version: "0.1.0".to_owned(),
            capabilities: vec![],
            endpoints: vec![],
            alpn: "godmesh/1".to_owned(),
            epoch: 1,
            issued_at: 1_700_000_000,
            signature: String::new(),
        };
        base.sign(&identity);
        base.verify().expect("assinatura válida");

        // Trocar a versão de protocolo declarada invalida a assinatura: a
        // versão não é campo decorativo.
        let mut outro_alpn = base.clone();
        outro_alpn.alpn = "godmesh/2".to_owned();
        assert_eq!(outro_alpn.verify(), Err(ManifestError::SignatureInvalid));
    }

    #[test]
    fn frame_faz_round_trip_e_verifica() {
        let identity = crate::identity::NodeIdentity::generate_in_memory().expect("identidade");
        let signed = manifest(&identity);
        let frame = signed.to_frame().expect("frame");
        let parsed = crate::manifest::SignedManifest::from_frame(&frame).expect("lê frame");
        assert_eq!(parsed, signed);
        parsed.verify().expect("assinatura válida depois do frame");
    }

    #[test]
    fn frame_acima_do_teto_e_recusado_antes_do_parse() {
        let bytes = vec![b'{'; MAX_MANIFEST_FRAME_BYTES + 1];
        assert_eq!(
            crate::manifest::SignedManifest::from_frame(&bytes),
            Err(ManifestError::FrameTooLarge {
                len: MAX_MANIFEST_FRAME_BYTES + 1
            })
        );
    }

    #[test]
    fn frame_com_campo_desconhecido_e_recusado() {
        // `deny_unknown_fields` vale no fio: campo extra não é ignorado em
        // silêncio, é recusado na fronteira.
        let identity = crate::identity::NodeIdentity::generate_in_memory().expect("identidade");
        let signed = manifest(&identity);
        let mut value = serde_json::to_value(&signed).expect("json");
        value["campo_extra"] = serde_json::json!(1);
        let bytes = serde_json::to_vec(&value).expect("json");
        assert!(matches!(
            crate::manifest::SignedManifest::from_frame(&bytes),
            Err(ManifestError::FrameMalformed(_))
        ));
    }

    #[test]
    fn manifesto_sem_alpn_continua_legivel() {
        // Retrocompatibilidade: manifesto de ingresso M1 não tem `alpn`.
        let identity = crate::identity::NodeIdentity::generate_in_memory().expect("identidade");
        let mut signed = manifest(&identity);
        signed.alpn = String::new();
        signed.sign(&identity);
        let mut value = serde_json::to_value(&signed).expect("json");
        value
            .as_object_mut()
            .expect("objeto")
            .remove("alpn");
        let bytes = serde_json::to_vec(&value).expect("json");
        let parsed = crate::manifest::SignedManifest::from_frame(&bytes).expect("lê sem alpn");
        assert_eq!(parsed.alpn, "");
        parsed.verify().expect("vazio é a forma assinada correta");
    }
}
