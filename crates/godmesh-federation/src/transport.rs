//! Transporte QUIC com mTLS — ADR-071 §(2) e ADR-072 §5.
//!
//! Decisões que este módulo materializa, e por quê:
//!
//! - **QUIC + TLS 1.3** (`quinn` sobre `rustls`). TLS 1.2 não é oferecido: a
//!   malha é nova, não tem legado a acomodar, e TLS 1.2 traria mais superfície.
//! - **ALPN fixo e versionado** (`godmesh/1`). Versão de protocolo é recusada,
//!   não negociada por tentativa.
//! - **mTLS obrigatório**: o servidor exige certificado de cliente
//!   (`client_auth_mandatory`) e o cliente valida o certificado do servidor.
//! - **Certificado autoemitido por nó** (`rcgen`), assinado com a **mesma** chave
//!   Ed25519 da identidade. Ele não é credencial de CA: serve para provar posse
//!   da chave privada no handshake TLS (o `CertificateVerify`).
//! - **Validação por chave registrada**: o verificador extrai o
//!   `SubjectPublicKeyInfo` do certificado e exige que a chave esteja na
//!   allowlist. Nome (CN/SAN), cadeia, data de validade e CA pública são
//!   **ignorados de propósito** — ADR-071: endereço e nome não são permissão,
//!   chave é.
//!
//! A verificação de assinatura do handshake TLS continua sendo a da biblioteca
//! (`rustls::crypto::verify_tls13_signature` com o provider `ring`): nenhuma
//! primitiva criptográfica é implementada aqui (ADR-026/ADR-031).

use std::fmt;
use std::sync::Arc;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Connection, Endpoint, ServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{WebPkiSupportedAlgorithms, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    CertificateError, DigitallySignedStruct, DistinguishedName, Error, OtherError, SignatureScheme,
};

use crate::identity::{NodeIdentity, PUBLIC_KEY_LEN};
use crate::peer::PeerRegistry;

/// ALPN desta versão do protocolo de federação.
pub const ALPN_V1: &[u8] = b"godmesh/1";
/// O mesmo ALPN em texto, para comparação com o manifesto assinado.
pub const ALPN_V1_TEXT: &str = "godmesh/1";
/// ALPN do listener HTTPS dedicado para configuração.
pub const ALPN_HTTP1: &[u8] = b"http/1.1";

/// Nome de servidor apresentado pelo cliente.
///
/// É **ignorado** pelo verificador (e documentado como tal): sem CA e sem DNS na
/// cadeia de confiança, o campo não carrega autoridade. Existe porque
/// `rustls`/`quinn` exigem um valor não vazio na abertura da conexão.
pub const CONNECT_SERVER_NAME: &str = "godmesh-peer.invalid";

/// OID de Ed25519 (1.3.101.112) em bytes DER.
const ED25519_OID: &[u8] = &[0x2b, 0x65, 0x70];

/// Falha ao preparar material de TLS ou endpoint.
#[derive(Debug)]
pub enum TransportError {
    /// A chave da identidade não pôde ser exportada para o certificado.
    Identity(String),
    /// `rcgen` recusou construir o certificado autoemitido.
    Certificate(String),
    /// `rustls` recusou a configuração (versão de protocolo, cert, chave).
    Tls(String),
    /// `quinn` recusou a configuração (suíte inicial indisponível).
    Quic(String),
    /// O socket do endpoint não pôde ser aberto.
    Io(String),
    /// O ALPN efetivo do handshake não é o esperado.
    AlpnMismatch(Option<String>),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(e) => write!(f, "identidade inválida: {e}"),
            Self::Certificate(e) => write!(f, "certificado autoemitido recusado: {e}"),
            Self::Tls(e) => write!(f, "configuração TLS recusada: {e}"),
            Self::Quic(e) => write!(f, "configuração QUIC recusada: {e}"),
            Self::Io(e) => write!(f, "socket do endpoint: {e}"),
            Self::AlpnMismatch(got) => write!(f, "ALPN efetivo inesperado: {got:?}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Material TLS de um nó: certificado autoemitido + chave privada.
pub struct NodeTls {
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl fmt::Debug for NodeTls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A chave privada nunca entra em log.
        f.debug_struct("NodeTls")
            .field("certs", &self.certs.len())
            .field("key", &"<redigido>")
            .finish()
    }
}

impl NodeTls {
    /// Emite o certificado autoemitido do nó com a chave da própria identidade.
    ///
    /// # Errors
    /// Devolve erro se a chave não puder ser exportada ou se o certificado não
    /// puder ser construído; nunca se segue com material incompleto.
    pub fn self_issued(identity: &NodeIdentity) -> Result<Self, TransportError> {
        let pkcs8 = identity
            .pkcs8_der()
            .map_err(|e| TransportError::Identity(e.to_string()))?;
        let key_pair = rcgen::KeyPair::from_der_and_sign_algo(
            &PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8.clone())),
            &rcgen::PKCS_ED25519,
        )
        .map_err(|e| TransportError::Certificate(e.to_string()))?;
        // SAN é cosmético: nenhum verificador desta malha lê nome. O CN fica
        // vazio para não sugerir autoridade por nome.
        let mut params = rcgen::CertificateParams::new(vec![CONNECT_SERVER_NAME.to_owned()])
            .map_err(|e| TransportError::Certificate(e.to_string()))?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| TransportError::Certificate(e.to_string()))?;
        Ok(Self {
            certs: vec![CertificateDer::from(cert.der().to_vec())],
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8)),
        })
    }
}

