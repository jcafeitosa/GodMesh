use super::*;

#[test]
fn cancelamento_descarta_fila_sem_estornar_credito_ou_liberar_vaga() {
    let now = Instant::now();
    let quotas = Quotas::new(now);
    let first = quotas.reserve("d1", now).unwrap();
    first.forward(now).unwrap();
    for _ in 0..4 {
        first
            .enqueue(Direction::ToDestination, vec![1; 1196])
            .unwrap();
    }
    assert_eq!(
        first.enqueue(Direction::ToDestination, vec![1]),
        Err(Error::QueueFull)
    );
    let sent = first.take(Direction::ToDestination, now).unwrap().unwrap();
    assert_eq!(sent.datagram().len(), 1196);
    assert_eq!(sent.deadline(), now + IO_TIMEOUT);
    assert!(first.take(Direction::ToDestination, now).unwrap().is_none());
    first.cancel();
    assert_eq!(
        first.take(Direction::ToDestination, now).err(),
        Some(Error::Cancelled)
    );
    assert!(matches!(
        quotas.reserve("d1", now),
        Err(Error::QuotaExceeded)
    ));
    let second = quotas.reserve("d2", now).unwrap();
    second.forward(now).unwrap();
    assert!(matches!(
        quotas.reserve("d3", now),
        Err(Error::QuotaExceeded)
    ));
    first.recollect();
    assert!(matches!(
        quotas.reserve("d1", now),
        Err(Error::QuotaExceeded)
    ));
    drop(sent);
    let replacement = quotas.reserve("d1", now).unwrap();
    replacement.forward(now).unwrap();
    assert!(
        replacement
            .take(Direction::ToDestination, now)
            .unwrap()
            .is_none()
    );
    assert_eq!(quotas.snapshot("d1").debited_bytes, 1200);
}

#[test]
fn debito_inteiro_por_destino_e_janela_com_sonda_independente() {
    let now = Instant::now();
    let quotas = Quotas::new(now);
    let d1 = quotas.reserve("d1", now).unwrap();
    let d2 = quotas.reserve("d2", now).unwrap();
    d1.forward(now).unwrap();
    d2.forward(now).unwrap();
    for _ in 0..25 {
        d1.enqueue(Direction::ToDestination, vec![1; 1400]).unwrap();
        drop(d1.take(Direction::ToDestination, now).unwrap().unwrap());
    }
    d1.enqueue(Direction::FromDestination, vec![1; 897])
        .unwrap();
    assert_eq!(
        d1.take(Direction::FromDestination, now).err(),
        Some(Error::RateExceeded)
    );
    assert_eq!(quotas.snapshot("d1").debited_bytes, 35100);
    d1.enqueue(Direction::FromDestination, vec![1; 896])
        .unwrap();
    drop(d1.take(Direction::FromDestination, now).unwrap().unwrap());
    assert_eq!(quotas.snapshot("d1").debited_bytes, 36000);

    for _ in 0..32 {
        d2.enqueue(Direction::FromDestination, vec![1]).unwrap();
        drop(d2.take(Direction::FromDestination, now).unwrap().unwrap());
    }
    d2.enqueue(Direction::ToDestination, vec![1]).unwrap();
    assert_eq!(
        d2.take(Direction::ToDestination, now).err(),
        Some(Error::RateExceeded)
    );
    assert_eq!(quotas.snapshot("d2").debited_frames, 32);
    let probe = quotas.reserve_probe().unwrap();
    probe.enqueue(vec![1; 64]).unwrap();
    assert_eq!(probe.take(now).unwrap().unwrap().len(), 64);
    probe.enqueue(vec![1]).unwrap();
    assert_eq!(probe.take(now).err(), Some(Error::RateExceeded));
    d1.enqueue(Direction::ToDestination, vec![1]).unwrap();
    drop(
        d1.take(Direction::ToDestination, now + WINDOW)
            .unwrap()
            .unwrap(),
    );
}

use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use godmesh_federation::{NodeIdentity, node::LinkIdentity};

use crate::{
    federation::JoinWindow,
    portable_join::PortableJoin,
    replica_operational::fixture::{CLUSTER, ReceiverFixture, configured_portable},
};

fn pin(id: &NodeIdentity) -> LinkIdentity {
    LinkIdentity {
        node_id: id.node_id().to_owned(),
        public_key: id.public_key_bytes(),
        cluster: CLUSTER.into(),
    }
}

fn route(requester: &NodeIdentity, destination: &NodeIdentity, port: u16) -> Route {
    Route {
        id: format!("route-{port}"),
        requester: pin(requester),
        destination: pin(destination),
        service: "fixture".into(),
        endpoint: ([127, 0, 0, 1], port).into(),
    }
}

