use super::*;

pub enum Socks5Target {
    V4(SocketAddrV4),
    V6(SocketAddrV6),
    Domain((String, u16)),
}

impl Display for Socks5Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::V4(x) => x.fmt(f),
            Self::V6(x) => x.fmt(f),
            Self::Domain(x) => write!(f, "{}:{}", x.0, x.1),
        }
    }
}

impl Socks5Target {
    fn parse_ipv4(data: &[u8]) -> Self {
        debug_assert_eq!(data.len(), 6);
        Self::V4(SocketAddrV4::new(
            (unsafe { *(data.as_ptr() as *const [u8; 4]) }).into(),
            u16::from_be_bytes([data[4], data[5]]),
        ))
    }

    fn parse_ipv6(data: &[u8]) -> Self {
        debug_assert_eq!(data.len(), 18);
        Self::V6(SocketAddrV6::new(
            (unsafe { *(data.as_ptr() as *const [u8; 16]) }).into(),
            u16::from_be_bytes([data[16], data[17]]),
            0,
            0,
        ))
    }

    fn parse_domain(data: &[u8]) -> Result<Self> {
        let len = data.len();
        debug_assert_eq!(len, 3 + data[0] as usize);
        let domain = match String::from_utf8(data[1..len - 2].into()) {
            Ok(s) => s,
            Err(e) => return Err(format!("Invalid domain: {}!", e).into()),
        };
        let port = u16::from_be_bytes([data[len - 2], data[len - 1]]);
        Ok(Self::Domain((domain, port)))
    }

    pub fn target_len(data: &[u8]) -> Result<usize> {
        debug_assert!(data.len() >= 2);
        Ok(match data[0] {
            1 => 7,
            4 => 19,
            3 => 4 + data[1] as usize,
            _ => return Err("Invalid address type!".into()),
        })
    }

    pub fn try_parse(data: &[u8]) -> Result<Socks5Target> {
        Ok(match data[0] {
            1 => Self::parse_ipv4(&data[1..]),
            4 => Self::parse_ipv6(&data[1..]),
            3 => Self::parse_domain(&data[1..])?,
            _ => return Err("Invalid address type!".into()),
        })
    }

    pub fn port(&self) -> u16 {
        match self {
            Self::V4(x) => x.port(),
            Self::V6(x) => x.port(),
            Self::Domain(x) => x.1,
        }
    }
}

pub struct Socks5Connector {
    target: Socks5Target,
    stream: Option<TcpStream>,
    udp_socket: Option<UdpSocket>,
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
        Ok(self.stream.take().into_result()?)
    }

    pub async fn udp_bind(&mut self) -> Result<()> {
        let bind = match self.target {
            Socks5Target::V4(_) => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
            Socks5Target::V6(_) => {
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0))
            }
            _ => return Err("Not support domain client for udp associate!".into()),
        };
        self.udp_socket = Some(UdpSocket::bind(bind).await?);
        Ok(())
    }

    pub async fn forward_udp(mut self, client: Socks5UdpClient) -> Result<()> {
        let client_addr = client.client_addr();

        let (client_receiver, client_sender) = &mut client.connect().await?.split();
        let (upstream_receiver, upstream_sender) =
            &mut self.udp_socket.take().into_result()?.split();

        let t1 = async {
            let mut buf = vec![0; 1472];
            loop {
                let len = client_receiver.recv(&mut buf).await?;
                if &buf[..3] != b"\0\0\0" {
                    return Err("Invalid socks5 udp request!".into());
                }
                let offset = Socks5Target::target_len(&buf[3..])?;
                let target = Socks5Target::try_parse(&buf[3..3 + offset])?;
                eprintln!("{} -> {} (udp)", client_addr, target);

                let data = &buf[3 + offset..len];
                match target {
                    Socks5Target::V4(x) => {
                        upstream_sender.send_to(data, &x.into()).await?;
                    }
                    Socks5Target::V6(x) => {
                        upstream_sender.send_to(data, &x.into()).await?;
                    }
                    Socks5Target::Domain(x) => {
                        match lookup_host((x.0.as_str(), x.1)).await?.next() {
                            Some(addr) => upstream_sender.send_to(data, &addr).await?,
                            None => return Err("No addresses to send data to!".into()),
                        };
                    }
                };
            }
        };

        let t2 = async {
            let mut buf = vec![0; 1472];
            loop {
                let (len, from) = upstream_receiver.recv_from(&mut buf).await?;

                let data = match from {
                    SocketAddr::V4(x) => [
                        b"\x00\x00\x00\x01",
                        x.ip().octets().as_ref(),
                        &x.port().to_be_bytes(),
                        &buf[..len],
                    ]
                    .concat(),
                    SocketAddr::V6(x) => [
                        b"\x00\x00\x00\x04",
                        x.ip().octets().as_ref(),
                        &x.port().to_be_bytes(),
                        &buf[..len],
                    ]
                    .concat(),
                };

                client_sender.send(&data).await?;
            }
        };

        tokio::select! {
            r1 = t1 => {
                r1
            },
            r2 = t2 => {
                r2
            },
        }
    }

    pub fn new(target: Socks5Target) -> Self {
        Self {
            target,
            stream: None,
            udp_socket: None,
        }
    }
}
