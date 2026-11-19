//! M4-A1. Serviço interno de metadados. O chamador mantém o guard M1C,
//! fornece identidades da fronteira confiável e confirma a transação externa.
//! Os adaptadores A2 devem verificar o vínculo M1C, o piso M3 e a sessão
//! autenticada na mesma transação, antes de expor qualquer resultado.
//! O relógio é avaliado uma vez, depois do lock de namespace.

#[cfg(test)]
pub(crate) mod snapshot_read;
mod store;

use godmesh_federation::{SignedEvent, identity};
use godmesh_identity::grant::GrantDecision;
use serde::Deserialize;
use serde_json::value::RawValue;
use sqlx::PgConnection;

use crate::{
    auth::{Scope, VerifiedGrant},
    events::lock_event_namespace,
    portable_join::ClusterGuard,
};
use store::{Policy, Slot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Namespace {
    pub origin_node: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRef {
    pub id: String,
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub namespace: Namespace,
    pub authority_epoch: i64,
    pub local_node: String,
    pub profile: String,
    pub profile_version: i64,
    pub retention_class: String,
    pub class_version: i64,
    pub jurisdiction: String,
    pub policy: PolicyRef,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeContext<'a> {
    pub node_id: &'a str,
    pub public_key: &'a str,
}

impl NodeContext<'_> {
    fn check(self, expected: &str) -> Result<(), Error> {
        if self.node_id != expected || !identity::verify_node_id(self.node_id, self.public_key) {
            return Err(Error::Authority);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    max_events: usize,
    max_envelope_bytes: usize,
    max_data_bytes: usize,
}

impl Limits {
    pub fn new(
        max_events: usize,
        max_envelope_bytes: usize,
        max_data_bytes: usize,
    ) -> Result<Self, Error> {
        if max_events == 0 || max_envelope_bytes == 0 || max_data_bytes == 0 {
            return Err(Error::LimitExceeded);
        }
        Ok(Self {
            max_events,
            max_envelope_bytes,
            max_data_bytes,
        })
    }

    pub(crate) fn component_min(self, other: Self) -> Self {
        Self {
            max_events: self.max_events.min(other.max_events),
            max_envelope_bytes: self.max_envelope_bytes.min(other.max_envelope_bytes),
            max_data_bytes: self.max_data_bytes.min(other.max_data_bytes),
        }
    }

    fn check_batch(self, frames: &[Vec<u8>]) -> Result<(), Error> {
        if frames.is_empty() {
            return Err(Error::InvalidProof);
        }
        if frames.len() > self.max_events {
            return Err(Error::LimitExceeded);
        }
        let mut bytes = 0usize;
        for frame in frames {
            bytes = bytes.checked_add(frame.len()).ok_or(Error::LimitExceeded)?;
            if frame.len() > self.max_envelope_bytes || bytes > self.max_data_bytes {
                return Err(Error::LimitExceeded);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidReservation,
    NotReserved,
    ReservationConflict,
    LegacyHistory,
    Authority,
    Policy,
    AclDenied,
    InvalidProof,
    NotFresh,
    RetentionExpired,
    ProofMissing,
    ProofConflict,
    SequenceGap,
    HistoryUnavailable,
    CheckpointMissing,
    LimitExceeded,
    HoldActive,
    HoldUnknown,
    DiscardDenied,
    Storage(sqlx::Error),
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reserved {
    Created,
    Replay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposal {
    Removed,
    AlreadyRemoved,
    NotDue,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CopyPage {
    pub frames: Vec<Vec<u8>>,
    pub has_more: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Applied {
    pub first: i64,
    pub last: i64,
    pub prefix: i64,
    pub inserted: usize,
}

#[derive(Clone, Copy)]
pub struct OriginAdmission<'a> {
    pub origin: NodeContext<'a>,
    pub grant: &'a VerifiedGrant,
    pub max_age_seconds: i64,
    pub limits: Limits,
}

#[derive(Clone, Copy)]
pub struct CopyAuthority<'a> {
    pub origin: NodeContext<'a>,
    pub destination: NodeContext<'a>,
    pub limits: Limits,
}

#[derive(Clone, Copy)]
pub struct Series<'a> {
    pub namespace: &'a Namespace,
    pub epoch: i64,
}

/// A1 prepara resultados. O chamador só pode anunciar persistência depois do COMMIT.
pub struct Replication;

impl Replication {
    /// Confere somente a reserva estática para o preflight. Política corrente
    /// e ACL são decisões de cada efeito.
    pub async fn verify_reservation_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        local: NodeContext<'_>,
        primary: bool,
    ) -> Result<(), Error> {
        let conn = locked(guard, series.namespace).await?;
        let reservation = load_series(conn, series).await?;
        reservation.validate()?;
        local.check(&reservation.local_node)?;
        if reservation.primary() != primary {
            return Err(Error::Authority);
        }
        Ok(())
    }

    /// Leitura estrita do cursor no destino. A ausência indica estado
    /// inconsistente e jamais é convertida em prefixo inicial.
    pub async fn receiver_checkpoint_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        local: NodeContext<'_>,
    ) -> Result<i64, Error> {
        let conn = locked(guard, series.namespace).await?;
        let reservation = load_series(conn, series).await?;
        local.check(&reservation.local_node)?;
        if reservation.primary() {
            return Err(Error::Authority);
        }
        store::checkpoint(conn, &reservation)
            .await?
            .ok_or(Error::CheckpointMissing)
    }

    /// Revalida a autorização e residência sem selecionar nem aplicar lote.
    pub(crate) async fn recheck_copy_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        authority: CopyAuthority<'_>,
        primary: bool,
        now: i64,
    ) -> Result<(), Error> {
        let conn = locked(guard, series.namespace).await?;
        let reservation = load_series(conn, series).await?;
        authority.origin.check(&reservation.namespace.origin_node)?;
        authority.destination.check(if primary {
            authority.destination.node_id
        } else {
            &reservation.local_node
        })?;
        if reservation.primary() != primary {
            return Err(Error::Authority);
        }
        store::current_policy(conn, &reservation).await?;
        store::copy_policy(conn, &reservation, Some(authority.destination.node_id), now).await?;
        Ok(())
    }

    pub async fn reserve_namespace_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        request: &Reservation,
        local: NodeContext<'_>,
        clock: impl FnOnce() -> i64,
    ) -> Result<Reserved, Error> {
        let conn = locked(guard, &request.namespace).await?;
        if let Some(stored) = store::reservation(conn, &request.namespace).await? {
            return if stored == *request {
                Ok(Reserved::Replay)
            } else {
                Err(Error::ReservationConflict)
            };
        }
        let now = clock();
        request.validate()?;
        local.check(&request.local_node)?;
        store::current_policy(conn, request).await?;
        let primary = request.primary();
        store::copy_policy(
            conn,
            request,
            (!primary).then_some(request.local_node.as_str()),
            now,
        )
        .await?;
        if store::legacy_history(conn, &request.namespace).await? {
            return Err(Error::LegacyHistory);
        }
        store::insert_reservation(conn, request).await?;
        Ok(Reserved::Created)
    }

    pub async fn admit_origin_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        admission: OriginAdmission<'_>,
        frame: &[u8],
        clock: impl FnOnce() -> i64,
    ) -> Result<Applied, Error> {
        if frame.len() > admission.limits.max_envelope_bytes
            || frame.len() > admission.limits.max_data_bytes
        {
            return Err(Error::LimitExceeded);
        }
        let conn = locked(guard, series.namespace).await?;
        let now = clock();
        let reservation = load_series(conn, series).await?;
        if !reservation.primary()
            || admission.grant.validate().is_err()
            || admission.grant.predicate.evaluate() != GrantDecision::Allow
            || !admission.grant.has_scope(Scope::EventWrite)
            || admission.grant.subject.tenant_id != series.namespace.tenant_id
        {
            return Err(Error::Authority);
        }
        admission.origin.check(&series.namespace.origin_node)?;
        store::origin_scope(conn, &reservation, admission.origin.public_key).await?;
        let policy = store::current_policy(conn, &reservation).await?;
        let event = verify(frame, &reservation, admission.origin)?;
        let seq = sequence(&event)?;
        let prefix = store::prefix(conn, &reservation).await?;
        if seq <= prefix {
            confirm_replay(conn, &reservation, &policy, &event, seq, now).await?;
            return Ok(Applied {
                first: seq,
                last: seq,
                prefix,
                inserted: 0,
            });
        }
        if seq != next(prefix)? {
            return Err(Error::SequenceGap);
        }
        if admission.max_age_seconds <= 0
            || event.verify_fresh(now, admission.max_age_seconds).is_err()
        {
            return Err(Error::NotFresh);
        }
        let expiry = policy.expiration(event.issued_at, now)?;
        require_empty(conn, &reservation, seq).await?;
        store::insert_proof(
            conn,
            &reservation,
            &policy,
            seq,
            event.issued_at,
            expiry,
            frame,
        )
        .await?;
        store::advance(conn, &reservation, seq).await?;
        Ok(Applied {
            first: seq,
            last: seq,
            prefix: seq,
            inserted: 1,
        })
    }

    pub async fn select_copy_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        authority: CopyAuthority<'_>,
        first: i64,
        clock: impl FnOnce() -> i64,
    ) -> Result<CopyPage, Error> {
        if first <= 0 {
            return Err(Error::SequenceGap);
        }
        let conn = locked(guard, series.namespace).await?;
        let now = clock();
        let reservation = load_series(conn, series).await?;
        authority.origin.check(&reservation.namespace.origin_node)?;
        authority.destination.check(authority.destination.node_id)?;
        if !reservation.primary() {
            return Err(Error::Authority);
        }
        let primary = store::current_policy(conn, &reservation).await?;
        let secondary =
            store::copy_policy(conn, &reservation, Some(authority.destination.node_id), now)
                .await?;
        let prefix = store::prefix(conn, &reservation).await?;
        if first > prefix && Some(first) != prefix.checked_add(1) {
            return Err(Error::SequenceGap);
        }
        let mut frames = Vec::new();
        let mut bytes = 0usize;
        for seq in first..=prefix {
            if frames.len() == authority.limits.max_events {
                return Ok(CopyPage {
                    frames,
                    has_more: true,
                });
            }
            let proof = match usable(conn, &reservation, &primary, seq, now).await {
                Err(Error::HistoryUnavailable) if !frames.is_empty() => {
                    return Ok(CopyPage {
                        frames,
                        has_more: true,
                    });
                }
                result => result?,
            };
            let combined = bytes
                .checked_add(proof.frame.len())
                .ok_or(Error::LimitExceeded)?;
            if proof.frame.len() > authority.limits.max_envelope_bytes
                || combined > authority.limits.max_data_bytes
            {
                return if frames.is_empty() {
                    Err(Error::LimitExceeded)
                } else {
                    Ok(CopyPage {
                        frames,
                        has_more: true,
                    })
                };
            }
            verify_local_proof(&proof, &reservation, authority.origin, seq, true)?;
            match secondary.expiration(proof.issued_at, now) {
                Err(Error::RetentionExpired) if !frames.is_empty() => {
                    return Ok(CopyPage {
                        frames,
                        has_more: true,
                    });
                }
                Err(Error::RetentionExpired) => return Err(Error::HistoryUnavailable),
                result => {
                    result?;
                }
            }
            bytes = combined;
            frames.push(proof.frame);
        }
        Ok(CopyPage {
            frames,
            has_more: false,
        })
    }

    pub async fn apply_copy_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        authority: CopyAuthority<'_>,
        frames: &[Vec<u8>],
        clock: impl FnOnce() -> i64,
    ) -> Result<Applied, Error> {
        authority.limits.check_batch(frames)?;
        let conn = locked(guard, series.namespace).await?;
        let now = clock();
        let reservation = load_series(conn, series).await?;
        authority.origin.check(&reservation.namespace.origin_node)?;
        authority.destination.check(&reservation.local_node)?;
        if reservation.primary() {
            return Err(Error::Authority);
        }
        let policy = store::current_policy(conn, &reservation).await?;
        store::copy_policy(conn, &reservation, Some(authority.destination.node_id), now).await?;
        let mut prefix = store::prefix(conn, &reservation).await?;
        let mut previous = None;
        let mut first = 0;
        let mut pending = Vec::new();
        for raw in frames {
            let event = verify(raw, &reservation, authority.origin)?;
            let seq = sequence(&event)?;
            if let Some(before) = previous {
                if seq != next(before)? {
                    return Err(Error::SequenceGap);
                }
            } else {
                first = seq;
            }
            previous = Some(seq);
            if seq <= prefix {
                confirm_replay(conn, &reservation, &policy, &event, seq, now).await?;
                continue;
            }
            if seq != next(prefix)? {
                return Err(Error::SequenceGap);
            }
            let expiry = policy.expiration(event.issued_at, now)?;
            require_empty(conn, &reservation, seq).await?;
            pending.push((seq, event.issued_at, expiry, raw));
            prefix = seq;
        }
        for (seq, issued, expiry, raw) in &pending {
            store::insert_proof(conn, &reservation, &policy, *seq, *issued, *expiry, raw).await?;
        }
        if !pending.is_empty() {
            store::advance(conn, &reservation, prefix).await?;
        }
        Ok(Applied {
            first,
            last: previous.ok_or(Error::InvalidProof)?,
            prefix,
            inserted: pending.len(),
        })
    }

    pub async fn discard_due_in_tx(
        guard: &mut ClusterGuard<'_, '_>,
        series: Series<'_>,
        local: NodeContext<'_>,
        seq: i64,
        clock: impl FnOnce() -> i64,
    ) -> Result<Disposal, Error> {
        if seq <= 0 {
            return Err(Error::SequenceGap);
        }
        let conn = locked(guard, series.namespace).await?;
        let now = clock();
        let reservation = load_series(conn, series).await?;
        local.check(&reservation.local_node)?;
        let proof = match store::slot(conn, &reservation, seq).await? {
            Slot::Discarded => return Ok(Disposal::AlreadyRemoved),
            Slot::Empty => return Err(Error::SequenceGap),
            Slot::Proof(proof) => proof,
        };
        let policy = store::historical_policy(conn, &reservation, &proof.policy).await?;
        proof.check_policy(&policy)?;
        let stored = parse(&proof.frame).map_err(|_| Error::ProofConflict)?;
        if stored.issued_at != proof.issued_at
            || i64::try_from(stored.position.seq) != Ok(seq)
            || proof.published != reservation.primary()
        {
            return Err(Error::ProofConflict);
        }
        if now < proof.expires_at {
            return Ok(Disposal::NotDue);
        }
        if !policy.allow_discard {
            return Err(Error::DiscardDenied);
        }
        match store::hold(conn, &reservation, seq).await? {
            Some(false) => {}
            Some(true) => return Err(Error::HoldActive),
            None => return Err(Error::HoldUnknown),
        }
        store::remove(conn, &reservation, seq, now).await?;
        Ok(Disposal::Removed)
    }
}