/// Configuração QUIC de **servidor**: exige certificado de cliente e valida a
/// chave apresentada contra o registro de pares.
///
/// # Errors
/// Erro de `rustls`/`quinn` se a configuração for inconsistente.
pub fn server_config(
    registry: Arc<PeerRegistry>,
    tls: &NodeTls,
) -> Result<ServerConfig, TransportError> {
    let mut config = rustls_server(tls, AcceptedKeys::Registry(registry))?;
    config.alpn_protocols = vec![ALPN_V1.to_vec()];
    let quic =
        QuicServerConfig::try_from(config).map_err(|e| TransportError::Quic(e.to_string()))?;
    let mut server_config = ServerConfig::with_crypto(Arc::new(quic));
    server_config.transport_config(Arc::new(transport_config()));
    Ok(server_config)
}

/// Configuração QUIC de cliente com a allowlist dinâmica do registro.
///
/// # Errors
/// Erro de `rustls` ou `quinn` quando a configuração é inconsistente.
pub fn client_config(
    registry: Arc<PeerRegistry>,
    tls: &NodeTls,
) -> Result<quinn::ClientConfig, TransportError> {
    let mut config = rustls_client(tls, AcceptedKeys::Registry(registry))?;
    config.alpn_protocols = vec![ALPN_V1.to_vec()];
    let quic =
        QuicClientConfig::try_from(config).map_err(|e| TransportError::Quic(e.to_string()))?;
    Ok(quinn::ClientConfig::new(Arc::new(quic)))
}

/// Configuração mTLS HTTP/1.1 de servidor com pins fixos de clientes.
/// Usa TLS 1.3 sem retomada nem dados antecipados.
///
/// # Errors
/// Erro de `rustls` quando a configuração é inconsistente.
pub fn http1_server_tls(
    tls: &NodeTls,
    client_pins: Vec<[u8; PUBLIC_KEY_LEN]>,
) -> Result<rustls::ServerConfig, TransportError> {
    let mut config = rustls_server(tls, AcceptedKeys::Pins(client_pins))?;
    config.alpn_protocols = vec![ALPN_HTTP1.to_vec()];
    config.max_early_data_size = 0;
    config.send_tls13_tickets = 0;
    config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    Ok(config)
}

/// Configuração mTLS HTTP/1.1 de cliente com um único pin do servidor.
///
/// # Errors
/// Erro de `rustls` quando a configuração é inconsistente.
pub fn http1_client_tls(
    tls: &NodeTls,
    server_pin: [u8; PUBLIC_KEY_LEN],
) -> Result<rustls::ClientConfig, TransportError> {
    let mut config = rustls_client(tls, AcceptedKeys::Pins(vec![server_pin]))?;
    config.alpn_protocols = vec![ALPN_HTTP1.to_vec()];
    config.resumption = rustls::client::Resumption::disabled();
    config.enable_early_data = false;
    Ok(config)
}

