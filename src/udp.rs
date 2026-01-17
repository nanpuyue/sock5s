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
        let mut local_addr = self.stream.local_addr()?;
        local_addr.set_port(0);
        let udp_socket = UdpSocket::bind(&local_addr).await?;
        local_addr = udp_socket.local_addr()?;

        let mut client_addr = self.stream.peer_addr()?;
        let target = Socks5Target::try_from(&self.buf[3..])?;
        client_addr.set_port(target.port());

        if client_addr.port() != 0 {
            eprintln!("{} == {} (udp)", client_addr, local_addr);
        }
        self.connected(&local_addr).await?;

        let mut connector = Socks5Connector::new(target);
        if let Err(e) = connector.udp_bind().await {
            self.closed(1).await?;
            return Err(e);
        };

        let done = async {
            let _ = self.stream.read(&mut [0]).await?;
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
