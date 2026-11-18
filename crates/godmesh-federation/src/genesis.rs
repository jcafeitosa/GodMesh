//! Bootstrap de confiança da malha: nós genesis declarados pelo operador.
//!
//! A emenda à ADR-071 §5 é explícita: **bootstrap não é confiança automática**.
//! Os primeiros nós de um cluster são autorizados *por configuração*, fora de
//! banda, e só a partir deles o ingresso passa a vir de aprovação assinada. Não
//! existe *trust on first use*: um nó que se apresenta sem estar no genesis e
//! sem aprovação assinada não é autorizado por nenhum caminho.
//!
//! ## Formato de `GODMESH_FEDERATION_GENESIS`
//!
//! Entradas separadas por vírgula, ponto e vírgula, espaço ou quebra de linha;
//! cada entrada é `<node_id>:<chave pública em hex>`. Ambos os campos são
//! obrigatórios e a linha **não** é aceita se:
//!
//! * `node_id` não tiver a forma `gmn_` + 64 hex minúsculos;
//! * a chave pública não for hex de 32 bytes nem ponto válido da curva;
//! * `node_id` não for o derivado daquela chave (o laço que a ADR exige);
//! * o mesmo `node_id` aparecer duas vezes (configuração divergente não é
//!   "último gagne" — é erro).
//!
//! A variável ausente ou vazia é o estado **sem genesis**: nada é autorizado
//! por configuração, e nenhum nó existe ainda — então aprovar é impossível. Isso
//! é o comportamento declarado, não um default inventado.
//!
//! Nenhuma chave privada aparece aqui: genesis declara identidade *pública*, que
//! é o que verifica assinatura.

use std::fmt;

use crate::identity::{self, NodeIdentity};

/// Variável de ambiente com a lista de nós genesis.
pub const GENESIS_ENV: &str = "GODMESH_FEDERATION_GENESIS";

/// Separadores aceitos entre entradas.
const SEPARATORS: [char; 4] = ['\n', '\r', ',', ';'];

/// Um nó autorizado por configuração, antes de qualquer aprovação assinada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisNode {
    /// `node_id` do nó (`gmn_` + hex de `sha256(pubkey)`).
    pub node_id: String,
    /// Chave pública Ed25519 em hex minúsculo.
    pub public_key: String,
}

impl GenesisNode {
    /// Constrói um nó genesis conferindo o laço `node_id`↔chave pública.
    ///
    /// # Errors
    /// [`GenesisError`] para formato inválido ou `node_id` não derivado da chave.
    pub fn new(node_id: &str, public_key: &str) -> Result<Self, GenesisError> {
        let node_id = node_id.trim();
        let public_key = public_key.trim();
        if !identity::is_node_id(node_id) {
            return Err(GenesisError::InvalidNodeId {
                entry: format!("{node_id}:{public_key}"),
            });
        }
        // Decodificar antes de comparar: hex inválido ou ponto fora da curva é
        // erro nomeado, não "chave que nunca casa".
        let key =
            identity::parse_public_key(public_key).map_err(|e| GenesisError::InvalidPublicKey {
                entry: node_id.to_owned(),
                detail: e.to_string(),
            })?;
        if identity::node_id_for(&key.to_bytes()) != node_id {
            return Err(GenesisError::NodeIdMismatch {
                entry: format!("{node_id}:{public_key}"),
            });
        }
        Ok(Self {
            node_id: node_id.to_owned(),
            public_key: public_key.to_owned(),
        })
    }

    /// Igual a [`Self::new`], a partir da identidade de um nó local.
    #[must_use]
    pub fn from_identity(identity: &NodeIdentity) -> Self {
        Self {
            node_id: identity.node_id().to_owned(),
            public_key: identity.public_key_hex(),
        }
    }
}

/// Lista dos nós genesis de um cluster.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Genesis {
    nodes: Vec<GenesisNode>,
}

