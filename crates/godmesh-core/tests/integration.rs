//! Integração entre os contratos do núcleo. Cada teste atravessa pelo menos duas
//! fronteiras: emergência, identidade, política, protocolo e sincronização.
//!
//! Regra de ouro: nenhuma operação sensível começa sem autoridade e sem política
//! de retenção conhecida.

use godmesh_core::{Core, EventDraft};
use godmesh_emergency::incident::{HumanProof, IncidentState};
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_policy::high_water::{AuthorityMark, DeviceEvidence};
use godmesh_policy::retention::{Jurisdiction, RetentionPolicy, RetentionTable};
use godmesh_protocol::ack::AckClass;

fn tabela() -> RetentionTable {
    let mut t = RetentionTable::new(Jurisdiction::Br);
    t.set(RetentionPolicy::new("incidente_sos", "dever_de_cuidado", 12, "incident_closed + 12m", Jurisdiction::Br).unwrap()).unwrap();
    t.set(RetentionPolicy::new("posicao", "mapa", 1, "expiry", Jurisdiction::Br).unwrap()).unwrap();
    t
}

fn grant_valido() -> GrantPredicate {
    GrantPredicate {
        operation: OfflineOp::OpenSosIncident,
        epoch_comparable: true,
        revocation: RevocationState::Absent,
        freshness: FreshnessSource::F3VerifiedOnline,
        retention_class_known: true,
        action_enumerated: true,
    }
}

#[test]
fn incident_is_refused_when_grant_predicate_denies() {
    let mut core = Core::new(tabela());
    let negado = GrantPredicate { freshness: FreshnessSource::None, ..grant_valido() };

    let r = core.open_incident(&negado, "inc-1", "tenant-1", "principal-42");
    assert!(r.is_err(), "sem raiz de confiança não abre incidente");
    assert!(core.incidents().is_empty(), "nada criado, nada enfileirado");
}

#[test]
fn incident_opens_only_after_grant_allows_and_stays_unpersisted() {
    let mut core = Core::new(tabela());
    let inc = core.open_incident(&grant_valido(), "inc-2", "tenant-1", "principal-42").expect("grant válido");
    assert_eq!(inc.state(), IncidentState::TriggeredLocally);
    assert_eq!(inc.ui_label(), "tentativa local, não persistida");
}

#[test]
fn persist_requires_retention_class_and_marks_not_persisted_when_missing() {
    // tabela sem a classe do incidente: persistência deve falhar fechado
    let mut sem_politica = RetentionTable::new(Jurisdiction::Br);
    sem_politica.set(RetentionPolicy::new("posicao", "mapa", 1, "expiry", Jurisdiction::Br).unwrap()).unwrap();
    let mut core_estreito = Core::new(sem_politica);
    let inc = core_estreito.open_incident(&grant_valido(), "inc-4", "tenant-1", "principal-42").unwrap();
    let id = inc.id().to_owned();

    let r = core_estreito.persist_incident(&id, "inbox:tenant-1:3-7-1");
    assert!(r.is_err(), "classe sem política não persiste");
    assert_eq!(core_estreito.incident(&id).unwrap().state(), IncidentState::NotPersisted);
}

#[test]
fn ack_class_round_trips_between_emergency_and_protocol() {
    let mut core = Core::new(tabela());
    let inc = core.open_incident(&grant_valido(), "inc-5", "tenant-1", "principal-42").unwrap();
    let id = inc.id().to_owned();
    core.persist_incident(&id, "inbox:tenant-1:3-7-2").unwrap();

    let classe = core.incident_ack_class(&id).unwrap();
    assert_eq!(classe, AckClass::PersistedLocal);
    assert_eq!(AckClass::parse(classe.as_str()), Ok(AckClass::PersistedLocal));
}

#[test]
fn human_ack_requires_technical_stage_across_crates() {
    let mut core = Core::new(tabela());
    let inc = core.open_incident(&grant_valido(), "inc-6", "tenant-1", "principal-42").unwrap();
    let id = inc.id().to_owned();
    core.persist_incident(&id, "inbox:tenant-1:3-7-3").unwrap();

    let tentativa = core.acknowledge_human(&id, HumanProof::person("resp-9", 1_700_000_000));
    assert!(tentativa.is_err(), "sem etapa técnica não há ACK humano");

    core.replicate_incident(&id, "node-a", 1_700_000_001).unwrap();
    core.ack_technical(&id, "node-a", 1_700_000_002).unwrap();
    core.acknowledge_human(&id, HumanProof::person("resp-9", 1_700_000_003)).expect("ACK humano após etapa técnica");
    assert_eq!(core.incident(&id).unwrap().state(), IncidentState::AcknowledgedHuman);
}

#[test]
fn event_is_refused_when_retention_class_is_unknown() {
    let core = Core::new(tabela());
    let r = core.record_event(EventDraft { origin: "node-a", tenant: "tenant-1", stream: "stream-9", partition: 3, epoch: 7, seq: 42 }, "audio_ptt");
    assert!(r.is_err(), "classe sem política não grava evento");

    let ok = core.record_event(EventDraft { origin: "node-a", tenant: "tenant-1", stream: "stream-9", partition: 3, epoch: 7, seq: 42 }, "posicao");
    assert!(ok.is_ok());
}

#[test]
fn offline_incident_requires_high_water_mark_to_advance() {
    let core = Core::new(tabela());
    let autoridade = AuthorityMark::new("principal-42", "device-7", 1_000);

    assert!(core.confirm_offline_freshness(&autoridade, DeviceEvidence::new("principal-42", "device-7", 900)).is_err());
    assert!(core.confirm_offline_freshness(&autoridade, DeviceEvidence::new("principal-42", "device-7", 1_000)).is_err());
    assert!(core.confirm_offline_freshness(&autoridade, DeviceEvidence::new("principal-42", "device-8", 1_100)).is_err());
    assert!(core.confirm_offline_freshness(&autoridade, DeviceEvidence::new("principal-42", "device-7", 1_100)).is_ok());
}
