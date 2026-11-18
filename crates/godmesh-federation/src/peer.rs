//! Registro de par e máquina de estados de confiança.
//!
//! `trust_state` é a única coisa que decide se um nó pode receber dado privado.
//! Por isso as transições são explícitas e fechadas:
//!
//! ```text
//! unknown  → pending
//! pending  → authorized            (aprovação administrativa)
//! pending  → revoked               (recusa depois de registrar)
//! authorized → revoked             (revogação)
//! revoked  → pending               (novo pedido; exige nova aprovação)
//! X → X                            (idempotência: gravar o mesmo estado é no-op)
//! ```
//!
//! O que **não** existe: voltar de `revoked` direto para `authorized`, pular de
//! `pending` para `authorized` por qualquer caminho que não seja a aprovação, ou
//! cair para `unknown` (estado inicial, não destino).

use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::identity::{self, PUBLIC_KEY_LEN};
use crate::manifest::SignedManifest;

/// Estado de confiança de um par.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    /// Pedido registrado, aguardando decisão administrativa.
    Pending,
    /// Aprovado: pode receber o escopo autorizado.
    Authorized,
    /// Revogado: conexões e leases do nó deixam de valer.
    Revoked,
    /// Sem decisão registrada (estado inicial de um registro criado fora do fluxo).
    Unknown,
}

impl TrustState {
    /// Nome estável no contrato JSON e no banco (`CHECK IN (...)`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Authorized => "authorized",
            Self::Revoked => "revoked",
            Self::Unknown => "unknown",
        }
    }

    /// Resolve o nome estável. Desconhecido é `None` e quem lê do banco trata
    /// isso como erro, nunca como "autorizado".
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "pending" => Some(Self::Pending),
            "authorized" => Some(Self::Authorized),
            "revoked" => Some(Self::Revoked),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Se a transição `self → next` é permitida.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        if matches!(
            (self, next),
            (Self::Pending, Self::Pending) | (Self::Authorized, Self::Authorized)
        ) {
            return true;
        }
        matches!(
            (self, next),
            (Self::Unknown, Self::Pending)
                | (Self::Pending, Self::Authorized)
                | (Self::Pending, Self::Revoked)
                | (Self::Authorized, Self::Revoked)
                | (Self::Revoked, Self::Pending)
        )
    }
}

impl fmt::Display for TrustState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Transição de confiança recusada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustTransitionError {
    /// Estado persistido.
    pub current: TrustState,
    /// Estado pretendido.
    pub next: TrustState,
}

impl fmt::Display for TrustTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "transição de confiança ilegal: {} → {}",
            self.current, self.next
        )
    }
}

impl std::error::Error for TrustTransitionError {}

/// Registro de um par da federação (`federation_peer`).
///
/// Só campo público: `public_key` é dado público por definição e é o que
/// permite verificar manifestos. Nenhuma chave privada tem lugar aqui.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRecord {
    /// Identificador do registro de par. Nesta fatia é igual a `node_id`; fica
    /// separado para que a fatia de transporte (M2) possa distinguir registros
    /// do mesmo nó por endpoint/relay sem migrar dado.
    pub peer_id: String,
    /// `node_id` do nó (`gmn_` + hex de `sha256(pubkey)`).
    pub node_id: String,
    /// Chave pública Ed25519 em hex.
    pub public_key: String,
    /// Cluster autorizado.
    pub cluster: String,
    /// Região declarada.
    pub region: String,
    /// Estado de confiança.
    pub trust_state: TrustState,
    /// Endpoints declarados.
    pub endpoints: Vec<String>,
    /// Versão do software.
    pub software_version: String,
    /// Capacidades declaradas.
    pub capabilities: Vec<String>,
    /// Geração da identidade de federação.
    pub epoch: u64,
    /// Última vez que o nó foi visto (segundos Unix).
    pub last_seen_at: i64,
}

