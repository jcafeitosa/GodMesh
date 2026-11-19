//! Handshake de par — ADR-071 §(1)/(4) e emenda §3.
//!
//! Um stream bidirecional dedicado por conexão carrega o `SignedManifest` de
//! **cada lado**: o iniciador abre o stream, escreve o próprio manifesto e fecha
//! a direção de escrita; o aceitador lê, verifica, e responde com o seu no mesmo
//! stream. Não existe "confiar no primeiro que falou".
//!
//! A verificação é a mesma nos dois sentidos e nesta ordem (falha = fecha):
//!
//! 1. frame bem formado e dentro do teto (`MAX_MANIFEST_FRAME_BYTES`);
//! 2. campos obrigatórios presentes e assinatura Ed25519 conferindo com a chave
//!    declarada (`SignedManifest::verify_fresh`, forma canônica de M1);
//! 3. `node_id` == `gmn_` + `sha256(chave declarada)` — a ligação é recomputada,
//!    não aceita;
//! 4. `cluster` do manifesto igual ao local **e** ao registrado;
//! 5. `issued_at` dentro da janela declarada (replay de manifesto antigo cai);
//! 6. ALPN negociado é o desta versão e é o mesmo que o manifesto declarou;
//! 7. chave do manifesto == chave do certificado TLS apresentado (senão um nó
//!    autorizado poderia se passar por outro no registro);
//! 8. no modo legado, chave registrada no registro de pares, com estado `authorized`;
//! 9. no modo legado, `epoch` do manifesto não é anterior ao epoch registrado.
//!
//! O `trust_state` **não** vem do manifesto: um nó não declara a própria
//! autorização. No modo legado quem decide é o `PeerRegistry`. No modo
//! operacional o nó consulta a autoridade injetada usando as chaves do mTLS.
//!
//! Motivo de fechamento é sempre uma string fixa deste crate — nunca dado do
//! par — para que log e alerta não carreguem conteúdo controlado pelo outro lado.

use std::fmt;
use std::sync::Arc;

use quinn::{Connection, ConnectionError, RecvStream, SendStream};

use crate::identity::{NodeIdentity, parse_public_key};
use crate::manifest::{MAX_MANIFEST_FRAME_BYTES, ManifestError, SignedManifest};
use crate::node::FederationConfig;
use crate::peer::{PeerRecord, PeerRegistry, TrustState};
use crate::transport;

/// Motivo de recusa do handshake, com código de fechamento QUIC estável.
///
/// A faixa `0x474D00xx` ("GM") é reservada desta fatia; código novo entra no fim
/// e código existente não muda de significado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Frame ilegível, grande demais ou fora do formato desta versão.
    BadFrame,
    /// Assinatura do manifesto não confere com a chave declarada.
    SignatureInvalid,
    /// `node_id` não deriva da chave pública declarada.
    NodeIdMismatch,
    /// A chave do manifesto não é a chave do certificado TLS (tentativa de se
    /// passar por outro nó).
    KeyMismatch,
    /// Chave não registrada no registro de pares.
    UnknownKey,
    /// Par em `pending` (ou `unknown`): apresenta manifesto e pede ingresso, mas
    /// não entra.
    NotAuthorized,
    /// Par revogado: não reconecta.
    Revoked,
    /// Cluster diferente do local (ou do registrado).
    ClusterMismatch,
    /// `issued_at` fora da janela aceita (replay de manifesto antigo).
    StaleManifest,
    /// `epoch` do manifesto é anterior ao epoch registrado (revogação já vista).
    StaleEpoch,
    /// ALPN diferente do fixo desta versão, ou divergente do declarado.
    AlpnMismatch,
    /// Falha de I/O/stream: conexão não chegou ao fim do handshake.
    Transport,
}

