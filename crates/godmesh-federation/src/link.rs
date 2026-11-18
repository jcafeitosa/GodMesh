//! Estado do enlace por par — observável e **unknown de primeira classe**.
//!
//! Invariante 6 do ADR-072 §2: `unknown` nunca é promovido a sucesso por
//! passagem de tempo. Aqui isso é estrutural: só um heartbeat efetivamente
//! recebido marca `ready`; todo o resto degrada para `unknown`.
//!
//! `last_seen_at`, `rtt_ms`, `negotiated_alpn` e `peer_version` são **última
//! evidência**, não promessa: quando o enlace cai, ficam como histórico e o
//! `state` diz a verdade. `rtt_ms` é zerado ao expirar porque medição viva de
//! enlace morto não é medição.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;


/// Estado do enlace de um par.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    /// Handshake concluído **e** heartbeat dentro da janela.
    Ready,
    /// Sem evidência recente: conexão caída, heartbeat parado ou nó nunca visto.
    /// Nunca vira `ready` só porque o tempo passou.
    Unknown,
    /// A tentativa de conexão foi recusada (assinatura, cluster, janela de
    /// manifesto, estado de confiança). Rejeição é veredito, não timeout.
    Rejected,
}

impl LinkState {
    /// Nome estável no contrato JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unknown => "unknown",
            Self::Rejected => "rejected",
        }
    }
}

/// Fotografia do enlace, como o endpoint de diagnóstico devolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct LinkSnapshot {
    /// `ready`, `unknown` ou `rejected`.
    pub state: LinkState,
    /// Segundo Unix do último heartbeat aceito (`None`: nenhuma evidência).
    pub last_seen_at: Option<i64>,
    /// RTT medido do QUIC no momento do handshake (`None` sem enlace vivo).
    pub rtt_ms: Option<u64>,
    /// ALPN efetivamente negociado (`None` sem handshake).
    pub negotiated_alpn: Option<String>,
    /// `software_version` declarada no manifesto assinado do par.
    pub peer_version: Option<String>,
}