impl PeerRecord {
    /// Cria o registro a partir de um manifesto **já verificado**, no estado
    /// inicial `pending`.
    ///
    /// Não verifica a assinatura: quem chama precisa ter chamado
    /// [`SignedManifest::verify`] antes. Isso é deliberado para que a criação do
    /// registro não dependa de uma segunda decisão implícita sobre validade.
    #[must_use]
    pub fn from_manifest(manifest: &SignedManifest, last_seen_at: i64) -> Self {
        Self {
            peer_id: manifest.node_id.clone(),
            node_id: manifest.node_id.clone(),
            public_key: manifest.public_key.clone(),
            cluster: manifest.cluster.clone(),
            region: manifest.region.clone(),
            trust_state: TrustState::Pending,
            endpoints: manifest.endpoints.clone(),
            software_version: manifest.software_version.clone(),
            capabilities: manifest.capabilities.clone(),
            epoch: manifest.epoch,
            last_seen_at,
        }
    }

    /// Aplica uma transição de estado, ou recusa sem alterar nada.
    ///
    /// # Errors
    /// [`TrustTransitionError`] quando a transição não está na máquina de
    /// estados; o registro permanece intacto.
    pub fn transition_to(&mut self, next: TrustState) -> Result<(), TrustTransitionError> {
        if !self.trust_state.can_transition_to(next) {
            return Err(TrustTransitionError {
                current: self.trust_state,
                next,
            });
        }
        self.trust_state = next;
        Ok(())
    }

    /// Se o par está autorizado. `pending`, `revoked` e `unknown` não estão.
    #[must_use]
    pub fn is_authorized(&self) -> bool {
        self.trust_state == TrustState::Authorized
    }

    /// Chave pública decodificada (32 bytes), ou erro se o campo persistido não
    /// for uma chave Ed25519.
    ///
    /// O transporte compara esta chave com a do certificado TLS; chave ilegível
    /// no registro é erro, não "aceita qualquer coisa".
    ///
    /// # Errors
    /// [`identity::IdentityError`] para hex inválido ou ponto fora da curva.
    pub fn public_key_bytes(&self) -> Result<[u8; PUBLIC_KEY_LEN], identity::IdentityError> {
        Ok(identity::parse_public_key(&self.public_key)?.to_bytes())
    }

    /// Registro de um par já **decidido** como `authorized`, a partir da chave.
    ///
    /// Serve ao runtime (carga do banco) e a teste. Não é caminho de promoção:
    /// a máquina de estados continua valendo para toda transição posterior, e
    /// quem chama já decidiu a autorização em outro lugar (aprovação no banco ou
    /// bootstrap declarado pelo operador).
    #[must_use]
    pub fn authorized_from_key(
        public_key: &[u8; PUBLIC_KEY_LEN],
        cluster: impl Into<String>,
    ) -> Self {
        let node_id = identity::node_id_for(public_key);
        Self {
            peer_id: node_id.clone(),
            node_id,
            public_key: hex::encode(public_key),
            cluster: cluster.into(),
            region: String::new(),
            trust_state: TrustState::Authorized,
            endpoints: Vec::new(),
            software_version: String::new(),
            capabilities: Vec::new(),
            epoch: 1,
            last_seen_at: 0,
        }
    }

    /// Fixa o estado de confiança na **construção** (teste e carga inicial).
    ///
    /// Transições depois da construção passam por [`Self::transition_to`].
    #[must_use]
    pub fn with_trust_state(mut self, state: TrustState) -> Self {
        self.trust_state = state;
        self
    }
}

/// Registro de pares **em memória**, compartilhado pelo listener QUIC, pelos
/// clientes de saída e pelo diagnóstico de enlace.
///
/// Complementa o banco: `federation_peer` é a autoridade persistida e este
/// registro é a cópia de runtime que os verificadores de certificado podem ler
/// sem `await` (o verificador do `rustls` é síncrono por contrato). Guarda
/// [`PeerRecord`] — não existe um segundo tipo de registro de par.
///
/// Sem CA, sem nome, sem TOFU: chave que não está aqui não abre conexão, e um
/// par revogado continua **conhecido** de propósito (o TLS aceita e o handshake
/// fecha com código explícito de revogação; o par aprende por quê).
#[derive(Debug, Default)]
pub struct PeerRegistry {
    inner: RwLock<HashMap<String, PeerRecord>>,
}

