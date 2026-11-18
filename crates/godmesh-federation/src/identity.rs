//! Identidade de nó da federação: par Ed25519 persistido em arquivo.
//!
//! Regras desta fatia (ADR-069 + tarefa M1):
//!
//! * **nenhuma criptografia própria**: a assinatura e a verificação são
//!   `ed25519_dalek` 3.0.0 (RustCrypto), sem construção de curva ou de hash
//!   escrita aqui;
//! * a chave privada **só** existe em memória e no arquivo apontado por
//!   `GODMESH_NODE_KEY`; ela nunca entra em resposta HTTP, log, erro ou `Debug`;
//! * o arquivo nasce com permissão `0600` e é recusado se estiver mais aberto
//!   (falha fechada: chave legível por outro usuário não é identidade confiável);
//! * `node_id` é derivado da chave pública — `gmn_` + hex de `sha256(pubkey)` —
//!   e a verificação reconfere essa ligação, para que um manifesto não possa
//!   declarar um `node_id` que não corresponde à chave que assinou.
//!
//! O `Debug` de [`NodeIdentity`] é escrito à mão e mostra apenas `node_id` e o
//! caminho do arquivo. Não existe `Display`, `Serialize` nem acesso à semente
//! por API pública: o byte secreto não sai de `sign`.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// Variável de ambiente com o caminho do arquivo da chave do nó.
pub const NODE_KEY_ENV: &str = "GODMESH_NODE_KEY";

/// Prefixo do identificador público do nó.
pub const NODE_ID_PREFIX: &str = "gmn_";

/// Permissão exigida do arquivo de chave (somente o dono lê e escreve).
pub const KEY_FILE_MODE: u32 = 0o600;

/// Permissão do diretório criado para conter a chave.
pub const KEY_DIR_MODE: u32 = 0o700;

/// Tamanho da chave pública Ed25519 em bytes.
pub const PUBLIC_KEY_LEN: usize = 32;

/// Tamanho da assinatura Ed25519 em bytes.
pub const SIGNATURE_LEN: usize = 64;

/// Tamanho da semente Ed25519 em bytes.
const SEED_LEN: usize = 32;

