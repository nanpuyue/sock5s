use super::*;

pub struct Socks5UdpClient {
    pub udp_socket: UdpSocket,
    pub client_addr: SocketAddr,
}

impl Socks5UdpClient {
    pub fn new(udp_socket: UdpSocket, client_addr: SocketAddr) -> Self {
        Self {
            udp_socket,
            client_addr,
        }
    }
}

impl Socks5Acceptor {
    pub async fn associate_udp(mut self) -> Result<()> {
        let mut local = self.stream.local_addr()?;
        local.set_port(0);
        let udp_socket = UdpSocket::bind(&local).await?;

        let mut client_addr = self.stream.peer_addr()?;
        let target = Socks5Target::try_from(&self.buf[3..])?;
        client_addr.set_port(target.port());

        if client_addr.port() != 0 {
            eprintln!("{} == {} (udp)", client_addr, udp_socket.local_addr()?);
        }
        let reply = match udp_socket.local_addr()? {
            SocketAddr::V4(x) => [
                b"\x05\x00\x00\x01".as_ref(),
                x.ip().octets().as_ref(),
                x.port().to_be_bytes().as_ref(),
            ]
            .concat(),
            SocketAddr::V6(x) => [
                b"\x05\x00\x00\x04".as_ref(),
                x.ip().octets().as_ref(),
                x.port().to_be_bytes().as_ref(),
            ]
            .concat(),
        };
        self.stream.write_all(&reply).await?;

        let mut connector = Socks5Connector::new(target);
        if let Err(e) = connector.udp_bind().await {
            self.closed(1).await?;
            return Err(e);
        };

        let done = async {
            self.stream.read(&mut [0]).await?;
            Ok(())
        };

        let udp_client = Socks5UdpClient::new(udp_socket, client_addr);
        let forward_udp = connector.forward_udp(udp_client);

        tokio::select! {
            r1 = done => {
                r1
            },
            r2 = forward_udp => {
                r2
            },
        }
    }
}
