use super::*;

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
        self.stream.take().into_result()
    }

    pub async fn udp_bind(&mut self) -> Result<()> {
        let udp_socket = match self.target {
            Socks5Target::V4(_) => {
                let bind = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
                UdpSocket::bind(bind).await?
            }
            Socks5Target::V6(_) | Socks5Target::Domain(_) => {
                let bind = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));
                let socket = UdpSocket::bind(bind).await?;
                SockRef::from(&socket).set_only_v6(false)?;
                socket
            }
        };

        self.udp_socket = Some(udp_socket);
        Ok(())
    }

    pub async fn forward_udp(mut self, client: Socks5UdpClient) -> Result<()> {
        let client_addr = client.client_addr;
        let udp_socket = client.udp_socket;

        let mut connected = false;
        if client.client_addr.port() != 0 {
            udp_socket.connect(client_addr).await?;
            connected = true;
        }

        let mut buf = vec![0; 1472];
        let (mut len, addr) = udp_socket.recv_from(&mut buf).await?;
        if !connected {
            if addr.ip() != client_addr.ip() {
                return Err(format!("Invalid client: {addr}!").into());
            }
            udp_socket.connect(addr).await?;
            eprintln!("{} == {} (udp)", addr, udp_socket.local_addr()?);
        }

        let (client_receiver, client_sender) = &mut udp_socket.split();
        let (upstream_receiver, upstream_sender) =
            &mut self.udp_socket.take().into_result()?.split();

        let t1 = async {
            loop {
                if &buf[..3] != b"\0\0\0" {
                    return Err("Invalid socks5 udp request!".into());
                }
                let offset = Socks5Target::target_len(&buf[3..])?;
                let target = Socks5Target::try_from(&buf[3..3 + offset])?;
                // eprintln!("{} -> {} (udp)", addr, target);

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
                            Some(addr) => {
                                let addr = if let SocketAddr::V4(addr) = addr {
                                    let ip = addr.ip().to_ipv6_mapped();
                                    SocketAddr::V6(SocketAddrV6::new(ip, addr.port(), 0, 0))
                                } else {
                                    addr
                                };
                                upstream_sender.send_to(data, &addr).await?
                            }
                            None => return Err("No addresses to send data to!".into()),
                        };
                    }
                };

                len = client_receiver.recv(&mut buf).await?;
            }
        };

        let t2 = async {
            let mut buf = vec![0; 1472];
            let mut header = (b"\x00\x00\x00").to_vec();

            loop {
                let (len, from) = upstream_receiver.recv_from(&mut buf).await?;
                header.truncate(3);
                header.extend_from_target(&from);

                let data = [IoSlice::new(&header), IoSlice::new(&buf[..len])];
                client_sender.send_vectored(&data).await?;
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