#[test]
fn cadastro_compara_pins_e_enderecos_normalizados() {
    let r = NodeIdentity::generate_in_memory().unwrap();
    let e = NodeIdentity::generate_in_memory().unwrap();
    let d1 = NodeIdentity::generate_in_memory().unwrap();
    let d2 = NodeIdentity::generate_in_memory().unwrap();
    let valid = route(&e, &d1, 9000);
    let mapped = "[::ffff:127.0.0.1]:9000".parse().unwrap();
    assert_eq!(normalize_endpoint(mapped).unwrap(), valid.endpoint);
    assert!(Registry::new(pin(&r), vec![valid.clone()], [mapped]).is_err());
    let mut duplicate = route(&e, &d2, 9001);
    duplicate.endpoint = mapped;
    assert!(Registry::new(pin(&r), vec![valid.clone(), duplicate], []).is_err());
    let mut wrong_pin = valid.clone();
    wrong_pin.destination.public_key = d2.public_key_bytes();
    assert!(Registry::new(pin(&r), vec![wrong_pin], []).is_err());
    assert!(Registry::new(pin(&r), vec![valid], []).is_ok());
}

#[tokio::test]
async fn autoridade_revalida_token_debito_e_rechecagem_sem_sinal() {
    let Some(f) = ReceiverFixture::new("relay_admission").await else {
        return;
    };
    let e = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let d2 = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let pins = [&*e, &*f.origin, &*d2, &*f.receiver]
        .map(|id| format!("{}:{}", id.node_id(), id.public_key_hex()))
        .join(";");
    let config = configured_portable(&pins);
    let portable = Arc::new(PortableJoin::new(
        config.clone(),
        f.pool.clone(),
        f.receiver.clone(),
    ));
    for peer in [&e, &d2] {
        let peer = PortableJoin::new(config.clone(), f.pool.clone(), peer.clone());
        portable
            .import_genesis(&peer.local_manifest().unwrap(), JoinWindow::default())
            .await
            .unwrap();
    }
    let r1 = route(&e, &f.origin, 9001);
    let r2 = route(&e, &d2, 9002);
    let clock = Arc::new(AtomicI64::new(110));
    let admission = Arc::new(
        RelayAdmission::new(
            Registry::new(pin(&f.receiver), vec![r1.clone(), r2.clone()], []).unwrap(),
            portable.clone(),
            f.bundle.clone(),
            Instant::now(),
        )
        .unwrap()
        .with_clock({
            let clock = clock.clone();
            Arc::new(move || Duration::from_secs(clock.load(Ordering::SeqCst) as u64))
        }),
    );
    assert!(matches!(
        admission.open(&r1.id, &pin(&e), Instant::now()).await,
        Err(Error::RouteDenied)
    ));
    admission.set_allowed(&r1.id, true).unwrap();
    admission.set_allowed(&r2.id, true).unwrap();
    assert!(matches!(
        admission.open(&r1.id, &pin(&d2), Instant::now()).await,
        Err(Error::RouteDenied)
    ));
    let d1_route = admission
        .open(&r1.id, &pin(&e), Instant::now())
        .await
        .unwrap();
    let d2_route = admission
        .open(&r2.id, &pin(&e), Instant::now())
        .await
        .unwrap();
    assert_eq!(
        d1_route.enqueue(Direction::ToDestination, vec![1]),
        Err(Error::RouteUnavailable)
    );
    admission.set_allowed(&r1.id, false).unwrap();
    assert!(d1_route.confirm_token().await.is_err());
    assert!(*d1_route.cancellation().borrow());
    d1_route.recollect();
    admission.set_allowed(&r1.id, true).unwrap();
    let d1_route = admission
        .open(&r1.id, &pin(&e), Instant::now())
        .await
        .unwrap();
    d1_route.confirm_token().await.unwrap();
    d2_route.confirm_token().await.unwrap();
    d1_route.enqueue(Direction::ToDestination, vec![1]).unwrap();
    let used = d1_route
        .next_credit(Direction::ToDestination)
        .await
        .unwrap()
        .unwrap();
    d1_route.enqueue(Direction::ToDestination, vec![2]).unwrap();

    // Controle durável sem passar pelo sinal local de trust.
    sqlx::query("UPDATE federation_peer SET trust_state='revoked' WHERE node_id=$1")
        .bind(f.origin.node_id())
        .execute(&f.pool)
        .await
        .unwrap();
    let invalidated = Instant::now();
    assert_eq!(
        d1_route
            .run_rechecks(invalidated + Duration::from_millis(200), None)
            .await,
        Err(Error::AuthorityBlocked)
    );
    assert!(invalidated.elapsed() <= Duration::from_millis(350));
    assert!(*d1_route.cancellation().borrow());
    let usage = admission.usage(Some(f.origin.node_id()), Direction::ToDestination);
    assert_eq!((usage.queued_frames, usage.debited_frames), (0, 1));
    drop(used);
    d1_route.recollect();
    d2_route
        .enqueue(Direction::FromDestination, vec![7])
        .unwrap();
    drop(
        d2_route
            .next_credit(Direction::FromDestination)
            .await
            .unwrap()
            .unwrap(),
    );

    let mut lock = f.pool.begin().await.unwrap();
    portable.lock_cluster(&mut lock).await.unwrap();
    let blocked = Instant::now();
    assert_eq!(
        d2_route.run_rechecks(blocked, None).await,
        Err(Error::RouteUnavailable)
    );
    assert!(blocked.elapsed() <= Duration::from_millis(350));
    assert!(*d2_route.cancellation().borrow());
    d2_route.recollect();
    lock.rollback().await.unwrap();

    let d2_route = admission
        .open(&r2.id, &pin(&e), Instant::now())
        .await
        .unwrap();
    clock.store(500, Ordering::SeqCst);
    assert_eq!(d2_route.confirm_token().await, Err(Error::AuthorityBlocked));
    assert!(*d2_route.cancellation().borrow());
    d2_route.recollect();

    clock.store(110, Ordering::SeqCst);
    sqlx::query("UPDATE federation_peer SET trust_state='authorized' WHERE node_id=$1")
        .bind(f.origin.node_id())
        .execute(&f.pool)
        .await
        .unwrap();
    let first = admission
        .open(&r1.id, &pin(&e), Instant::now())
        .await
        .unwrap();
    let second = admission
        .open(&r2.id, &pin(&e), Instant::now())
        .await
        .unwrap();
    first.confirm_token().await.unwrap();
    second.confirm_token().await.unwrap();
    // Isola a reação ao piso já efetivo; a transição assinada pertence aos testes M3.
    sqlx::query(
        "UPDATE federation_config_frame SET floor=2 WHERE hash =
        (SELECT effective FROM federation_config_state)",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    let (a, b) = tokio::join!(first.recheck(), second.recheck());
    assert_eq!(a, Err(Error::AuthorityBlocked));
    assert_eq!(b, Err(Error::AuthorityBlocked));
    assert!(*first.cancellation().borrow());
    assert!(*second.cancellation().borrow());
    first.recollect();
    second.recollect();
}

#[test]
fn filas_e_creditos_preservam_ambas_as_parcelas_no_teto() {
    let now = Instant::now();
    let quotas = Quotas::new(now);
    let dir = Direction::FromDestination;
    let d1 = quotas.reserve("d1", now).unwrap();
    let d2 = quotas.reserve("d2", now).unwrap();
    for destination in [&d1, &d2] {
        destination.forward(now).unwrap();
        for _ in 0..3 {
            destination.enqueue(dir, vec![1; 1400]).unwrap();
        }
        assert_eq!(
            destination.enqueue(dir, vec![2; 585]),
            Err(Error::QueueFull)
        );
        destination.enqueue(dir, vec![3; 584]).unwrap();
        assert_eq!(destination.enqueue(dir, vec![4]), Err(Error::QueueFull));
    }
    let aggregate = quotas.usage(None, dir);
    assert_eq!((aggregate.queued_frames, aggregate.queued_bytes), (8, 9600));
    for destination in [&d1, &d2] {
        for expected in [1400, 1400, 1400, 584] {
            assert_eq!(
                destination
                    .take(dir, now)
                    .unwrap()
                    .unwrap()
                    .datagram()
                    .len(),
                expected
            );
        }
        let next = now + WINDOW;
        // 31 * 1125 + 1125 = 36000, contando o prefixo.
        for _ in 0..32 {
            destination.enqueue(dir, vec![1; 1121]).unwrap();
            drop(destination.take(dir, next).unwrap().unwrap());
        }
        destination.enqueue(dir, vec![1]).unwrap();
        assert_eq!(destination.take(dir, next).err(), Some(Error::RateExceeded));
    }
    let aggregate = quotas.usage(None, dir);
    assert_eq!(
        (aggregate.debited_frames, aggregate.debited_bytes),
        (72, 81_600)
    );
}

#[tokio::test]
async fn espera_por_direcao_acorda_com_fila_credito_e_cancelamento() {
    use std::{future::poll_fn, task::Poll};
    let now = Instant::now();
    let quotas = Quotas::new(now);
    let route = quotas.reserve("d1", now).unwrap();
    route.forward(now).unwrap();
    let dir = Direction::ToDestination;
    let mut waiting = std::pin::pin!(route.wait_for_packet(dir));
    poll_fn(|cx| {
        assert!(waiting.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    route.enqueue(dir, vec![1]).unwrap();
    waiting.await.unwrap();
    let credit = route.take(dir, now).unwrap().unwrap();

    let mut waiting = std::pin::pin!(route.wait_for_packet(dir));
    route.enqueue(dir, vec![2]).unwrap();
    poll_fn(|cx| {
        assert!(waiting.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(credit);
    waiting.await.unwrap();

    let mut waiting = std::pin::pin!(route.wait_for_packet(Direction::FromDestination));
    poll_fn(|cx| {
        assert!(waiting.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    route.cancel();
    assert_eq!(waiting.await, Err(Error::Cancelled));
}