/// Falhas de leitura do genesis. Toda entrada inválida é erro: configuração de
/// confiança não tem meio-termo entre "válido" e "ignorado".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenesisError {
    /// Entrada sem `:`, vazia ou com campos faltando.
    MalformedEntry { entry: String },
    /// `node_id` fora da forma `gmn_` + 64 hex.
    InvalidNodeId { entry: String },
    /// Chave pública ilegível (hex, tamanho ou ponto).
    InvalidPublicKey { entry: String, detail: String },
    /// `node_id` não é o derivado da chave pública da entrada.
    NodeIdMismatch { entry: String },
    /// O mesmo `node_id` aparece mais de uma vez.
    DuplicateNode { node_id: String },
}

impl fmt::Display for GenesisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEntry { entry } => write!(
                f,
                "entrada de {GENESIS_ENV} malformada: {entry:?}; esperado <node_id>:<chave pública em hex>"
            ),
            Self::InvalidNodeId { entry } => {
                write!(f, "node_id inválido em {GENESIS_ENV}: {entry:?}")
            }
            Self::InvalidPublicKey { entry, detail } => {
                write!(
                    f,
                    "chave pública inválida em {GENESIS_ENV} para {entry}: {detail}"
                )
            }
            Self::NodeIdMismatch { entry } => write!(
                f,
                "node_id não é o derivado da chave pública em {GENESIS_ENV}: {entry:?}"
            ),
            Self::DuplicateNode { node_id } => {
                write!(f, "{node_id} declarado mais de uma vez em {GENESIS_ENV}")
            }
        }
    }
}

impl std::error::Error for GenesisError {}

impl Genesis {
    /// Lista vazia: sem bootstrap por configuração.
    #[must_use]
    pub const fn empty() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Constrói a partir de entradas já validadas.
    #[must_use]
    pub fn from_nodes(nodes: impl IntoIterator<Item = GenesisNode>) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
        }
    }

    /// Lê `GODMESH_FEDERATION_GENESIS`.
    ///
    /// # Errors
    /// [`GenesisError`] para entrada malformada, duplicada ou com laço
    /// `node_id`↔chave quebrado. Ausente ou vazio é lista vazia (sem erro): é o
    /// estado declarado "nenhum nó autorizado por configuração".
    pub fn from_env_with(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, GenesisError> {
        Ok(match lookup(GENESIS_ENV) {
            Some(raw) => Self::parse(&raw)?,
            None => Self::empty(),
        })
    }

    /// Lê o ambiente do processo. Ver [`Self::from_env_with`].
    ///
    /// # Errors
    /// [`GenesisError`] conforme [`Self::from_env_with`].
    pub fn from_env() -> Result<Self, GenesisError> {
        Self::from_env_with(|name| std::env::var(name).ok())
    }

    /// Analisa o conteúdo bruto da variável.
    ///
    /// # Errors
    /// [`GenesisError`] para qualquer entrada inválida; nunca entra no meio-termo
    /// de "aceitar as boas e ignorar o resto".
    pub fn parse(raw: &str) -> Result<Self, GenesisError> {
        let mut nodes: Vec<GenesisNode> = Vec::new();
        for entry in raw.split(|c: char| SEPARATORS.contains(&c)) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let Some((node_id, public_key)) = entry.split_once(':') else {
                return Err(GenesisError::MalformedEntry {
                    entry: entry.to_owned(),
                });
            };
            let node = GenesisNode::new(node_id, public_key)?;
            if nodes.iter().any(|known| known.node_id == node.node_id) {
                return Err(GenesisError::DuplicateNode {
                    node_id: node.node_id,
                });
            }
            nodes.push(node);
        }
        Ok(Self { nodes })
    }

    /// Nós declarados, na ordem da configuração.
    #[must_use]
    pub fn nodes(&self) -> &[GenesisNode] {
        &self.nodes
    }

    /// Quantidade declarada — o número que `GET /v1/meta` publica como estado.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Sem bootstrap por configuração.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// A chave pública registrada para `node_id`, se ele for genesis.
    #[must_use]
    pub fn public_key_of(&self, node_id: &str) -> Option<&str> {
        self.nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .map(GenesisNode::public_key)
    }

    /// Se o nó é genesis **com esta chave pública**.
    ///
    /// A chave faz parte da decisão: `node_id` é um compromisso público com
    /// ela, e uma configuração com o identificador certo e a chave errada deve
    /// recusar, não "confiar no identificador".
    #[must_use]
    pub fn is_authorized(&self, node_id: &str, public_key: &str) -> bool {
        self.public_key_of(node_id) == Some(public_key)
    }
}