fn rustls_server(
    tls: &NodeTls,
    keys: AcceptedKeys,
) -> Result<rustls::ServerConfig, TransportError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(KeyAllowlist::new(keys, &provider));
    rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| TransportError::Tls(e.to_string()))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(tls.certs.clone(), tls.key.clone_key())
        .map_err(|e| TransportError::Tls(e.to_string()))
}

fn rustls_client(
    tls: &NodeTls,
    keys: AcceptedKeys,
) -> Result<rustls::ClientConfig, TransportError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(KeyAllowlist::new(keys, &provider));
    rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| TransportError::Tls(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(tls.certs.clone(), tls.key.clone_key())
        .map_err(|e| TransportError::Tls(e.to_string()))
}

/// Transporte: keep-alive do QUIC ligado (o heartbeat é da aplicação; isto evita
/// path congelado em silêncio) e idle timeout declarado.
fn transport_config() -> quinn::TransportConfig {
    let mut transport = quinn::TransportConfig::default();
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(2)));
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(15)).unwrap_or_default(),
    ));
    transport
}

/// Abre um endpoint de servidor escutando em `addr`.
///
/// # Errors
/// Erro se o socket não puder ser aberto.
pub fn bind_server(addr: std::net::SocketAddr, config: ServerConfig) -> std::io::Result<Endpoint> {
    Endpoint::server(config, addr)
}

/// Abre um endpoint de cliente (sem configuração padrão; ela vai por conexão,
/// para o registro poder mudar sem recriar o socket).
///
/// # Errors
/// Erro se o socket não puder ser aberto.
pub fn bind_client(addr: std::net::SocketAddr) -> std::io::Result<Endpoint> {
    Endpoint::client(addr)
}

/// Chave pública Ed25519 apresentada pelo par no certificado TLS.
///
/// Lida do `SubjectPublicKeyInfo` do certificado de **folha**; a cadeia é
/// ignorada (não existe CA nesta malha).
#[must_use]
pub fn peer_public_key(connection: &Connection) -> Option<[u8; PUBLIC_KEY_LEN]> {
    let identity = connection.peer_identity()?;
    let certs = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    certificate_public_key(certs.first()?)
}

/// ALPN efetivamente negociado na conexão.
#[must_use]
pub fn negotiated_alpn(connection: &Connection) -> Option<String> {
    let data = connection.handshake_data()?;
    let data = data
        .downcast::<quinn::crypto::rustls::HandshakeData>()
        .ok()?;
    data.protocol
        .as_deref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map(str::to_owned)
}

/// Verifica que o ALPN negociado é o desta versão.
///
/// # Errors
/// [`TransportError::AlpnMismatch`] quando o ALPN não é `godmesh/1`.
pub fn require_alpn(connection: &Connection) -> Result<String, TransportError> {
    let alpn = negotiated_alpn(connection);
    match alpn.as_deref() {
        Some(ALPN_V1_TEXT) => Ok(ALPN_V1_TEXT.to_owned()),
        _ => Err(TransportError::AlpnMismatch(alpn)),
    }
}

/// Extrai a chave Ed25519 do SPKI do certificado.
///
/// Só o `SubjectPublicKeyInfo` importa. Recusa qualquer outra curva/algoritmo em
/// vez de aceitar "o que der".
#[must_use]
pub fn certificate_public_key(cert: &CertificateDer<'_>) -> Option<[u8; PUBLIC_KEY_LEN]> {
    let (_, parsed) = x509_parser::parse_x509_certificate(cert.as_ref()).ok()?;
    let spki = &parsed.tbs_certificate.subject_pki;
    if spki.algorithm.algorithm.as_bytes() != ED25519_OID {
        return None;
    }
    spki.subject_public_key.data.as_ref().try_into().ok()
}

fn reject(reason: &'static str) -> Error {
    Error::InvalidCertificate(CertificateError::Other(OtherError(Arc::new(
        std::io::Error::other(reason),
    ))))
}

enum AcceptedKeys {
    Registry(Arc<PeerRegistry>),
    Pins(Vec<[u8; PUBLIC_KEY_LEN]>),
}

