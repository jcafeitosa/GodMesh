//! Integração do snapshot no executável descartável de testes.

use std::num::{NonZeroU32, NonZeroUsize};

use godmesh_federation::node::SnapshotSessionLimits;
use godmesh_federation::snapshot::{
    SnapshotIdentity, SnapshotLimits, SnapshotPolicy, verify_snapshot,
};
use godmesh_federation::snapshot_wire::{
    self as wire, Ack, AckClass, Control, ErrorCode, ErrorMessage, Namespace, Request, Response,
};
use godmesh_server::replica_operational::snapshot_harness::{
    EligibleReceiver, HarnessHistory, Recovery, SnapshotEffectError, Window, capture,
};
use sha2::{Digest, Sha256};

use super::*;
use crate::snapshot_lab::{Fault, LabContext};

pub(super) struct SnapshotRuntime {
    context: LabContext,
    signer: Arc<NodeIdentity>,
    receiver: Option<EligibleReceiver>,
}

impl ReplicaRuntime {
    pub(crate) async fn with_snapshot(
        mut self,
        context: LabContext,
        signer: Arc<NodeIdentity>,
    ) -> Result<Self, String> {
        let config = self.service.config();
        if context.node_id != signer.node_id()
            || context.origin_node != config.origin_pin.node_id
            || context.cluster != config.cluster
            || context.tenant_id != config.namespace.tenant_id
            || context.stream_id != config.namespace.stream_id
            || i64::from(context.partition) != i64::from(config.namespace.partition)
            || context.authority_epoch != config.authority_epoch
            || (context.bootstrap_base.is_some() != (config.role == Role::Receiver))
        {
            return Err("contexto snapshot diverge da réplica configurada".to_owned());
        }
        let receiver = if config.role == Role::Receiver {
            let history = HarnessHistory {
                database: context.database.clone(),
                generation: context.generation,
            };
            let deadline = Instant::now() + config.attempt_timeout;
            Some(
                EligibleReceiver::new(Arc::clone(&self.service), history, deadline)
                    .await
                    .map_err(|error| format!("receptor snapshot inelegível: {error:?}"))?,
            )
        } else {
            None
        };
        self.snapshot = Some(Arc::new(SnapshotRuntime {
            context,
            signer,
            receiver,
        }));
        Ok(self)
    }
}

impl SnapshotRuntime {
    pub(super) fn session_limits(&self, a2: SessionLimits) -> SnapshotSessionLimits {
        let context = &self.context;
        SnapshotSessionLimits {
            a2,
            max_session_frame_bytes: a2
                .max_frame_bytes
                .max(context.max_control_bytes)
                .max(context.max_snapshot_bytes),
            max_control_bytes: context.max_control_bytes,
            max_snapshot_bytes: context.max_snapshot_bytes,
        }
    }

    fn namespace(&self) -> Namespace {
        let context = &self.context;
        Namespace {
            cluster: context.cluster.clone(),
            tenant_id: context.tenant_id.clone(),
            stream_id: context.stream_id.clone(),
            partition: context.partition,
            authority_epoch: context.authority_epoch as u64,
        }
    }

    fn policy(&self, remote: &Remote, max_package: NonZeroU32) -> SnapshotPolicy {
        let context = &self.context;
        let local_identity = SnapshotIdentity {
            node_id: context.node_id.clone(),
            public_key: self.signer.public_key_hex(),
        };
        let remote_identity = SnapshotIdentity {
            node_id: remote.node_id.clone(),
            public_key: remote.key.clone(),
        };
        let (origin, destination) = if self.receiver.is_some() {
            (remote_identity, local_identity)
        } else {
            (local_identity, remote_identity)
        };
        SnapshotPolicy {
            cluster: context.cluster.clone(),
            origin,
            destination,
            tenant_id: context.tenant_id.clone(),
            stream_id: context.stream_id.clone(),
            partition: context.partition,
            authority_epoch: context.authority_epoch as u64,
            retention_class: context.retention_class.clone(),
            limits: SnapshotLimits {
                max_package_bytes: NonZeroUsize::new(max_package.get() as usize)
                    .expect("teto positivo representável no host"),
                max_manifest_bytes: context.max_manifest_bytes,
                max_event_bytes: context.max_event_bytes,
                max_events: context.max_events,
            },
        }
    }

