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

use std::fmt;

use serde::{Deserialize, Serialize};

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
}
