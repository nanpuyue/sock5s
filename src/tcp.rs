use super::*;

pub struct Socks5TcpConnector(TcpStream);

impl Socks5TcpConnector {
    pub async fn connect(target: Socks5Target) -> Result<Self> {
        let stream = match target.0 {
            Socks5Host::IpAddr(x) => TcpStream::connect((x, target.1)).await?,
            Socks5Host::Domain(x) => TcpStream::connect((x, target.1)).await?,
        };
        Ok(Self(stream))
    }

    pub async fn connect_tcp(mut self, mut stream: TcpStream) -> Result<()> {
        tokio::io::copy_bidirectional(&mut self.0, &mut stream).await?;
        Ok(())
    }
}

impl Socks5Acceptor {
    pub async fn connect(mut self, target: Socks5Target) -> Result<()> {
        eprintln!("{} -> {}", self.peer_addr(), target);
        let connector = Socks5TcpConnector::connect(target).await?;
        self.connected(self.stream.local_addr()?).await?;

        connector.connect_tcp(self.stream).await
    }
}