impl Reservation {
    fn primary(&self) -> bool {
        self.local_node == self.namespace.origin_node
    }

    fn validate(&self) -> Result<(), Error> {
        if !identity::is_node_id(&self.local_node)
            || self.authority_epoch <= 0
            || self.profile != "metadata"
            || self.profile_version != 1
            || self.retention_class.trim().is_empty()
            || self.class_version <= 0
            || !matches!(self.jurisdiction.as_str(), "BR" | "CO")
            || self.policy.id.trim().is_empty()
            || self.policy.version <= 0
        {
            return Err(Error::InvalidReservation);
        }
        Ok(())
    }
}

pub(crate) async fn locked<'a>(
    guard: &'a mut ClusterGuard<'_, '_>,
    ns: &Namespace,
) -> Result<&'a mut PgConnection, Error> {
    if ns.partition != 0
        || !identity::is_node_id(&ns.origin_node)
        || ns.tenant_id.trim().is_empty()
        || !ns
            .stream_id
            .strip_prefix("m4a.metadata.")
            .is_some_and(|rest| !rest.trim().is_empty())
    {
        return Err(Error::InvalidReservation);
    }
    let connection = guard.connection();
    crate::sql_budget::refresh(connection).await?;
    lock_event_namespace(
        connection,
        &ns.origin_node,
        &ns.tenant_id,
        &ns.stream_id,
        ns.partition,
    )
    .await?;
    Ok(connection)
}