impl Rejection {
    /// Código de fechamento QUIC (`CONNECTION_CLOSE`).
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::BadFrame => 0x474D_0001,
            Self::SignatureInvalid => 0x474D_0002,
            Self::NodeIdMismatch => 0x474D_0003,
            Self::KeyMismatch => 0x474D_0004,
            Self::UnknownKey => 0x474D_0005,
            Self::NotAuthorized => 0x474D_0006,
            Self::Revoked => 0x474D_0007,
            Self::ClusterMismatch => 0x474D_0008,
            Self::StaleManifest => 0x474D_0009,
            Self::StaleEpoch => 0x474D_000A,
            Self::AlpnMismatch => 0x474D_000B,
            Self::Transport => 0x474D_000C,
        }
    }

    /// Reconstrói o motivo a partir do código de fechamento informado pelo par.
    ///
    /// Sem isso, um nó que recusa fecharia a conexão e o outro lado reportaria
    /// "frame ilegível" — o erro de quem *recebe* a recusa, não o motivo dela.
    #[must_use]
    pub const fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            0x474D_0001 => Self::BadFrame,
            0x474D_0002 => Self::SignatureInvalid,
            0x474D_0003 => Self::NodeIdMismatch,
            0x474D_0004 => Self::KeyMismatch,
            0x474D_0005 => Self::UnknownKey,
            0x474D_0006 => Self::NotAuthorized,
            0x474D_0007 => Self::Revoked,
            0x474D_0008 => Self::ClusterMismatch,
            0x474D_0009 => Self::StaleManifest,
            0x474D_000A => Self::StaleEpoch,
            0x474D_000B => Self::AlpnMismatch,
            0x474D_000C => Self::Transport,
            _ => return None,
        })
    }

    /// Motivo sanitizado (ASCII fixo, sem interpolação de dado do par).
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::BadFrame => "manifesto ilegivel",
            Self::SignatureInvalid => "assinatura invalida",
            Self::NodeIdMismatch => "node_id nao corresponde a chave",
            Self::KeyMismatch => "chave do manifesto difere da chave TLS",
            Self::UnknownKey => "chave nao registrada",
            Self::NotAuthorized => "par pendente de autorizacao",
            Self::Revoked => "par revogado",
            Self::ClusterMismatch => "cluster diferente",
            Self::StaleManifest => "manifesto fora da janela",
            Self::StaleEpoch => "epoch do manifesto obsoleto",
            Self::AlpnMismatch => "alpn diferente",
            Self::Transport => "handshake incompleto",
        }
    }
}

/// Par que completou o handshake com sucesso.
///
/// `node_id` é o `gmn_` canônico de M1 (o mesmo que o banco guarda), não um
/// identificador paralelo.
#[derive(Debug, Clone)]
pub struct AcceptedPeer {
    /// Identidade verificada do par.
    pub node_id: String,
    /// Versão de software declarada no manifesto assinado.
    pub software_version: String,
    /// ALPN verificado (o desta versão).
    pub alpn: String,
    /// Segundo Unix de `issued_at` do manifesto aceito.
    pub issued_at: i64,
}

/// Handshake recusado. Carrega só o motivo sanitizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeError {
    /// Motivo (e código) da recusa.
    pub rejection: Rejection,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "handshake recusado: {} ({})",
            self.rejection.reason(),
            self.rejection.code()
        )
    }
}

impl std::error::Error for HandshakeError {}

/// Lado **iniciador**: abre o stream de handshake, envia o próprio manifesto e
/// verifica o do servidor.
///
/// # Errors
/// [`HandshakeError`] com o motivo; a conexão é fechada com o código
/// correspondente antes de retornar.
pub async fn initiate(
    connection: &Connection,
    identity: &NodeIdentity,
    config: &FederationConfig,
    registry: &Arc<PeerRegistry>,
) -> Result<AcceptedPeer, HandshakeError> {
    initiate_with_registry(connection, identity, config, Some(registry)).await
}