impl GenesisNode {
    /// Chave pública declarada.
    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(identity: &NodeIdentity) -> String {
        format!("{}:{}", identity.node_id(), identity.public_key_hex())
    }

    #[test]
    fn parse_aceita_separadores_e_preserva_a_ordem() {
        let first = NodeIdentity::generate_in_memory().expect("identidade");
        let second = NodeIdentity::generate_in_memory().expect("identidade");
        let raw = format!(" {}\n{};{} ", pair(&first), "   ", pair(&second));
        let genesis = Genesis::parse(&raw).expect("parse com espaços e separadores");
        assert_eq!(
            genesis.len(),
            2,
            "entrada vazia entre separadores é ignorada"
        );
        assert_eq!(genesis.nodes()[0].node_id, first.node_id());
        assert_eq!(genesis.nodes()[1].node_id, second.node_id());
        assert!(genesis.is_authorized(second.node_id(), &second.public_key_hex()));

        // Repetir o mesmo node_id é configuração divergente, não "último gagne".
        assert_eq!(
            Genesis::parse(&format!("{};{}", pair(&first), pair(&first))),
            Err(GenesisError::DuplicateNode {
                node_id: first.node_id().to_owned()
            })
        );
    }

    #[test]
    fn ambiente_ausente_ou_vazio_e_lista_vazia_sem_erro() {
        assert!(
            Genesis::from_env_with(|_| None)
                .expect("sem variável")
                .is_empty()
        );
        assert!(
            Genesis::from_env_with(|_| Some("   ".to_owned()))
                .expect("variável vazia")
                .is_empty(),
            "vazio é o estado declarado, não um default inventado"
        );
    }

    #[test]
    fn entrada_sem_dois_pontos_e_recusada() {
        assert_eq!(
            Genesis::parse("gmn_deadbeef"),
            Err(GenesisError::MalformedEntry {
                entry: "gmn_deadbeef".to_owned()
            })
        );
    }

    #[test]
    fn node_id_que_nao_e_derivado_da_chave_e_recusado() {
        let signer = NodeIdentity::generate_in_memory().expect("identidade");
        let outro = NodeIdentity::generate_in_memory().expect("outra identidade");
        // Identificador de um nó, chave de outro: é exatamente o typo que a ADR
        // não permite virar confiança.
        let raw = format!("{}:{}", outro.node_id(), signer.public_key_hex());
        assert_eq!(
            Genesis::parse(&raw),
            Err(GenesisError::NodeIdMismatch { entry: raw })
        );
    }

    #[test]
    fn chave_invalida_e_nomeada_como_tal() {
        let signer = NodeIdentity::generate_in_memory().expect("identidade");
        let raw = format!("{}:{}", signer.node_id(), "zz");
        let err = Genesis::parse(&raw).expect_err("hex inválido");
        assert!(
            matches!(err, GenesisError::InvalidPublicKey { .. }),
            "{err}"
        );
        assert_eq!(
            Genesis::parse(&format!("{}:{}", "gmn_curto", signer.public_key_hex())),
            Err(GenesisError::InvalidNodeId {
                entry: format!("gmn_curto:{}", signer.public_key_hex())
            })
        );
    }

    #[test]
    fn is_authorized_exige_a_chave_declarada() {
        let signer = NodeIdentity::generate_in_memory().expect("identidade");
        let outro = NodeIdentity::generate_in_memory().expect("outra identidade");
        let genesis = Genesis::from_nodes([GenesisNode::from_identity(&signer)]);
        assert!(genesis.is_authorized(signer.node_id(), &signer.public_key_hex()));
        assert!(
            !genesis.is_authorized(signer.node_id(), &outro.public_key_hex()),
            "identificador certo com chave errada não autoriza nada"
        );
        assert!(!genesis.is_authorized(outro.node_id(), &outro.public_key_hex()));
    }
}
