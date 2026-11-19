#[path = "../src/relay_process/udp.rs"]
mod udp;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tokio::{net::UdpSocket, time::timeout};

use udp::{DATAGRAM_LIMIT, UdpReader};

async fn connected_pair(addr: SocketAddr) -> (UdpSocket, UdpSocket) {
    let first = UdpSocket::bind(addr).await.unwrap();
    let second = UdpSocket::bind(addr).await.unwrap();
    first.connect(second.local_addr().unwrap()).await.unwrap();
    second.connect(first.local_addr().unwrap()).await.unwrap();
    (first, second)
}

async fn assert_boundaries(addr: SocketAddr) {
    let (first, second) = connected_pair(addr).await;
    for (sender, receiver) in [(&first, &second), (&second, &first)] {
        let mut reader = UdpReader::default();
        let max = DATAGRAM_LIMIT.get() as usize;
        let exact: Vec<u8> = (0..max).map(|index| (index % 251) as u8).collect();
        for (packet, admitted) in [
            (Vec::new(), false),
            (vec![8; max + 1], false),
            (vec![9; 8192], false),
            (exact.clone(), true),
        ] {
            assert_eq!(sender.send(&packet).await.unwrap(), packet.len());
            let observed = timeout(Duration::from_secs(1), reader.receive(receiver))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(observed, admitted.then_some(packet.as_slice()));
        }
    }
}

#[tokio::test]
async fn relay_udp_descarta_excedente_sem_enquadrar_datagrama_parcial_ipv4() {
    assert_boundaries((Ipv4Addr::LOCALHOST, 0).into()).await;
}

#[tokio::test]
async fn relay_udp_descarta_excedente_sem_enquadrar_datagrama_parcial_ipv6() {
    assert_boundaries((Ipv6Addr::LOCALHOST, 0).into()).await;
}
