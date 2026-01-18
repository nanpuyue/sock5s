use super::*;

pub struct Socks5Connector {
    target: Socks5Target,
    stream: Option<TcpStream>,
}

impl Socks5Connector {
    pub async fn connect(&mut self) -> Result<()> {
        self.stream = Some(match &self.target {
            Socks5Target::V4(x) => TcpStream::connect(x).await?,
            Socks5Target::V6(x) => TcpStream::connect(x).await?,
            Socks5Target::Domain(x) => TcpStream::connect((x.0.as_str(), x.1)).await?,
        });
        Ok(())
    }

    pub async fn connected(mut self, payload: &[u8]) -> Result<TcpStream> {
        self.stream
            .as_mut()
            .into_result()?
            .write_all(payload)
            .await?;
        self.stream.take().into_result()
    }

    pub fn new(target: Socks5Target) -> Self {
        Self {
            target,
            stream: None,
        }
    }
}