impl PeerRegistry {
    /// Registro vazio: nenhuma chave é aceita até ser declarada.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insere/substitui um par. Preserva `last_seen_at` quando o registro novo
    /// não traz evidência própria (0), porque evidência de vida não se perde por
    /// recarga de configuração.
    pub fn insert(&self, record: PeerRecord) {
        let mut guard = self.write();
        let last_seen_at = match guard.get(&record.node_id) {
            Some(existing) if record.last_seen_at == 0 => existing.last_seen_at,
            _ => record.last_seen_at,
        };
        let mut record = record;
        record.last_seen_at = last_seen_at;
        guard.insert(record.node_id.clone(), record);
    }

    /// Substitui o conjunto inteiro pelo snapshot da autoridade persistida.
    ///
    /// Par que deixou de estar autorizado no banco **sai** do registro: a
    /// allowlist é o snapshot, não a união dele com o passado. `last_seen_at`
    /// observado é preservado por `node_id`.
    pub fn replace(&self, records: impl IntoIterator<Item = PeerRecord>) {
        let mut replacement = HashMap::new();
        for record in records {
            let mut record = record;
            if let Some(previous) = self.read().get(&record.node_id)
                && record.last_seen_at == 0
            {
                record.last_seen_at = previous.last_seen_at;
            }
            replacement.insert(record.node_id.clone(), record);
        }
        *self.write() = replacement;
    }

    /// Muda o estado de confiança pela máquina de estados canônica.
    ///
    /// Transição ilegal (por exemplo `revoked → authorized`) devolve `false` e
    /// não altera nada. `epoch` declarado menor que o registrado também é
    /// ignorado: não existe "revogação atrasada" rebaixando estado.
    pub fn set_trust_state(&self, node_id: &str, next: TrustState, epoch: Option<u64>) -> bool {
        let mut guard = self.write();
        let Some(record) = guard.get_mut(node_id) else {
            return false;
        };
        if let Some(epoch) = epoch {
            if epoch < record.epoch {
                return false;
            }
            record.epoch = epoch;
        }
        record.transition_to(next).is_ok()
    }

    /// Revoga o par com o `epoch` declarado.
    pub fn revoke(&self, node_id: &str, epoch: u64) -> bool {
        self.set_trust_state(node_id, TrustState::Revoked, Some(epoch))
    }

    /// Devolve o par ao fluxo de aprovação (`pending`).
    ///
    /// É o único caminho de `revoked` de volta: `revoked → pending → authorized`
    /// exige decisão administrativa nova.
    pub fn request_rejoin(&self, node_id: &str) -> bool {
        self.set_trust_state(node_id, TrustState::Pending, None)
    }

    /// Registro do par, copiado (nunca um empréstimo através de `await`).
    #[must_use]
    pub fn record(&self, node_id: &str) -> Option<PeerRecord> {
        self.read().get(node_id).cloned()
    }

    /// Todos os pares conhecidos, ordenados por `node_id`.
    #[must_use]
    pub fn peers(&self) -> Vec<PeerRecord> {
        let mut peers: Vec<_> = self.read().values().cloned().collect();
        peers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        peers
    }

    /// Quantidade de pares conhecidos.
    #[must_use]
    pub fn len(&self) -> usize {
        self.read().len()
    }

    /// Se o registro está vazio.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    /// A chave está registrada? Decidido pela **chave**, nunca pelo nome do
    /// certificado (ADR-072 §2, invariante 8: endereço/chave, não nome).
    #[must_use]
    pub fn knows_key(&self, public_key: &[u8; PUBLIC_KEY_LEN]) -> bool {
        let wanted = hex::encode(public_key);
        self.read()
            .values()
            .any(|record| record.public_key == wanted)
    }

    /// Registra evidência de vida do par. Não muda `trust_state`: heartbeat não
    /// autoriza, só informa (ADR-072 §2, invariante 6).
    pub fn touch(&self, node_id: &str, seen_at: i64) {
        let mut guard = self.write();
        if let Some(record) = guard.get_mut(node_id) {
            record.last_seen_at = seen_at;
        }
    }