pub(crate) async fn initiate_with_registry(
    connection: &Connection,
    identity: &NodeIdentity,
    config: &FederationConfig,
    registry: Option<&PeerRegistry>,
) -> Result<AcceptedPeer, HandshakeError> {
    initiate_with_epoch(connection, identity, config, registry)
        .await
        .map(|(peer, _)| peer)
}

pub(crate) async fn initiate_with_epoch(
    connection: &Connection,
    identity: &NodeIdentity,
    config: &FederationConfig,
    registry: Option<&PeerRegistry>,
) -> Result<(AcceptedPeer, u64), HandshakeError> {
    let result = async {
        let ours = config.signed_manifest(identity);
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|_| Rejection::Transport)?;
        send_manifest(&mut send, &ours).await?;
        let theirs = read_manifest(connection, &mut recv).await?;
        verify_peer(connection, &theirs, config, registry).map(|peer| (peer, theirs.epoch))
    };
    finish(connection, result.await)
}

/// Lado **aceitador**: lê o manifesto do cliente, verifica, e só então envia o
/// seu. O servidor não se apresenta a quem não passou na verificação.
///
/// # Errors
/// [`HandshakeError`] com o motivo; a conexão é fechada com o código
/// correspondente antes de retornar.
pub async fn accept(
    connection: &Connection,
    identity: &NodeIdentity,
    config: &FederationConfig,
    registry: &Arc<PeerRegistry>,
) -> Result<AcceptedPeer, HandshakeError> {
    accept_with_registry(connection, identity, config, Some(registry)).await
}

pub(crate) async fn accept_with_registry(
    connection: &Connection,
    identity: &NodeIdentity,
    config: &FederationConfig,
    registry: Option<&PeerRegistry>,
) -> Result<AcceptedPeer, HandshakeError> {
    let result = async {
        let ours = config.signed_manifest(identity);
        let (mut send, mut recv) = connection
            .accept_bi()
            .await
            .map_err(|_| Rejection::Transport)?;
        let theirs = read_manifest(connection, &mut recv).await?;
        let peer = verify_peer(connection, &theirs, config, registry)?;
        send_manifest(&mut send, &ours).await?;
        Ok(peer)
    };
    finish(connection, result.await)
}

fn finish<T>(connection: &Connection, result: Result<T, Rejection>) -> Result<T, HandshakeError> {
    match result {
        Ok(peer) => Ok(peer),
        Err(rejection) => {
            // Fecha com código e motivo sanitizado: o par aprende *por quê* (sem
            // receber dado nosso) e nenhum stream de dados é aberto. Se o par já
            // fechou, não se sobrescreve o motivo dele.
            if connection.close_reason().is_none() {
                connection.close(rejection.code().into(), rejection.reason().as_bytes());
            }
            Err(HandshakeError { rejection })
        }
    }
}

async fn send_manifest(send: &mut SendStream, ours: &SignedManifest) -> Result<(), Rejection> {
    let frame = ours.to_frame().map_err(map_manifest_error)?;
    send.write_all(&frame)
        .await
        .map_err(|_| Rejection::Transport)?;
    send.finish().map_err(|_| Rejection::Transport)
}

async fn read_manifest(
    connection: &Connection,
    recv: &mut RecvStream,
) -> Result<SignedManifest, Rejection> {
    match recv.read_to_end(MAX_MANIFEST_FRAME_BYTES).await {
        Ok(bytes) => SignedManifest::from_frame(&bytes).map_err(map_manifest_error),
        // Stream morreu: se o par fechou com veredito de aplicação, o motivo é
        // dele; senão é falha de transporte (TLS/alerta), não "frame ruim".
        Err(_) => Err(peer_verdict(connection).unwrap_or(Rejection::Transport)),
    }
}

