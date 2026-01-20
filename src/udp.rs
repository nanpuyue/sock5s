use super::*;

pub struct Socks5UdpClient {
    pub udp_socket: UdpSocket,
    pub client_addr: SocketAddr,
}

pub struct Socks5UdpForwarder {
    ipv4_only: bool,
    pub udp_socket: Option<UdpSocket>,
    pub hosts: Option<HashMap<String, IpAddr>>,
}

impl Socks5UdpClient {
    pub fn new(udp_socket: UdpSocket, client_addr: SocketAddr) -> Self {
        Self {
            udp_socket,
            client_addr,
        }
    }
}

impl Socks5UdpForwarder {
    pub fn bind() -> Result<Self> {
        let mut ipv4_only = true;

        let udp_socket = if let Ok(socket) = (|| {
            let socket = Socket::new_raw(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
            socket.set_only_v6(false)?;
            socket.set_nonblocking(true)?;
            socket.bind(&SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0).into())?;
            ipv4_only = false;
            UdpSocket::from_std(socket.into())
        })() {
            Some(socket)
        } else {
            let socket = Socket::new_raw(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            socket.set_nonblocking(true)?;
            socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into())?;
            Some(UdpSocket::from_std(socket.into())?)
        };

        Ok(Self {
            ipv4_only,
            udp_socket,
            hosts: None,
        })
    }

    pub fn ipv4_only(&self) -> bool {
        self.ipv4_only
    }

    pub async fn lookup_host(&mut self, host: &str) -> Option<IpAddr> {
        let hosts = self.hosts.get_or_insert_default();

        if let Some(x) = hosts.get(host) {
            if !x.is_unspecified() {
                return Some(*x);
            } else {
                return None;
            }
        } else {
            if let Ok(mut x) = tokio::net::lookup_host((host, 0)).await {
                for x in x.by_ref() {
                    if self.ipv4_only && x.is_ipv6() {
                        continue;
                    }
                    let mut ip = x.ip();
                    if !self.ipv4_only
                        && let IpAddr::V4(x) = ip
                    {
                        ip = x.to_ipv6_mapped().into()
                    }
                    hosts.insert(host.into(), ip);
                    return Some(ip);
                }
            }
            hosts.insert(host.into(), (Ipv4Addr::UNSPECIFIED).into());
        }
        None
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
                let ip = match target.0 {
                    Socks5Host::IpAddr(IpAddr::V4(x)) if !self.ipv4_only() => {
                        Some(x.to_ipv6_mapped().into())
                    }
                    Socks5Host::IpAddr(IpAddr::V6(_)) if self.ipv4_only() => None,
                    Socks5Host::IpAddr(x) => Some(x),
                    Socks5Host::Domain(x) => self.lookup_host(&x).await,
                };
                if let Some(ip) = ip {
                    upstream_sender.send_to(data, (ip, target.1)).await?;
                }

                len = client_receiver.recv(&mut buf).await?;
            }
        };

        let t2 = async {
            let mut buf = vec![0; 1472];
            let mut header = (b"\x00\x00\x00").to_vec();

            loop {
                let (len, mut from) = upstream_receiver.recv_from(&mut buf).await?;
                if from.is_ipv6() {
                    from.set_ip(from.ip().to_canonical());
                }
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
}

impl Socks5Acceptor {
    pub async fn associate_udp(mut self) -> Result<()> {
        let mut local_addr = self.stream.local_addr()?;
        local_addr.set_port(0);
        let udp_socket = UdpSocket::bind(&local_addr).await?;
        local_addr = udp_socket.local_addr()?;

        let mut client_addr = self.stream.peer_addr()?;
        let target = Socks5Target::try_from(&self.buf[3..])?;
        client_addr.set_port(target.1);

        if client_addr.port() != 0 {
            eprintln!("{} == {} (udp)", client_addr, local_addr);
        }
        self.connected(local_addr).await?;

        let forwarder = match Socks5UdpForwarder::bind() {
            Ok(x) => x,
            Err(e) => {
                self.closed(1).await?;
                return Err(e);
            }
        };
        let udp_client = Socks5UdpClient::new(udp_socket, client_addr);
        let forward_udp = forwarder.forward_udp(udp_client);

        let done = async {
            let _ = self.stream.read(&mut [0]).await?;
            Ok(())
        };

        tokio::select! {
            r1 = forward_udp => {
                r1
            },
            r2 = done => {
                r2
            },
        }
    }
}