    pub(super) async fn serve(
        &self,
        service: &ReplicaService,
        remote: &Remote,
        stream: &mut OperationalBiStream,
        body: &[u8],
    ) -> Result<(), AttemptError> {
        let context = &self.context;
        let deadline = stream.deadline();
        let Control::Request(request) =
            wire::decode_control(body, context.max_control_bytes).map_err(protocol_error)?
        else {
            return Err(AttemptError::Protocol);
        };
        if request.origin_node != context.node_id
            || request.destination_node != remote.node_id
            || request.namespace != self.namespace()
        {
            return self
                .reply_error(stream, request, ErrorCode::ScopeDenied)
                .await;
        }
        let max = context
            .max_snapshot_bytes
            .min(request.receiver_max_snapshot_bytes);
        let policy = self.policy(remote, max);
        let now = godmesh_federation::now_unix();
        let window = Window {
            not_before: now,
            expires_at: now
                .checked_add(i64::from(context.validity_seconds.get()))
                .ok_or(AttemptError::Protocol)?,
        };
        let package = match capture(
            service,
            &self.signer,
            remote.peer(),
            request.base_seq as i64,
            window,
            &policy,
            deadline,
        )
        .await
        {
            Ok(package) => package,
            Err(error @ SnapshotEffectError::Effect(EffectError::CommitUnknown(_))) => {
                return Err(effect_error(error));
            }
            Err(error) => {
                return self
                    .reply_error(stream, request, capture_error(error))
                    .await;
            }
        };
        let verified = verify_snapshot(&package, &policy).map_err(protocol_error)?;
        let response = Response {
            request_id: request.request_id.clone(),
            origin_node: request.origin_node.clone(),
            destination_node: request.destination_node.clone(),
            namespace: request.namespace.clone(),
            base_seq: request.base_seq,
            cut_seq: verified.cut().1,
            package_bytes: u32::try_from(package.len())
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or(AttemptError::Protocol)?,
            source_max_snapshot_bytes: context.max_snapshot_bytes,
            package_sha256: hex::encode(Sha256::digest(&package)),
        };
        let response_frame = wire::encode_control(
            &Control::Response(response.clone()),
            context.max_control_bytes,
        )
        .map_err(protocol_error)?;
        service.recheck(remote.peer(), deadline).await?;
        stream.write_snapshot_control(&response_frame).await?;
        stream
            .write_snapshot_body(&package, response.package_bytes, max)
            .await?;
        finish(stream, deadline)?;
        let ack_frame = stream.read_snapshot_control().await?;
        stream.expect_fin().await?;
        let Control::Ack(ack) =
            wire::decode_control(&ack_frame, context.max_control_bytes).map_err(protocol_error)?
        else {
            return Err(AttemptError::Protocol);
        };
        wire::validate_ack(&response, &ack).map_err(protocol_error)?;
        service.recheck(remote.peer(), deadline).await?;
        eprintln!(
            "event=m4_snapshot_ack ack_class=persisted_local destination={} cut={}",
            remote.node_id, ack.cut_seq
        );
        Ok(())
    }

    async fn reply_error(
        &self,
        stream: &mut OperationalBiStream,
        request: Request,
        error: ErrorCode,
    ) -> Result<(), AttemptError> {
        let body = wire::encode_control(
            &Control::Error(ErrorMessage {
                request_id: request.request_id,
                origin_node: request.origin_node,
                destination_node: request.destination_node,
                namespace: request.namespace,
                error,
            }),
            self.context.max_control_bytes,
        )
        .map_err(protocol_error)?;
        stream.write_snapshot_control(&body).await?;
        finish(stream, stream.deadline())?;
        stream.expect_fin().await?;
        Ok(())
    }