/// Veredito de aplicação que o par enviou no fechamento (`CONNECTION_CLOSE`).
pub(crate) fn peer_verdict(connection: &Connection) -> Option<Rejection> {
    match connection.close_reason()? {
        ConnectionError::ApplicationClosed(close) => u32::try_from(u64::from(close.error_code))
            .ok()
            .and_then(Rejection::from_code),
        _ => None,
    }
}

/// Verificação do manifesto do par (mesma função nos dois sentidos).
fn verify_peer(
    connection: &Connection,
    theirs: &SignedManifest,
    config: &FederationConfig,
    registry: Option<&PeerRegistry>,
) -> Result<AcceptedPeer, Rejection> {
    // Forma canônica de M1: campos, ligação `node_id`↔chave, assinatura e janela
    // de `issued_at`. Um só caminho de verificação para o ingresso e o
    // transporte.
    theirs
        .verify_fresh(crate::now_unix(), config.manifest_window_seconds)
        .map_err(map_manifest_error)?;

    if theirs.cluster != config.cluster {
        return Err(Rejection::ClusterMismatch);
    }

    // ALPN: o negociado no TLS tem que ser o desta versão **e** o declarado no
    // conteúdo assinado. Versão de protocolo não é adivinhada nem divergente.
    let alpn = transport::require_alpn(connection).map_err(|_| Rejection::AlpnMismatch)?;
    if theirs.alpn != alpn {
        return Err(Rejection::AlpnMismatch);
    }

    // A chave do manifesto tem que ser a chave que o TLS provou possuir.
    let manifest_key =
        parse_public_key(&theirs.public_key).map_err(|_| Rejection::SignatureInvalid)?;
    let tls_key = transport::peer_public_key(connection).ok_or(Rejection::KeyMismatch)?;
    if tls_key != manifest_key.to_bytes() {
        return Err(Rejection::KeyMismatch);
    }

    // A allowlist continua autenticando o candidato no TLS. Com autorizador,
    // a autoridade operacional vem da consulta durável do nó.
    if let Some(registry) = registry {
        registry.with_record(&theirs.node_id, |record| {
            verify_registry_record(record, &theirs.cluster, tls_key, theirs.epoch)
        })?;
    }

    Ok(AcceptedPeer {
        node_id: theirs.node_id.clone(),
        software_version: theirs.software_version.clone(),
        alpn,
        issued_at: theirs.issued_at,
    })
}

pub(crate) fn verify_registry_record(
    record: Option<&PeerRecord>,
    cluster: &str,
    key: [u8; 32],
    epoch: u64,
) -> Result<(), Rejection> {
    let record = record.ok_or(Rejection::UnknownKey)?;
    if record.public_key_bytes().ok() != Some(key) {
        return Err(Rejection::UnknownKey);
    }
    if record.cluster != cluster {
        return Err(Rejection::ClusterMismatch);
    }
    match record.trust_state {
        TrustState::Authorized => {}
        TrustState::Revoked => return Err(Rejection::Revoked),
        TrustState::Pending | TrustState::Unknown => return Err(Rejection::NotAuthorized),
    }
    if epoch < record.epoch {
        return Err(Rejection::StaleEpoch);
    }
    Ok(())
}

fn map_manifest_error(error: ManifestError) -> Rejection {
    match error {
        ManifestError::SignatureInvalid => Rejection::SignatureInvalid,
        ManifestError::NodeIdMismatch => Rejection::NodeIdMismatch,
        ManifestError::Expired { .. } | ManifestError::NotYetValid { .. } => {
            Rejection::StaleManifest
        }
        ManifestError::EmptyField(_)
        | ManifestError::FieldTooLong { .. }
        | ManifestError::TooManyItems { .. }
        | ManifestError::InvalidNodeId(_)
        | ManifestError::InvalidPublicKey(_)
        | ManifestError::InvalidSignatureEncoding(_)
        | ManifestError::FrameTooLarge { .. }
        | ManifestError::FrameMalformed(_) => Rejection::BadFrame,
    }
}
