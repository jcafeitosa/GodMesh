use sqlx::{PgConnection, Row};

use super::{Error, Namespace, PolicyRef, Reservation};

macro_rules! namespace {
    ($sql:expr, $ns:expr) => {
        sqlx::query($sql)
            .bind(&$ns.origin_node)
            .bind(&$ns.tenant_id)
            .bind(&$ns.stream_id)
            .bind($ns.partition)
    };
}

macro_rules! series {
    ($sql:expr, $r:expr) => {
        namespace!($sql, $r.namespace).bind($r.authority_epoch)
    };
}

pub(super) async fn reservation(
    db: &mut PgConnection,
    ns: &Namespace,
) -> Result<Option<Reservation>, Error> {
    let row = namespace!(
        "SELECT * FROM federation_replica_namespace
         WHERE (origin_node, tenant_id, stream_id, partition)=($1,$2,$3,$4)",
        ns
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| {
        Ok(Reservation {
            namespace: ns.clone(),
            authority_epoch: r.try_get("authority_epoch")?,
            local_node: r.try_get("local_node")?,
            profile: r.try_get("profile")?,
            profile_version: r.try_get("profile_version")?,
            retention_class: r.try_get("retention_class")?,
            class_version: r.try_get("class_version")?,
            jurisdiction: r.try_get("jurisdiction")?,
            policy: PolicyRef {
                id: r.try_get("policy_id")?,
                version: r.try_get("policy_version")?,
            },
        })
    })
    .transpose()
}

pub(super) async fn insert_reservation(
    db: &mut PgConnection,
    r: &Reservation,
) -> Result<(), Error> {
    series!(
        "INSERT INTO federation_replica_namespace
         (origin_node,tenant_id,stream_id,partition,authority_epoch,local_node,profile,
          profile_version,retention_class,class_version,jurisdiction,policy_id,policy_version)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        r
    )
    .bind(&r.local_node)
    .bind(&r.profile)
    .bind(r.profile_version)
    .bind(&r.retention_class)
    .bind(r.class_version)
    .bind(&r.jurisdiction)
    .bind(&r.policy.id)
    .bind(r.policy.version)
    .execute(&mut *db)
    .await?;
    advance(db, r, 0).await
}

pub(super) async fn legacy_history(db: &mut PgConnection, ns: &Namespace) -> Result<bool, Error> {
    let row = namespace!(
        "SELECT EXISTS (
           SELECT 1 FROM event_position WHERE (origin_node,tenant_id,stream_id,partition)=($1,$2,$3,$4)
           UNION ALL
           SELECT 1 FROM sync_inbox_state WHERE (origin_node,tenant_id,stream_id,partition)=($1,$2,$3,$4)
           UNION ALL
           SELECT 1 FROM outbox_entry WHERE (origin_node,tenant_id,stream_id,partition)=($1,$2,$3,$4)
         ) AS found", ns
    ).fetch_one(db).await?;
    Ok(row.try_get("found")?)
}

pub(super) async fn origin_scope(
    db: &mut PgConnection,
    r: &Reservation,
    key: &str,
) -> Result<(), Error> {
    sqlx::query(
        "SELECT 1 FROM federation_peer p JOIN federation_peer_scope s USING(node_id)
         WHERE p.node_id=$1 AND p.public_key=$2 AND p.trust_state='authorized'
           AND s.tenant_id=$3 AND s.stream_id=$4 FOR SHARE OF p,s",
    )
    .bind(&r.namespace.origin_node)
    .bind(key)
    .bind(&r.namespace.tenant_id)
    .bind(&r.namespace.stream_id)
    .fetch_optional(db)
    .await?
    .ok_or(Error::Authority)?;
    Ok(())
}

pub(super) struct Policy {
    key: PolicyRef,
    duration: i64,
    pub allow_discard: bool,
    active: bool,
}

impl Policy {
    fn decode(row: sqlx::postgres::PgRow) -> Result<Self, Error> {
        Ok(Self {
            key: PolicyRef {
                id: row.try_get("policy_id")?,
                version: row.try_get("policy_version")?,
            },
            duration: row.try_get("duration_seconds")?,
            allow_discard: row.try_get("allow_discard")?,
            active: row.try_get("active")?,
        })
    }

    pub fn expiration(&self, issued: i64, now: i64) -> Result<i64, Error> {
        let expires = issued.checked_add(self.duration).ok_or(Error::Policy)?;
        if now >= expires {
            return Err(Error::RetentionExpired);
        }
        Ok(expires)
    }
}