/// Falhas de leitura, criação e verificação da identidade.
#[derive(Debug)]
pub enum IdentityError {
    /// `GODMESH_NODE_KEY` ausente ou vazia.
    EnvMissing,
    /// Falha de sistema de arquivos, com o caminho para diagnóstico.
    Io { path: PathBuf, detail: String },
    /// Conteúdo do arquivo não é 32 bytes em hex (64 caracteres).
    InvalidKeyMaterial { path: PathBuf, detail: String },
    /// Arquivo com permissão mais aberta que `0600`.
    InsecureMode { path: PathBuf, mode: u32 },
    /// O caminho existe e não é arquivo regular (diretório, socket, ...).
    NotARegularFile { path: PathBuf },
    /// O RNG do sistema não respondeu: sem entropia não se inventa chave.
    RandomnessUnavailable(String),
    /// A chave não pôde ser exportada em PKCS#8 (exigida pelo certificado
    /// autoemitido do transporte). O nó não sobe com identidade duvidosa.
    KeyExport(String),
    /// Chave pública fora do formato (hex de 32 bytes).
    InvalidPublicKey(String),
    /// Assinatura fora do formato (hex de 64 bytes).
    InvalidSignatureEncoding(String),
    /// Assinatura não confere com a chave pública e os bytes apresentados.
    SignatureInvalid,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnvMissing => write!(
                f,
                "{NODE_KEY_ENV} não definida; o nó não tem onde persistir a identidade"
            ),
            Self::Io { path, detail } => {
                write!(f, "falha de arquivo em {}: {detail}", path.display())
            }
            Self::InvalidKeyMaterial { path, detail } => write!(
                f,
                "material de chave inválido em {}: {detail}",
                path.display()
            ),
            Self::InsecureMode { path, mode } => write!(
                f,
                "arquivo de chave {} tem permissão {mode:o}; exigido {:o}",
                path.display(),
                KEY_FILE_MODE
            ),
            Self::NotARegularFile { path } => {
                write!(f, "{} existe e não é arquivo regular", path.display())
            }
            Self::RandomnessUnavailable(detail) => {
                write!(
                    f,
                    "RNG do sistema indisponível; nenhuma chave foi criada: {detail}"
                )
            }
            Self::KeyExport(detail) => write!(f, "chave não exportável em PKCS#8: {detail}"),
            Self::InvalidPublicKey(detail) => write!(f, "chave pública inválida: {detail}"),
            Self::InvalidSignatureEncoding(detail) => {
                write!(f, "assinatura inválida: {detail}")
            }
            Self::SignatureInvalid => write!(f, "assinatura não confere com a chave pública"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// `node_id` derivado da chave pública: `gmn_` + hex minúsculo de
/// `sha256(pubkey)`.
///
/// O que se publica é um compromisso com a chave, não um identificador livre: o
/// verificador recomputa a partir da chave que recebeu, então um manifesto que
/// declare outro `node_id` é rejeitado (ver
/// [`verify_node_id`]).
#[must_use]
pub fn node_id_for(public_key: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(public_key);
    format!("{NODE_ID_PREFIX}{}", hex::encode(digest))
}

/// Reconhece o formato de `node_id`: prefixo e 64 hex minúsculos.
#[must_use]
pub fn is_node_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix(NODE_ID_PREFIX) else {
        return false;
    };
    rest.len() == 64
        && rest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Confere que `node_id` é o derivado de `public_key_hex`.
#[must_use]
pub fn verify_node_id(node_id: &str, public_key_hex: &str) -> bool {
    match parse_public_key(public_key_hex) {
        Ok(key) => node_id_for(&key.to_bytes()) == node_id,
        Err(_) => false,
    }
}

/// Decodifica a chave pública Ed25519 de hex.
///
/// # Errors
/// Hex inválido, tamanho diferente de 32 bytes ou ponto fora da curva.
pub fn parse_public_key(public_key_hex: &str) -> Result<VerifyingKey, IdentityError> {
    let bytes = hex::decode(public_key_hex)
        .map_err(|e| IdentityError::InvalidPublicKey(format!("hex inválido: {e}")))?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| IdentityError::InvalidPublicKey("esperados 32 bytes".to_owned()))?;
    VerifyingKey::from_bytes(&key)
        .map_err(|e| IdentityError::InvalidPublicKey(format!("ponto inválido: {e}")))
}

/// Verifica uma assinatura hex sobre `bytes`.
///
/// `verify_strict` é usado de propósito: recusa chaves públicas de pequena
/// ordem e assinaturas maleáveis, o que a verificação "permissiva" aceitaria.
///
/// # Errors
/// Chave pública, assinatura ou bytes que não conferem.
pub fn verify_signature(
    public_key_hex: &str,
    bytes: &[u8],
    signature_hex: &str,
) -> Result<(), IdentityError> {
    let key = parse_public_key(public_key_hex)?;
    let raw = hex::decode(signature_hex)
        .map_err(|e| IdentityError::InvalidSignatureEncoding(format!("hex inválido: {e}")))?;
    let signature: [u8; 64] = raw
        .try_into()
        .map_err(|_| IdentityError::InvalidSignatureEncoding("esperados 64 bytes".to_owned()))?;
    let signature = Signature::from_bytes(&signature);
    key.verify_strict(bytes, &signature)
        .map_err(|_| IdentityError::SignatureInvalid)
}

/// Identidade do nó: chave privada em memória, chave pública e `node_id`.
///
/// O `Debug` mostra `node_id`, chave pública e caminho — nunca a semente.
pub struct NodeIdentity {
    path: Option<PathBuf>,
    signing_key: SigningKey,
    node_id: String,
}

impl fmt::Debug for NodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeIdentity")
            .field("node_id", &self.node_id)
            .field(
                "public_key",
                &hex::encode(self.signing_key.verifying_key().to_bytes()),
            )
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl NodeIdentity {
    /// Carrega a identidade de `path`; se o arquivo não existir, gera e grava.
    ///
    /// O arquivo é criado com `0600` e o diretório pai, se ausente, com `0700`.
    /// Depois de gravar, a permissão é reaplicada (o umask poderia reduzi-la, e
    /// o contrato é `0600` exato).
    ///
    /// # Errors
    /// [`IdentityError`] para falha de arquivo, material inválido, permissão
    /// aberta demais ou RNG indisponível.
    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let path = path.as_ref().to_path_buf();
        match OpenOptions::new().read(true).open(&path) {
            Ok(_) => Self::load(&path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::create(&path),
            Err(e) => Err(IdentityError::Io {
                path,
                detail: e.to_string(),
            }),
        }
    }

    /// Carrega a identidade do caminho em `GODMESH_NODE_KEY`.
    ///
    /// # Errors
    /// [`IdentityError::EnvMissing`] sem a variável; demais falhas de
    /// [`Self::load_or_create`].
    pub fn from_env() -> Result<Self, IdentityError> {
        let path = std::env::var(NODE_KEY_ENV)
            .map_err(|_| IdentityError::EnvMissing)?
            .trim()
            .to_owned();
        if path.is_empty() {
            return Err(IdentityError::EnvMissing);
        }
        Self::load_or_create(path)
    }

    /// Carrega a identidade de um arquivo existente, exigindo `0600`.
    ///
    /// # Errors
    /// [`IdentityError::InsecureMode`] se o arquivo estiver mais aberto que
    /// `0600`; [`IdentityError::InvalidKeyMaterial`] se o conteúdo não for a
    /// semente em hex.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let path = path.as_ref().to_path_buf();
        let meta = fs::metadata(&path).map_err(|e| IdentityError::Io {
            path: path.clone(),
            detail: e.to_string(),
        })?;
        if !meta.is_file() {
            return Err(IdentityError::NotARegularFile { path });
        }
        let mode = meta.permissions().mode() & 0o777;
        if mode != KEY_FILE_MODE {
            return Err(IdentityError::InsecureMode { path, mode });
        }
        let mut file = fs::File::open(&path).map_err(|e| IdentityError::Io {
            path: path.clone(),
            detail: e.to_string(),
        })?;
        let mut raw = String::new();
        file.read_to_string(&mut raw)
            .map_err(|e| IdentityError::Io {
                path: path.clone(),
                detail: e.to_string(),
            })?;
        Self::from_seed_hex(path.clone(), raw.trim()).map(|identity| Self {
            path: Some(path),
            ..identity
        })
    }