/// Verificador de certificado nos dois sentidos, por **allowlist de chaves**.
///
/// Implementa `ServerCertVerifier` (cliente valida servidor) e
/// `ClientCertVerifier` (servidor valida cliente) com a mesma regra: a chave
/// pública do certificado precisa estar registrada. Não há exceção por nome,
/// IP, validade ou CA — e a API `dangerous`/`danger` do `rustls` é usada de
/// propósito, porque o padrão dele é validação por cadeia de CA, que este
/// desenho não usa.
struct KeyAllowlist {
    keys: AcceptedKeys,
    supported: WebPkiSupportedAlgorithms,
}

impl fmt::Debug for KeyAllowlist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyAllowlist").finish_non_exhaustive()
    }
}

impl KeyAllowlist {
    fn new(keys: AcceptedKeys, provider: &rustls::crypto::CryptoProvider) -> Self {
        Self {
            keys,
            supported: provider.signature_verification_algorithms,
        }
    }

    /// Regra única dos dois sentidos.
    fn accepts(&self, cert: &CertificateDer<'_>) -> Result<(), Error> {
        let key = certificate_public_key(cert)
            .ok_or_else(|| reject("certificado de par sem chave Ed25519"))?;
        let pinned = match &self.keys {
            AcceptedKeys::Registry(registry) => registry.knows_key(&key),
            AcceptedKeys::Pins(pins) => pins.contains(&key),
        };
        if !pinned {
            return Err(reject("chave pública do par não registrada"));
        }
        Ok(())
    }
}

impl ServerCertVerifier for KeyAllowlist {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        self.accepts(end_entity)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        // TLS 1.2 não é oferecido por esta configuração; chegar aqui significa que
        // as versões foram violadas: nega.
        Err(reject("TLS 1.2 não é oferecido na malha"))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        // Verificação de assinatura é da biblioteca (`ring`), não deste crate.
        verify_tls13_signature(message, cert, dss, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Só Ed25519: a identidade de nó é Ed25519 e nada mais é aceito.
        vec![SignatureScheme::ED25519]
    }
}

impl ClientCertVerifier for KeyAllowlist {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // Sem CA, sem sugestão de emitente: o cliente deve apresentar o próprio
        // certificado.
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        self.accepts(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Err(reject("TLS 1.2 não é oferecido na malha"))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NodeIdentity;
    use crate::peer::PeerRecord;

    #[test]
    fn certificate_key_is_the_identity_key() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let tls = NodeTls::self_issued(&identity).expect("certificado");
        let key = certificate_public_key(&tls.certs[0]).expect("spki legível");
        assert_eq!(key, identity.public_key_bytes());
    }

    #[test]
    fn allowlist_refuses_unregistered_key_and_accepts_registered() {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let tls = NodeTls::self_issued(&identity).expect("certificado");
        let provider = rustls::crypto::ring::default_provider();

        let empty = Arc::new(PeerRegistry::new());
        let verifier = KeyAllowlist::new(AcceptedKeys::Registry(Arc::clone(&empty)), &provider);
        assert!(verifier.accepts(&tls.certs[0]).is_err());

        let registry = Arc::new(PeerRegistry::new());
        registry.insert(PeerRecord::authorized_from_key(
            &identity.public_key_bytes(),
            "br-south",
        ));
        let verifier = KeyAllowlist::new(AcceptedKeys::Registry(Arc::clone(&registry)), &provider);
        assert!(verifier.accepts(&tls.certs[0]).is_ok());
    }

    #[test]
    fn non_ed25519_certificate_is_refused() {
        // Byte qualquer não é certificado: recusa antes de qualquer decisão.
        let garbage = CertificateDer::from(vec![0u8; 64]);
        let registry = Arc::new(PeerRegistry::new());
        let provider = rustls::crypto::ring::default_provider();
        let verifier = KeyAllowlist::new(AcceptedKeys::Registry(registry), &provider);
        assert!(verifier.accepts(&garbage).is_err());
    }

    #[test]
    fn only_ed25519_is_advertised() {
        let registry = Arc::new(PeerRegistry::new());
        let provider = rustls::crypto::ring::default_provider();
        let verifier = KeyAllowlist::new(AcceptedKeys::Registry(registry), &provider);
        assert_eq!(
            <KeyAllowlist as ServerCertVerifier>::supported_verify_schemes(&verifier),
            vec![SignatureScheme::ED25519]
        );
        assert_eq!(
            <KeyAllowlist as ClientCertVerifier>::supported_verify_schemes(&verifier),
            vec![SignatureScheme::ED25519]
        );
    }
}