pub(super) async fn historical_policy(
    db: &mut PgConnection,
    r: &Reservation,
    key: &PolicyRef,
) -> Result<Policy, Error> {
    if key != &r.policy {
        return Err(Error::ProofConflict);
    }
    let row = sqlx::query(
        "SELECT policy_id,policy_version,duration_seconds,allow_discard,active
         FROM federation_replica_policy WHERE (policy_id,policy_version) = ($1,$2)
         AND (tenant_id,retention_class,class_version,jurisdiction,copy_node,copy_kind)=($3,$4,$5,$6,$7,$8)
         FOR SHARE"
    ).bind(&key.id).bind(key.version).bind(&r.namespace.tenant_id).bind(&r.retention_class)
        .bind(r.class_version).bind(&r.jurisdiction).bind(&r.local_node)
        .bind(if r.primary() { "primary" } else { "secondary" })
        .fetch_optional(db).await?.ok_or(Error::Policy)?;
    Policy::decode(row)
}

pub(super) async fn current_policy(
    db: &mut PgConnection,
    r: &Reservation,
) -> Result<Policy, Error> {
    sqlx::query(
        "SELECT 1 FROM tenant_region t JOIN retention_class c USING(jurisdiction)
         WHERE (t.tenant_id,t.jurisdiction,c.class)=($1,$2,$3) FOR SHARE OF t,c",
    )
    .bind(&r.namespace.tenant_id)
    .bind(&r.jurisdiction)
    .bind(&r.retention_class)
    .fetch_optional(&mut *db)
    .await?
    .ok_or(Error::Policy)?;
    let policy = historical_policy(db, r, &r.policy).await?;
    if !policy.active {
        return Err(Error::Policy);
    }
    Ok(policy)
}

pub(super) async fn copy_policy(
    db: &mut PgConnection,
    r: &Reservation,
    dest: Option<&str>,
    now: i64,
) -> Result<Policy, Error> {
    let row = series!(
        "SELECT p.policy_id,p.policy_version,p.duration_seconds,p.allow_discard,p.active
         FROM federation_replica_acl a JOIN federation_replica_policy p USING(policy_id,policy_version)
         WHERE (a.origin_node,a.tenant_id,a.stream_id,a.partition,a.authority_epoch)=($1,$2,$3,$4,$5)
           AND ($6::text IS NULL OR a.destination=$6)
           AND (a.direction,a.retention_class,a.class_version,a.jurisdiction)=($7,$8,$9,$10)
           AND NOT a.revoked AND a.not_before <= $11 AND $11 < a.expires_at AND p.active
           AND (p.tenant_id,p.retention_class,p.class_version,p.jurisdiction,p.copy_node,p.copy_kind)
               =(a.tenant_id,a.retention_class,a.class_version,a.jurisdiction,a.destination,a.copy_kind)
           AND ($7='export' OR (p.policy_id,p.policy_version)=($12,$13))
         ORDER BY a.destination LIMIT 1 FOR SHARE OF a,p", r
    ).bind(dest).bind(if r.primary() { "export" } else { "import" })
        .bind(&r.retention_class).bind(r.class_version).bind(&r.jurisdiction).bind(now)
        .bind(&r.policy.id).bind(r.policy.version)
        .fetch_optional(db).await?.ok_or(Error::AclDenied)?;
    Policy::decode(row)
}

pub(super) async fn prefix(db: &mut PgConnection, r: &Reservation) -> Result<i64, Error> {
    let row = series!(
        "SELECT prefix FROM federation_replica_checkpoint
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,local_node)=($1,$2,$3,$4,$5,$6)", r
    ).bind(&r.local_node).fetch_optional(db).await?;
    Ok(row
        .map(|row| row.try_get("prefix"))
        .transpose()?
        .unwrap_or(0))
}

pub(super) async fn advance(
    db: &mut PgConnection,
    r: &Reservation,
    prefix: i64,
) -> Result<(), Error> {
    series!(
        "INSERT INTO federation_replica_checkpoint (origin_node,tenant_id,stream_id,partition,authority_epoch,local_node,prefix)
         VALUES ($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)
         DO UPDATE SET prefix=excluded.prefix WHERE federation_replica_checkpoint.prefix < excluded.prefix", r
    ).bind(&r.local_node).bind(prefix).execute(db).await?;
    Ok(())
}

pub(super) enum Slot {
    Empty,
    Discarded,
    Proof(Proof),
}

pub(super) struct Proof {
    pub frame: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub policy: PolicyRef,
    pub published: bool,
    discard_allowed: bool,
}