    /// Gera uma identidade nova e grava o arquivo com `0600`.
    ///
    /// # Errors
    /// Falha de arquivo ou RNG indisponível.
    pub fn create(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| IdentityError::Io {
                path: parent.to_path_buf(),
                detail: e.to_string(),
            })?;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(KEY_DIR_MODE));
        }
        let mut seed = [0_u8; SEED_LEN];
        getrandom::fill(&mut seed)
            .map_err(|e| IdentityError::RandomnessUnavailable(e.to_string()))?;
        let signing_key = SigningKey::from_bytes(&seed);

        // `create_new` é O_EXCL: dois processos não sobrescrevem a chave um do
        // outro. Se o arquivo apareceu entre a leitura e a criação, carrega-se
        // o existente em vez de substituí-lo.
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(KEY_FILE_MODE)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Self::load(&path),
            Err(e) => {
                return Err(IdentityError::Io {
                    path,
                    detail: e.to_string(),
                });
            }
        };
        fs::set_permissions(&path, fs::Permissions::from_mode(KEY_FILE_MODE)).map_err(|e| {
            IdentityError::Io {
                path: path.clone(),
                detail: e.to_string(),
            }
        })?;
        let body = format!("{}\n", hex::encode(seed));
        if let Err(e) = file
            .write_all(body.as_bytes())
            .and_then(|()| file.sync_all())
        {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(IdentityError::Io {
                path,
                detail: e.to_string(),
            });
        }
        drop(file);
        Ok(Self {
            path: Some(path),
            node_id: node_id_for(&signing_key.verifying_key().to_bytes()),
            signing_key,
        })
    }

    fn from_seed_hex(path: PathBuf, seed_hex: &str) -> Result<Self, IdentityError> {
        let bytes = hex::decode(seed_hex).map_err(|e| IdentityError::InvalidKeyMaterial {
            path: path.clone(),
            detail: format!("hex inválido: {e}"),
        })?;
        let seed: [u8; SEED_LEN] =
            bytes
                .try_into()
                .map_err(|_| IdentityError::InvalidKeyMaterial {
                    path: path.clone(),
                    detail: format!("esperados {SEED_LEN} bytes, obtidos {}", seed_hex.len() / 2),
                })?;
        let signing_key = SigningKey::from_bytes(&seed);
        Ok(Self {
            path: Some(path),
            node_id: node_id_for(&signing_key.verifying_key().to_bytes()),
            signing_key,
        })
    }

    /// Identidade **somente em memória**, para teste. Não toca em disco.
    ///
    /// A semente vem do RNG do sistema; o chamador não escolhe o byte secreto.
    ///
    /// # Errors
    /// [`IdentityError::RandomnessUnavailable`] sem entropia.
    pub fn generate_in_memory() -> Result<Self, IdentityError> {
        let mut seed = [0_u8; SEED_LEN];
        getrandom::fill(&mut seed)
            .map_err(|e| IdentityError::RandomnessUnavailable(e.to_string()))?;
        let signing_key = SigningKey::from_bytes(&seed);
        Ok(Self {
            path: None,
            node_id: node_id_for(&signing_key.verifying_key().to_bytes()),
            signing_key,
        })
    }

    /// `node_id` público (`gmn_` + hex de `sha256(pubkey)`).
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Chave pública Ed25519 em hex minúsculo.
    #[must_use]
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_bytes())
    }

    /// Chave pública Ed25519 crua (32 bytes).
    ///
    /// Usada pelo transporte: a chave do certificado TLS é comparada byte a
    /// byte com a chave do manifesto, sem passar por hex.
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; PUBLIC_KEY_LEN] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Chave privada em PKCS#8 DER, para o certificado autoemitido do transporte
    /// usar **a mesma** chave da identidade.
    ///
    /// O DER nunca é gravado nem logado: existe só para `rcgen` emitir o
    /// certificado em memória. Nenhum `Debug` desta estrutura o mostra.
    ///
    /// # Errors
    /// [`IdentityError::KeyExport`] se a serialização falhar.
    pub fn pkcs8_der(&self) -> Result<Vec<u8>, IdentityError> {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        self.signing_key
            .to_pkcs8_der()
            .map(|doc| doc.as_bytes().to_vec())
            .map_err(|e| IdentityError::KeyExport(e.to_string()))
    }

    /// Arquivo da identidade, quando ela veio de disco.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Assina `bytes`.
    #[must_use]
    pub fn sign(&self, bytes: &[u8]) -> Signature {
        self.signing_key.sign(bytes)
    }

    /// Assina `bytes` e devolve a assinatura em hex minúsculo.
    #[must_use]
    pub fn sign_hex(&self, bytes: &[u8]) -> String {
        hex::encode(self.sign(bytes).to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("godmesh-identity-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir temporário");
        dir
    }

    #[test]
    fn cria_com_permissao_600_e_recarrega_a_mesma_chave() {
        let dir = temp_dir("perm");
        let path = dir.join("node.key");
        let first = NodeIdentity::load_or_create(&path).expect("cria identidade");
        let mode = fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(
            mode, KEY_FILE_MODE,
            "arquivo de chave com permissão {mode:o}"
        );

        let second = NodeIdentity::load(&path).expect("recarrega identidade");
        assert_eq!(first.node_id(), second.node_id());
        assert_eq!(first.public_key_hex(), second.public_key_hex());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn arquivo_mais_aberto_que_600_e_recusado() {
        let dir = temp_dir("aberto");
        let path = dir.join("node.key");
        NodeIdentity::load_or_create(&path).expect("cria identidade");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        let e = NodeIdentity::load(&path).expect_err("permissão aberta recusada");
        assert!(
            matches!(e, IdentityError::InsecureMode { mode: 0o644, .. }),
            "{e}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn debug_nao_expoe_a_chave_privada() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade em memória");
        let text = format!("{identity:?}");
        assert!(text.contains(identity.node_id()), "{text}");
        assert!(text.contains(&identity.public_key_hex()), "{text}");
        // A semente nunca sai: nenhuma representação do Debug a contém.
        let seed = identity.signing_key.to_bytes();
        let seed_hex = hex::encode(seed);
        assert!(!text.contains(&seed_hex), "Debug expôs a semente: {text}");
        assert!(!text.to_lowercase().contains("secret"), "{text}");
        assert!(!text.contains(&hex::encode(&seed[..16])), "{text}");
    }

    #[test]
    fn assinatura_verifica_e_bytes_alterados_nao() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let signature = identity.sign_hex(b"manifesto");
        verify_signature(&identity.public_key_hex(), b"manifesto", &signature)
            .expect("assinatura válida");
        assert!(matches!(
            verify_signature(&identity.public_key_hex(), b"manifesto ", &signature),
            Err(IdentityError::SignatureInvalid)
        ));
    }

    #[test]
    fn node_id_e_derivado_da_chave_publica() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        assert!(is_node_id(identity.node_id()));
        assert!(verify_node_id(
            identity.node_id(),
            &identity.public_key_hex()
        ));

        let other = NodeIdentity::generate_in_memory().expect("outra identidade");
        assert!(!verify_node_id(identity.node_id(), &other.public_key_hex()));
        assert!(!is_node_id("gmn_curto"));
        assert!(!is_node_id("xyz"));
    }

    #[test]
    fn chave_publica_crua_e_a_mesma_do_public_key_hex() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        assert_eq!(identity.public_key_hex(), hex::encode(identity.public_key_bytes()));
        assert_eq!(identity.public_key_bytes().len(), PUBLIC_KEY_LEN);
    }

    #[test]
    fn pkcs8_exporta_a_mesma_chave_da_identidade() {
        // O certificado TLS tem que provar posse da chave da identidade; se o
        // export divergisse, o handshake recusaria o próprio nó.
        use ed25519_dalek::pkcs8::DecodePrivateKey;
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let der = identity.pkcs8_der().expect("export pkcs8");
        let reloaded = SigningKey::from_pkcs8_der(&der).expect("recarrega pkcs8");
        assert_eq!(reloaded.verifying_key().to_bytes(), identity.public_key_bytes());
        // O DER carrega a semente: nunca pode aparecer no Debug.
        assert!(!format!("{identity:?}").contains(&hex::encode(&der)));
    }
}
