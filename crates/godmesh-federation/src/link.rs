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

use quinn::Connection;
use serde::Serialize;
use tokio::sync::watch;

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

/// Evidência operacional exclusiva desta conexão.
#[derive(Debug, Clone)]
pub(crate) struct LinkValidity {
    pub(crate) connection: Connection,
    pub(crate) trust: watch::Receiver<u64>,
    pub(crate) deadline: watch::Receiver<Option<Instant>>,
}

impl LinkValidity {
    pub(crate) fn is_current(&self, now: Instant) -> bool {
        self.connection.close_reason().is_none()
            && matches!(self.trust.has_changed(), Ok(false))
            && self.deadline.borrow().is_none_or(|deadline| now < deadline)
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
    validity: Option<LinkValidity>,
}

impl Entry {
    fn is_current(&self, now: Instant) -> bool {
        self.validity
            .as_ref()
            .is_none_or(|validity| validity.is_current(now))
    }

    fn is_ready(&self, now: Instant, freshness: Duration) -> bool {
        self.state == LinkState::Ready
            && self.is_current(now)
            && (self.validity.is_none()
                || now.saturating_duration_since(self.last_beat) <= freshness)
    }

    fn belongs_to(&self, connection: &Connection) -> bool {
        self.validity
            .as_ref()
            .is_some_and(|validity| validity.connection.stable_id() == connection.stable_id())
    }
}

/// Registro de enlaces observados, compartilhado entre os nós QUIC e o serviço
/// de diagnóstico.
#[derive(Debug)]
pub struct PeerLinks {
    inner: Mutex<HashMap<String, Vec<Entry>>>,
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
            vec![Entry {
                state: LinkState::Ready,
                last_seen_at: Some(last_seen_at),
                rtt_ms,
                alpn: Some(alpn),
                peer_version: Some(peer_version),
                last_beat: now,
                validity: None,
            }],
        );
    }

    pub(crate) fn mark_operational_ready(
        &self,
        node_id: &str,
        snapshot: LinkSnapshot,
        validity: LinkValidity,
    ) -> bool {
        let mut guard = self.lock();
        let now = Instant::now();
        if !validity.is_current(now) {
            return false;
        }
        let entries = guard.entry(node_id.to_owned()).or_default();
        // Conserve enlaces paralelos e descarte apenas o histórico sem conexão.
        entries.retain(|entry| entry.validity.is_some());
        entries.push(Entry {
            state: snapshot.state,
            last_seen_at: snapshot.last_seen_at,
            rtt_ms: snapshot.rtt_ms,
            alpn: snapshot.negotiated_alpn,
            peer_version: snapshot.peer_version,
            last_beat: now,
            validity: Some(validity),
        });
        true
    }

    pub(crate) fn operational_is_ready(&self, node_id: &str, connection: &Connection) -> bool {
        let guard = self.lock();
        let now = Instant::now();
        guard.get(node_id).is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.belongs_to(connection) && entry.is_ready(now, self.freshness))
        })
    }

    pub(crate) fn beat_operational(
        &self,
        node_id: &str,
        connection: &Connection,
        seen_at: i64,
    ) -> bool {
        let mut guard = self.lock();
        let now = Instant::now();
        if let Some(entry) = guard.get_mut(node_id).and_then(|entries| {
            entries
                .iter_mut()
                .find(|entry| entry.belongs_to(connection) && entry.is_ready(now, self.freshness))
        }) {
            entry.last_seen_at = Some(seen_at);
            entry.last_beat = now;
            return true;
        }
        false
    }

    pub(crate) fn close_operational(&self, node_id: &str, connection: &Connection) {
        let mut guard = self.lock();
        if let Some(entries) = guard.get_mut(node_id)
            && let Some(index) = entries
                .iter()
                .position(|entry| entry.belongs_to(connection))
        {
            if entries.len() > 1 {
                entries.remove(index);
            } else {
                let entry = &mut entries[index];
                entry.state = LinkState::Unknown;
                entry.rtt_ms = None;
                entry.validity = None;
            }
        }
    }

    /// Heartbeat aceito: renova a evidência. **Não** promove `unknown` a `ready`
    /// — para isso o handshake precisa ter sido refeito, porque a conexão que
    /// recebeu o heartbeat é a mesma que foi verificada.
    pub fn beat(&self, node_id: &str, seen_at: i64, now: Instant) {
        let mut guard = self.lock();
        if let Some(entry) = guard
            .get_mut(node_id)
            .and_then(|entries| entries.last_mut())
        {
            entry.last_seen_at = Some(seen_at);
            entry.last_beat = now;
            // Entrada em `unknown`/`rejected` que recebe heartbeat é anomalia de
            // ordem: registra evidência mas **não** promove para `ready`.
        }
    }

    /// Enlace encerrado (conexão fechada/timeout): `unknown`, sem RTT.
    pub fn mark_unknown(&self, node_id: &str) {
        let mut guard = self.lock();
        if let Some(entry) = guard
            .get_mut(node_id)
            .and_then(|entries| entries.last_mut())
        {
            entry.state = LinkState::Unknown;
            entry.rtt_ms = None;
        }
    }

    /// Tentativa de conexão recusada.
    pub fn mark_rejected(&self, node_id: &str) {
        self.record_rejection(node_id, false);
    }

    pub(crate) fn record_rejection(&self, node_id: &str, operational: bool) {
        let mut guard = self.lock();
        if operational
            && guard
                .get(node_id)
                .is_some_and(|entries| entries.iter().any(|entry| entry.validity.is_some()))
        {
            return;
        }
        let entries = guard.entry(node_id.to_owned()).or_default();
        if let Some(entry) = entries.last_mut() {
            entry.state = LinkState::Rejected;
            entry.rtt_ms = None;
        } else {
            entries.push(Entry {
                state: LinkState::Rejected,
                last_seen_at: None,
                rtt_ms: None,
                alpn: None,
                peer_version: None,
                last_beat: Instant::now(),
                validity: None,
            });
        }
    }

    /// Expira enlaces silenciosos e fecha as conexões operacionais afetadas.
    /// Devolve os `node_id` que passaram a `unknown`
    /// (para log/telemetria; sem conteúdo de dados).
    pub fn expire(&self, now: Instant) -> Vec<String> {
        let mut expired = Vec::new();
        let mut guard = self.lock();
        for (node_id, entries) in guard.iter_mut() {
            let mut changed = false;
            for entry in entries.iter_mut() {
                if entry.state == LinkState::Ready
                    && now.duration_since(entry.last_beat) > self.freshness
                {
                    entry.state = LinkState::Unknown;
                    entry.rtt_ms = None;
                    if let Some(validity) = &entry.validity {
                        validity
                            .connection
                            .close(0u32.into(), b"heartbeat expirado");
                    }
                    changed = true;
                }
            }
            if changed
                && !entries
                    .iter()
                    .any(|entry| entry.is_ready(now, self.freshness))
            {
                expired.push(node_id.clone());
            }
        }
        expired
    }

    /// Fotografia do enlace, ou `None` se o par nunca foi observado.
    #[must_use]
    pub fn snapshot(&self, node_id: &str) -> Option<LinkSnapshot> {
        let guard = self.lock();
        let entries = guard.get(node_id)?;
        let now = Instant::now();
        let entry = entries
            .iter()
            .rev()
            .find(|entry| entry.is_ready(now, self.freshness))
            .or_else(|| entries.last())?;
        let invalid = !entry.is_current(now)
            || (entry.state == LinkState::Ready && !entry.is_ready(now, self.freshness));
        Some(LinkSnapshot {
            state: if invalid {
                LinkState::Unknown
            } else {
                entry.state
            },
            last_seen_at: entry.last_seen_at,
            rtt_ms: if invalid { None } else { entry.rtt_ms },
            negotiated_alpn: entry.alpn.clone(),
            peer_version: entry.peer_version.clone(),
        })
    }

    /// Snapshot com piso `unknown` — usado pelo diagnóstico quando não há
    /// registro do par.
    #[must_use]
    pub fn snapshot_or_never_seen(&self, node_id: &str) -> LinkSnapshot {
        self.snapshot(node_id)
            .unwrap_or_else(LinkSnapshot::never_seen)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<Entry>>> {
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
        links.mark_ready(
            &node,
            10,
            Some(7),
            "godmesh/1".to_owned(),
            "0.1.0".to_owned(),
            start,
        );
        assert_eq!(
            links.snapshot(&node).expect("enlace").state,
            LinkState::Ready
        );

        assert!(links.expire(start).is_empty());
        // Um heartbeat renova a evidência.
        links.beat(&node, 11, start + Duration::from_millis(20));
        assert!(links.expire(start + Duration::from_millis(40)).is_empty());

        // Silêncio maior que a janela: unknown.
        let expired = links.expire(start + Duration::from_millis(200));
        assert_eq!(expired, vec![node.clone()]);
        let snapshot = links.snapshot(&node).expect("enlace");
        assert_eq!(snapshot.state, LinkState::Unknown);
        assert_eq!(
            snapshot.rtt_ms, None,
            "medição viva não sobrevive ao enlace"
        );
        assert_eq!(
            snapshot.last_seen_at,
            Some(11),
            "última evidência é histórica"
        );

        // Passar mais tempo não promove nada.
        assert!(links.expire(start + Duration::from_secs(10)).is_empty());
        assert_eq!(
            links.snapshot(&node).expect("enlace").state,
            LinkState::Unknown
        );
    }

    #[test]
    fn rejected_is_terminal_for_the_attempt() {
        let links = PeerLinks::new(Duration::from_secs(1));
        let node = id(3);
        links.mark_rejected(&node);
        assert_eq!(
            links.snapshot(&node).expect("enlace").state,
            LinkState::Rejected
        );
        assert!(links.expire(Instant::now()).is_empty());
    }
}