async fn load_series(conn: &mut PgConnection, series: Series<'_>) -> Result<Reservation, Error> {
    let reservation = store::reservation(conn, series.namespace)
        .await?
        .ok_or(Error::NotReserved)?;
    if reservation.authority_epoch != series.epoch {
        return Err(Error::ReservationConflict);
    }
    Ok(reservation)
}

fn next(seq: i64) -> Result<i64, Error> {
    seq.checked_add(1).ok_or(Error::SequenceGap)
}

fn sequence(event: &SignedEvent) -> Result<i64, Error> {
    i64::try_from(event.position.seq)
        .ok()
        .filter(|seq| *seq > 0)
        .ok_or(Error::InvalidProof)
}

fn parse(frame: &[u8]) -> Result<SignedEvent, Error> {
    #[derive(Deserialize)]
    struct Shape<'a> {
        #[serde(borrow)]
        position: &'a RawValue,
    }
    let object = |raw: &[u8]| raw.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{');
    if !object(frame) {
        return Err(Error::InvalidProof);
    }
    let shape: Shape<'_> = serde_json::from_slice(frame).map_err(|_| Error::InvalidProof)?;
    if !object(shape.position.get().as_bytes()) {
        return Err(Error::InvalidProof);
    }
    serde_json::from_slice(frame).map_err(|_| Error::InvalidProof)
}

