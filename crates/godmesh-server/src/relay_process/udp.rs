use std::{io, num::NonZeroU32};

use tokio::net::UdpSocket;

pub(super) const DATAGRAM_LIMIT: NonZeroU32 = NonZeroU32::new(1400).unwrap();

pub(super) struct UdpReader {
    buffer: [u8; DATAGRAM_LIMIT.get() as usize + 1],
}

impl Default for UdpReader {
    fn default() -> Self {
        Self {
            buffer: [0; DATAGRAM_LIMIT.get() as usize + 1],
        }
    }
}

impl UdpReader {
    pub(super) async fn receive(&mut self, socket: &UdpSocket) -> io::Result<Option<&[u8]>> {
        let bytes = socket.recv(&mut self.buffer).await?;
        // Uma recepção truncada preenche o byte extra e nunca vira um frame parcial.
        Ok((1..=DATAGRAM_LIMIT.get() as usize)
            .contains(&bytes)
            .then(|| &self.buffer[..bytes]))
    }
}
