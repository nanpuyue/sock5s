use super::*;

pub struct Socks5TcpConnector;

impl Socks5TcpConnector {
    pub async fn connect(self, target: Socks5Target) -> Result<TcpStream> {
        let stream = match target.0 {
            Socks5Host::IpAddr(x) => TcpStream::connect((x, target.1)).await?,
            Socks5Host::Domain(x) => TcpStream::connect((x, target.1)).await?,
        };
        Ok(stream)
    }
}