fn verify(
    frame: &[u8],
    reservation: &Reservation,
    origin: NodeContext<'_>,
) -> Result<SignedEvent, Error> {
    let event = parse(frame)?;
    event.validate_fields().map_err(|_| Error::InvalidProof)?;
    let ns = &reservation.namespace;
    if event.position.origin_node != ns.origin_node
        || event.node_id != origin.node_id
        || event.tenant_id != ns.tenant_id
        || event.position.stream_id != ns.stream_id
        || event.position.partition != ns.partition as u32
        || event.position.authority_epoch != reservation.authority_epoch as u64
        || event.retention_class != reservation.retention_class
        || event.verify_origin_is_signer().is_err()
        || event.verify_signature_against(origin.public_key).is_err()
    {
        return Err(Error::InvalidProof);
    }
    sequence(&event)?;
    Ok(event)
}

fn verify_local_proof(
    proof: &store::Proof,
    r: &Reservation,
    origin: NodeContext<'_>,
    seq: i64,
    published: bool,
) -> Result<(), Error> {
    let event = verify(&proof.frame, r, origin).map_err(|_| Error::ProofConflict)?;
    if sequence(&event)? != seq
        || event.issued_at != proof.issued_at
        || proof.published != published
    {
        return Err(Error::ProofConflict);
    }
    Ok(())
}

