#[path = "../src/relay_process/blocked_path.rs"]
mod blocked_path;

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::{net::UdpSocket, time::Instant};

use blocked_path::BlockedPath;

#[tokio::test]
async fn confirma_apenas_datagrama_recebido_e_descartado_do_endpoint_direto_de_e() {
    let loopback = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let edge = UdpSocket::bind(loopback).await.unwrap();
    let unrelated = UdpSocket::bind(loopback).await.unwrap();
    let sink = BlockedPath::bind(loopback, edge.local_addr().unwrap())
        .await
        .unwrap();
    let direct = sink.endpoint().unwrap();

    for sender in [&unrelated, &edge] {
        if sender.local_addr().unwrap() != edge.local_addr().unwrap() {
            sender.send_to(b"outra origem", direct).await.unwrap();
        }
        let deadline = Instant::now() + Duration::from_millis(30);
        let error = sink.observe_drop(deadline).await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }

    let before = std::time::Instant::now();
    edge.send_to(&[5; 1200], direct).await.unwrap();
    let proof = sink
        .observe_drop(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(proof.endpoint(), direct);
    assert!(proof.confirmed_at() >= before);
    assert!(
        sink.observe_drop(Instant::now() + Duration::from_millis(30))
            .await
            .is_err()
    );
}