impl Proof {
    pub fn check_policy(&self, policy: &Policy) -> Result<(), Error> {
        if self.policy != policy.key
            || self.discard_allowed != policy.allow_discard
            || self.issued_at.checked_add(policy.duration) != Some(self.expires_at)
        {
            return Err(Error::ProofConflict);
        }
        Ok(())
    }
}

pub(super) async fn slot(db: &mut PgConnection, r: &Reservation, seq: i64) -> Result<Slot, Error> {
    let removed = series!(
        "SELECT 1 FROM federation_replica_disposition
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,seq)=($1,$2,$3,$4,$5,$6)",
        r
    )
    .bind(seq)
    .fetch_optional(&mut *db)
    .await?;
    if removed.is_some() {
        return Ok(Slot::Discarded);
    }

    let row = series!(
        "SELECT e.retention_class,e.jurisdiction,p.frame,p.issued_at,p.expires_at,
                p.policy_id,p.policy_version,p.published,p.discard_allowed
         FROM event_position e LEFT JOIN federation_replica_proof p
           USING(origin_node,tenant_id,stream_id,partition,authority_epoch,seq)
         WHERE (e.origin_node,e.tenant_id,e.stream_id,e.partition,e.authority_epoch,e.seq)=($1,$2,$3,$4,$5,$6)", r
    ).bind(seq).fetch_optional(db).await?;
    let Some(row) = row else {
        return Ok(Slot::Empty);
    };
    let Some(frame) = row.try_get::<Option<Vec<u8>>, _>("frame")? else {
        return Err(Error::ProofMissing);
    };
    if row.try_get::<&str, _>("retention_class")? != r.retention_class
        || row.try_get::<&str, _>("jurisdiction")? != r.jurisdiction
    {
        return Err(Error::ProofConflict);
    }
    Ok(Slot::Proof(Proof {
        frame,
        issued_at: row.try_get("issued_at")?,
        expires_at: row.try_get("expires_at")?,
        policy: PolicyRef {
            id: row.try_get("policy_id")?,
            version: row.try_get("policy_version")?,
        },
        published: row.try_get("published")?,
        discard_allowed: row.try_get("discard_allowed")?,
    }))
}

pub(super) async fn insert_proof(
    db: &mut PgConnection,
    r: &Reservation,
    policy: &Policy,
    seq: i64,
    issued: i64,
    expires: i64,
    frame: &[u8],
) -> Result<(), Error> {
    series!(
        "INSERT INTO event_position (origin_node,tenant_id,stream_id,partition,authority_epoch,seq,retention_class,jurisdiction)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)", r
    ).bind(seq).bind(&r.retention_class).bind(&r.jurisdiction).execute(&mut *db).await?;
    series!(
        "INSERT INTO federation_replica_proof
         (origin_node,tenant_id,stream_id,partition,authority_epoch,seq,frame,issued_at,expires_at,
          policy_id,policy_version,published,discard_allowed)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        r
    )
    .bind(seq)
    .bind(frame)
    .bind(issued)
    .bind(expires)
    .bind(&policy.key.id)
    .bind(policy.key.version)
    .bind(r.primary())
    .bind(policy.allow_discard)
    .execute(db)
    .await?;
    Ok(())
}

pub(super) async fn hold(
    db: &mut PgConnection,
    r: &Reservation,
    seq: i64,
) -> Result<Option<bool>, Error> {
    let row = series!(
        "SELECT active FROM federation_replica_hold
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,seq)=($1,$2,$3,$4,$5,$6)
         FOR SHARE",
        r
    )
    .bind(seq)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|row| row.try_get("active")).transpose()?)
}

pub(super) async fn remove(
    db: &mut PgConnection,
    r: &Reservation,
    seq: i64,
    now: i64,
) -> Result<(), Error> {
    series!(
        "DELETE FROM federation_replica_proof
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,seq)=($1,$2,$3,$4,$5,$6)",
        r
    )
    .bind(seq)
    .execute(&mut *db)
    .await?;
    series!(
        "DELETE FROM event_position
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,seq)=($1,$2,$3,$4,$5,$6)",
        r
    )
    .bind(seq)
    .execute(&mut *db)
    .await?;
    series!(
        "DELETE FROM federation_replica_hold
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,seq)=($1,$2,$3,$4,$5,$6)",
        r
    )
    .bind(seq)
    .execute(&mut *db)
    .await?;
    series!(
        "INSERT INTO federation_replica_disposition (origin_node,tenant_id,stream_id,partition,authority_epoch,seq,removed_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)", r
    ).bind(seq).bind(now).execute(db).await?;
    Ok(())
}