impl LinkSnapshot {
    /// Estado sem nenhuma evidência. É o piso honesto: nada foi observado.
    #[must_use]
    pub fn never_seen() -> Self {
        Self {
            state: LinkState::Unknown,
            last_seen_at: None,
            rtt_ms: None,
            negotiated_alpn: None,
            peer_version: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Entry {
    state: LinkState,
    last_seen_at: Option<i64>,
    rtt_ms: Option<u64>,
    alpn: Option<String>,
    peer_version: Option<String>,
    /// Instante monotônico do último heartbeat — a única base para expirar.
    last_beat: Instant,
}

/// Registro de enlaces observados, compartilhado entre os nós QUIC e o serviço
/// de diagnóstico.
#[derive(Debug)]
pub struct PeerLinks {
    inner: Mutex<HashMap<String, Entry>>,
    freshness: Duration,
}

impl PeerLinks {
    /// `freshness` é a janela de silêncio tolerada: `heartbeat_interval` ×
    /// `missed_heartbeats` do nó.
    #[must_use]
    pub fn new(freshness: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            freshness,
        }
    }

    /// Marca o enlace como pronto — só chamado depois de handshake verificado e
    /// ALPN conferido.
    pub fn mark_ready(
        &self,
        node_id: &str,
        last_seen_at: i64,
        rtt_ms: Option<u64>,
        alpn: String,
        peer_version: String,
        now: Instant,
    ) {
        let mut guard = self.lock();
        guard.insert(
            node_id.to_owned(),
            Entry {
                state: LinkState::Ready,
                last_seen_at: Some(last_seen_at),
                rtt_ms,
                alpn: Some(alpn),
                peer_version: Some(peer_version),
                last_beat: now,
            },
        );
    }

    /// Heartbeat aceito: renova a evidência. **Não** promove `unknown` a `ready`
    /// — para isso o handshake precisa ter sido refeito, porque a conexão que
    /// recebeu o heartbeat é a mesma que foi verificada.
    pub fn beat(&self, node_id: &str, seen_at: i64, now: Instant) {
        let mut guard = self.lock();
        if let Some(entry) = guard.get_mut(node_id) {
            entry.last_seen_at = Some(seen_at);
            entry.last_beat = now;
            // Entrada em `unknown`/`rejected` que recebe heartbeat é anomalia de
            // ordem: registra evidência mas **não** promove para `ready`.
        }
    }

    /// Enlace encerrado (conexão fechada/timeout): `unknown`, sem RTT.
    pub fn mark_unknown(&self, node_id: &str) {
        let mut guard = self.lock();
        if let Some(entry) = guard.get_mut(node_id) {
            entry.state = LinkState::Unknown;
            entry.rtt_ms = None;
        }
    }

    /// Tentativa de conexão recusada.
    pub fn mark_rejected(&self, node_id: &str) {
        let mut guard = self.lock();
        let entry = guard.entry(node_id.to_owned()).or_insert_with(|| Entry {
            state: LinkState::Rejected,
            last_seen_at: None,
            rtt_ms: None,
            alpn: None,
            peer_version: None,
            last_beat: Instant::now(),
        });
        entry.state = LinkState::Rejected;
        entry.rtt_ms = None;
    }

    /// Expira enlaces silenciosos. Devolve os `node_id` que passaram a `unknown`
    /// (para log/telemetria; sem conteúdo de dados).
    pub fn expire(&self, now: Instant) -> Vec<String> {
        let mut expired = Vec::new();
        let mut guard = self.lock();
        for (node_id, entry) in guard.iter_mut() {
            if entry.state == LinkState::Ready && now.duration_since(entry.last_beat) > self.freshness
            {
                entry.state = LinkState::Unknown;
                entry.rtt_ms = None;
                expired.push(node_id.clone());
            }
        }
        expired
    }

    /// Fotografia do enlace, ou `None` se o par nunca foi observado.
    #[must_use]
    pub fn snapshot(&self, node_id: &str) -> Option<LinkSnapshot> {
        let guard = self.lock();
        guard.get(node_id).map(|entry| LinkSnapshot {
            state: entry.state,
            last_seen_at: entry.last_seen_at,
            rtt_ms: entry.rtt_ms,
            negotiated_alpn: entry.alpn.clone(),
            peer_version: entry.peer_version.clone(),
        })
    }

    /// Snapshot com piso `unknown` — usado pelo diagnóstico quando não há
    /// registro do par.
    #[must_use]
    pub fn snapshot_or_never_seen(&self, node_id: &str) -> LinkSnapshot {
        self.snapshot(node_id).unwrap_or_else(LinkSnapshot::never_seen)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::node_id_for;

    fn id(byte: u8) -> String {
        node_id_for(&[byte; 32])
    }

    #[test]
    fn never_seen_is_unknown_without_evidence() {
        let links = PeerLinks::new(Duration::from_secs(1));
        let snapshot = links.snapshot_or_never_seen(&id(1));
        assert_eq!(snapshot.state, LinkState::Unknown);
        assert_eq!(snapshot.last_seen_at, None);
        assert_eq!(snapshot.rtt_ms, None);
        assert_eq!(snapshot.negotiated_alpn, None);
        assert_eq!(snapshot.peer_version, None);
    }

    #[test]
    fn silent_link_expires_to_unknown_and_never_returns_to_ready() {
        let links = PeerLinks::new(Duration::from_millis(50));
        let node = id(2);
        let start = Instant::now();
        links.mark_ready(&node, 10, Some(7), "godmesh/1".to_owned(), "0.1.0".to_owned(), start);
        assert_eq!(links.snapshot(&node).expect("enlace").state, LinkState::Ready);

        assert!(links.expire(start).is_empty());
        // Um heartbeat renova a evidência.
        links.beat(&node, 11, start + Duration::from_millis(20));
        assert!(links.expire(start + Duration::from_millis(40)).is_empty());

        // Silêncio maior que a janela: unknown.
        let expired = links.expire(start + Duration::from_millis(200));
        assert_eq!(expired, vec![node.clone()]);
        let snapshot = links.snapshot(&node).expect("enlace");
        assert_eq!(snapshot.state, LinkState::Unknown);
        assert_eq!(snapshot.rtt_ms, None, "medição viva não sobrevive ao enlace");
        assert_eq!(snapshot.last_seen_at, Some(11), "última evidência é histórica");

        // Passar mais tempo não promove nada.
        assert!(links.expire(start + Duration::from_secs(10)).is_empty());
        assert_eq!(links.snapshot(&node).expect("enlace").state, LinkState::Unknown);
    }

    #[test]
    fn rejected_is_terminal_for_the_attempt() {
        let links = PeerLinks::new(Duration::from_secs(1));
        let node = id(3);
        links.mark_rejected(&node);
        assert_eq!(links.snapshot(&node).expect("enlace").state, LinkState::Rejected);
        assert!(links.expire(Instant::now()).is_empty());
    }
}