    pub(super) async fn receive(
        &self,
        runtime: &ReplicaRuntime,
        session: &OperationalSession,
        remote: &Remote,
        deadline: Instant,
    ) -> Result<Option<Cycle>, AttemptError> {
        let context = &self.context;
        let receiver = self.receiver.as_ref().ok_or(AttemptError::Protocol)?;
        let peer = remote.peer();
        let policy = self.policy(remote, context.max_snapshot_bytes);
        let base = match receiver
            .recover(peer, &policy, deadline)
            .await
            .map_err(effect_error)?
        {
            Recovery::Blocked { .. } => return Ok(Some(Cycle::Idle)),
            Recovery::Applied { after_seq } => {
                eprintln!("event=m4_snapshot_recovered cut={after_seq}");
                return Ok(Some(Cycle::More));
            }
            Recovery::Idle { after_seq } | Recovery::NoEffect { after_seq } => after_seq,
        };
        let configured_base = context.bootstrap_base.ok_or(AttemptError::Protocol)?;
        if base < configured_base {
            return Ok(Some(Cycle::Idle));
        }
        if base > configured_base {
            return Ok(None);
        }
        let request = Request {
            request_id: runtime.request_id()?,
            origin_node: context.origin_node.clone(),
            destination_node: context.node_id.clone(),
            namespace: self.namespace(),
            base_seq: base as u64,
            receiver_max_snapshot_bytes: context.max_snapshot_bytes,
        };
        let body = wire::encode_control(
            &Control::Request(request.clone()),
            context.max_control_bytes,
        )
        .map_err(protocol_error)?;
        let mut stream = session.open_bi(deadline).await?;
        stream.write_snapshot_opening(&body).await?;
        let response_frame = stream.read_snapshot_control().await?;
        let response = match wire::decode_control(&response_frame, context.max_control_bytes)
            .map_err(protocol_error)?
        {
            Control::Response(response) => response,
            Control::Error(error) => {
                wire::validate_error(&request, &error).map_err(protocol_error)?;
                stream.expect_fin().await?;
                finish(&mut stream, deadline)?;
                return Err(AttemptError::Remote);
            }
            _ => return Err(AttemptError::Protocol),
        };
        let max = wire::validate_response(&request, &response, context.max_snapshot_bytes)
            .map_err(protocol_error)?;
        let package = stream
            .read_snapshot_body(response.package_bytes, max)
            .await?;
        stream.expect_fin().await?;
        wire::validate_package(&request, &response, &package, context.max_snapshot_bytes)
            .map_err(protocol_error)?;
        let policy = self.policy(remote, max);
        let verified = verify_snapshot(&package, &policy).map_err(protocol_error)?;
        wire::validate_manifest(&response, &verified).map_err(protocol_error)?;
        let token = receiver
            .intend(peer, &package, &policy, deadline)
            .await
            .map_err(effect_error)?;
        eprintln!("event=m4_snapshot_intent attempt={}", token.get());
        if context.fault == Some(Fault::AfterIntent) {
            std::future::pending::<()>().await;
        }
        let application = receiver
            .apply(peer, token, &package, &policy, deadline)
            .await;
        if context.fault == Some(Fault::AfterCommitUnknown)
            && matches!(
                &application,
                Err(SnapshotEffectError::Effect(EffectError::CommitUnknown(_)))
            )
        {
            eprintln!("event=m4_snapshot_commit_unknown attempt={}", token.get());
            std::future::pending::<()>().await;
        }
        let applied = application.map_err(effect_error)?;
        if applied.replay || i64::try_from(response.cut_seq) != Ok(applied.through) {
            return Err(AttemptError::Protocol);
        }
        eprintln!("event=m4_snapshot_committed cut={}", applied.through);
        if context.fault == Some(Fault::AfterCommit) {
            std::future::pending::<()>().await;
        }
        let ack = Ack {
            request_id: response.request_id,
            origin_node: response.origin_node,
            destination_node: response.destination_node,
            namespace: response.namespace,
            cut_seq: response.cut_seq,
            package_sha256: response.package_sha256,
            ack_class: AckClass::PersistedLocal,
        };
        let body = wire::encode_control(&Control::Ack(ack), context.max_control_bytes)
            .map_err(protocol_error)?;
        stream.write_snapshot_control(&body).await?;
        finish(&mut stream, deadline)?;
        Ok(Some(Cycle::More))
    }
}

fn protocol_error(_: impl std::error::Error) -> AttemptError {
    AttemptError::Protocol
}

fn effect_error(error: SnapshotEffectError) -> AttemptError {
    match error {
        SnapshotEffectError::Effect(error) => error.into(),
        _ => AttemptError::Protocol,
    }
}

fn capture_error(error: SnapshotEffectError) -> ErrorCode {
    match error {
        SnapshotEffectError::Effect(error) => match error_on_wire(&error) {
            Some(code) => match code {
                wire_a2::AuthorityBlocked => ErrorCode::AuthorityBlocked,
                wire_a2::ResidenceDenied => ErrorCode::ResidenceDenied,
                wire_a2::HistoryUnavailable => ErrorCode::HistoryUnavailable,
                wire_a2::SequenceGap => ErrorCode::SequenceGap,
                wire_a2::ProofMissing => ErrorCode::ProofMissing,
                wire_a2::ProofConflict => ErrorCode::ProofConflict,
                wire_a2::LimitExceeded => ErrorCode::LimitExceeded,
                wire_a2::StorageUnavailable => ErrorCode::StorageUnavailable,
                wire_a2::ScopeDenied => ErrorCode::ScopeDenied,
            },
            None => ErrorCode::SnapshotConflict,
        },
        SnapshotEffectError::Scope => ErrorCode::ScopeDenied,
        SnapshotEffectError::LimitExceeded => ErrorCode::LimitExceeded,
        SnapshotEffectError::Window => ErrorCode::AuthorityBlocked,
        _ => ErrorCode::SnapshotConflict,
    }
}

use godmesh_federation::m4a_wire::ErrorCode as wire_a2;
