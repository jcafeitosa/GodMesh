use std::{io, net::SocketAddr, time::Instant};

use tokio::{net::UdpSocket, time::timeout_at};

/// Sumidouro da fixture. Mantém o endpoint direto ocupado e descarta o que recebe.
pub(super) struct BlockedPath(UdpSocket);

#[derive(Debug)]
pub(super) struct ObservedDrop {
    endpoint: SocketAddr,
    confirmed_at: Instant,
}

impl ObservedDrop {
    pub(super) fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub(super) fn confirmed_at(&self) -> Instant {
        self.confirmed_at
    }
}

impl BlockedPath {
    pub(super) async fn bind(endpoint: SocketAddr, edge: SocketAddr) -> io::Result<Self> {
        if !endpoint.ip().is_loopback() || !edge.ip().is_loopback() || edge.port() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "o controlador M5 exige loopback e endpoint de origem fixo",
            ));
        }
        let socket = UdpSocket::bind(endpoint).await?;
        socket.connect(edge).await?;
        Ok(Self(socket))
    }

    pub(super) fn endpoint(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }

    pub(super) async fn observe_drop(
        &self,
        deadline: tokio::time::Instant,
    ) -> io::Result<ObservedDrop> {
        let endpoint = self.endpoint()?;
        // O socket conectado só recebe de E. Um byte basta para observar e
        // descartar o datagrama inteiro, sem interpretar o conteúdo.
        while tokio::time::Instant::now() < deadline {
            let bytes = timeout_at(deadline, self.0.recv(&mut [0_u8]))
                .await
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::TimedOut, "nenhum datagrama direto")
                })??;
            if bytes > 0 {
                return Ok(ObservedDrop {
                    endpoint,
                    confirmed_at: Instant::now(),
                });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "nenhum datagrama direto",
        ))
    }
}