    /// Leitura com lock envenenado tratado como registro vazio (falha fechada:
    /// sem registro, nenhuma chave é aceita).
    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, PeerRecord>> {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, PeerRecord>> {
        self.inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NodeIdentity;

    fn manifest() -> SignedManifest {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let mut manifest = SignedManifest {
            node_id: String::new(),
            public_key: String::new(),
            cluster: "cluster-br-sul".to_owned(),
            region: "br-sul".to_owned(),
            software_version: "0.1.0".to_owned(),
            alpn: "godmesh/1".to_owned(),
            capabilities: vec!["sync".to_owned()],
            endpoints: vec!["quic://10.0.0.1:7000".to_owned()],
            epoch: 1,
            issued_at: 1_700_000_000,
            signature: String::new(),
        };
        manifest.sign(&identity);
        manifest
    }

    #[test]
    fn manifesto_verificado_vira_par_pending() {
        let manifest = manifest();
        manifest.verify().expect("manifesto válido");
        let peer = PeerRecord::from_manifest(&manifest, 1_700_000_010);
        assert_eq!(peer.trust_state, TrustState::Pending);
        assert!(!peer.is_authorized());
        assert_eq!(peer.peer_id, manifest.node_id);
        assert_eq!(peer.node_id, manifest.node_id);
        assert_eq!(peer.public_key, manifest.public_key);
        assert_eq!(peer.epoch, 1);
        assert_eq!(peer.last_seen_at, 1_700_000_010);
    }

    #[test]
    fn pendente_nao_e_autorizado_ate_a_aprovacao() {
        let mut peer = PeerRecord::from_manifest(&manifest(), 0);
        assert!(!peer.is_authorized());
        peer.transition_to(TrustState::Authorized)
            .expect("aprovação");
        assert!(peer.is_authorized());
    }

    #[test]
    fn revogar_impede_uso_e_exige_nova_aprovacao() {
        let mut peer = PeerRecord::from_manifest(&manifest(), 0);
        peer.transition_to(TrustState::Authorized)
            .expect("aprovação");
        peer.transition_to(TrustState::Revoked).expect("revogação");
        assert!(!peer.is_authorized());
        // Volta a pedir: o caminho é pending, não authorized direto.
        peer.transition_to(TrustState::Pending)
            .expect("novo pedido");
        assert!(!peer.is_authorized());
    }

    #[test]
    fn transicoes_legais_e_idempotentes() {
        let legal = [
            (TrustState::Unknown, TrustState::Pending),
            (TrustState::Pending, TrustState::Authorized),
            (TrustState::Pending, TrustState::Revoked),
            (TrustState::Authorized, TrustState::Revoked),
            (TrustState::Revoked, TrustState::Pending),
            (TrustState::Pending, TrustState::Pending),
            (TrustState::Authorized, TrustState::Authorized),
        ];
        for (from, to) in legal {
            assert!(from.can_transition_to(to), "{from} → {to} deveria valer");
        }
    }

    #[test]
    fn transicoes_legais_sao_exatamente_as_declaradas() {
        // Varredura completa: qualquer par fora da lista é ilegal. Sem isso, uma
        // transição nova entra sem ninguém decidir.
        let all = [
            TrustState::Pending,
            TrustState::Authorized,
            TrustState::Revoked,
            TrustState::Unknown,
        ];
        let legal = [
            (TrustState::Unknown, TrustState::Pending),
            (TrustState::Pending, TrustState::Authorized),
            (TrustState::Pending, TrustState::Revoked),
            (TrustState::Authorized, TrustState::Revoked),
            (TrustState::Revoked, TrustState::Pending),
            (TrustState::Pending, TrustState::Pending),
            (TrustState::Authorized, TrustState::Authorized),
        ];
        for from in all {
            for to in all {
                assert_eq!(
                    from.can_transition_to(to),
                    legal.contains(&(from, to)),
                    "par inesperado: {from} → {to}"
                );
            }
        }
    }

    #[test]
    fn revogado_nao_volta_direto_para_autorizado() {
        let mut peer = PeerRecord::from_manifest(&manifest(), 0);
        peer.transition_to(TrustState::Authorized)
            .expect("aprovação");
        peer.transition_to(TrustState::Revoked).expect("revogação");
        let e = peer
            .transition_to(TrustState::Authorized)
            .expect_err("revogado não volta direto");
        assert_eq!(e.current, TrustState::Revoked);
        assert_eq!(e.next, TrustState::Authorized);
        assert_eq!(peer.trust_state, TrustState::Revoked, "registro intacto");
    }

    #[test]
    fn estado_fora_do_contrato_nao_e_lido_como_autorizado() {
        assert_eq!(
            TrustState::from_name("authorized"),
            Some(TrustState::Authorized)
        );
        assert_eq!(TrustState::from_name("aprovado"), None);
        assert_eq!(TrustState::from_name("AUTHORIZED"), None);
        assert_eq!(TrustState::Authorized.as_str(), "authorized");
    }

    fn key(byte: u8) -> [u8; PUBLIC_KEY_LEN] {
        [byte; PUBLIC_KEY_LEN]
    }

    #[test]
    fn chave_nao_registrada_nao_e_conhecida() {
        let registry = PeerRegistry::new();
        assert!(registry.is_empty());
        assert!(!registry.knows_key(&key(9)));
        assert!(registry.record(&identity::node_id_for(&key(9))).is_none());
    }

    #[test]
    fn revogar_mantem_a_chave_conhecida_e_exige_nova_aprovacao() {
        let registry = PeerRegistry::new();
        let record = PeerRecord::authorized_from_key(&key(1), "br-south");
        let node_id = record.node_id.clone();
        registry.insert(record);
        assert!(registry.knows_key(&key(1)));
        assert_eq!(
            registry.record(&node_id).expect("par").trust_state,
            TrustState::Authorized
        );

        assert!(registry.revoke(&node_id, 2));
        let record = registry.record(&node_id).expect("par");
        assert_eq!(record.trust_state, TrustState::Revoked);
        assert_eq!(record.epoch, 2);
        // Revogado segue conhecido: o TLS aceita e o handshake fecha com motivo.
        assert!(registry.knows_key(&key(1)));

        // `revoked → authorized` direto é ilegal pela máquina canônica.
        assert!(!registry.set_trust_state(&node_id, TrustState::Authorized, None));
        assert_eq!(
            registry.record(&node_id).expect("par").trust_state,
            TrustState::Revoked
        );
        // O caminho de volta é `pending` e nova decisão.
        assert!(registry.request_rejoin(&node_id));
        assert!(registry.set_trust_state(&node_id, TrustState::Authorized, None));
        assert!(registry.record(&node_id).expect("par").is_authorized());
    }

    #[test]
    fn epoch_de_revogacao_menor_nao_rebaixa_o_estado() {
        let registry = PeerRegistry::new();
        let record = PeerRecord::authorized_from_key(&key(2), "br-south");
        let node_id = record.node_id.clone();
        registry.insert(record);
        assert!(registry.revoke(&node_id, 5));
        assert!(!registry.revoke(&node_id, 4));
        let record = registry.record(&node_id).expect("par");
        assert_eq!(record.epoch, 5);
        assert_eq!(record.trust_state, TrustState::Revoked);
    }

    #[test]
    fn evidencia_de_vida_nao_promove_estado() {
        let registry = PeerRegistry::new();
        let record = PeerRecord::authorized_from_key(&key(3), "br-south")
            .with_trust_state(TrustState::Pending);
        let node_id = record.node_id.clone();
        registry.insert(record);
        registry.touch(&node_id, 42);
        let record = registry.record(&node_id).expect("par");
        assert_eq!(record.trust_state, TrustState::Pending);
        assert_eq!(record.last_seen_at, 42);
    }

    #[test]
    fn recarga_substitui_o_conjunto_e_preserva_evidencia() {
        let registry = PeerRegistry::new();
        let first = PeerRecord::authorized_from_key(&key(4), "br-south");
        let second = PeerRecord::authorized_from_key(&key(5), "br-south");
        let kept = first.node_id.clone();
        let dropped = second.node_id.clone();
        registry.insert(first);
        registry.insert(second);
        registry.touch(&kept, 99);

        // O banco agora só autoriza a primeira chave.
        registry.replace([PeerRecord::authorized_from_key(&key(4), "br-south")]);
        assert_eq!(registry.len(), 1);
        assert!(registry.record(&dropped).is_none(), "par removido sai do registro");
        assert_eq!(
            registry.record(&kept).expect("par").last_seen_at,
            99,
            "evidência de vida não se perde na recarga"
        );
    }
}
