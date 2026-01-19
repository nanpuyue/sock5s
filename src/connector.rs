use super::*;

pub struct Socks5TcpConnector;

impl Socks5TcpConnector {
    pub async fn connect(self, target: Socks5Target) -> Result<TcpStream> {
        let stream = match target {
            Socks5Target::V4(x) => TcpStream::connect(x).await?,
            Socks5Target::V6(x) => TcpStream::connect(x).await?,
            Socks5Target::Domain(x) => TcpStream::connect((x.0.as_str(), x.1)).await?,
        };
        Ok(stream)
    }
}