async fn usable(
    conn: &mut PgConnection,
    reservation: &Reservation,
    policy: &Policy,
    seq: i64,
    now: i64,
) -> Result<store::Proof, Error> {
    match store::slot(conn, reservation, seq).await? {
        Slot::Discarded => Err(Error::HistoryUnavailable),
        Slot::Empty => Err(Error::SequenceGap),
        Slot::Proof(proof) => {
            proof.check_policy(policy)?;
            if now >= proof.expires_at {
                return Err(Error::HistoryUnavailable);
            }
            Ok(proof)
        }
    }
}

async fn require_empty(
    conn: &mut PgConnection,
    reservation: &Reservation,
    seq: i64,
) -> Result<(), Error> {
    match store::slot(conn, reservation, seq).await? {
        Slot::Empty => Ok(()),
        Slot::Discarded => Err(Error::HistoryUnavailable),
        Slot::Proof(_) => Err(Error::ProofConflict),
    }
}

async fn confirm_replay(
    conn: &mut PgConnection,
    reservation: &Reservation,
    policy: &Policy,
    event: &SignedEvent,
    seq: i64,
    now: i64,
) -> Result<(), Error> {
    let proof = usable(conn, reservation, policy, seq, now).await?;
    let stored = parse(&proof.frame).map_err(|_| Error::ProofConflict)?;
    if proof.published != reservation.primary()
        || stored.canonical_bytes() != event.canonical_bytes()
        || hex::decode(&stored.signature).map_err(|_| Error::ProofConflict)?
            != hex::decode(&event.signature).map_err(|_| Error::InvalidProof)?
        || stored.issued_at != proof.issued_at
    {
        return Err(Error::ProofConflict);
    }
    Ok(())
}
