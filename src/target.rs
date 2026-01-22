use super::*;

pub enum Socks5Host {
    IpAddr(IpAddr),
    Domain(String),
}

pub struct Socks5Target(pub Socks5Host, pub u16);

impl Display for Socks5Host {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Socks5Host::IpAddr(x) => x.fmt(f),
            Socks5Host::Domain(x) => x.fmt(f),
        }
    }
}

impl Display for Socks5Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Socks5Host::IpAddr(x) => write!(f, "{}:{}", x, self.1),
            Socks5Host::Domain(x) => write!(f, "{}:{}", x, self.1),
        }
    }
}

impl Socks5Target {
    fn parse_ipv4(data: &[u8]) -> Self {
        debug_assert_eq!(data.len(), 6);
        let ip = Ipv4Addr::from_octets(data[0..4].try_into().unwrap());
        let port = u16::from_be_bytes([data[4], data[5]]);
        Self(Socks5Host::IpAddr(IpAddr::V4(ip)), port)
    }

    fn parse_ipv6(data: &[u8]) -> Self {
        debug_assert_eq!(data.len(), 18);
        let ip = Ipv6Addr::from_octets(data[0..16].try_into().unwrap());
        let port = u16::from_be_bytes([data[16], data[17]]);
        Self(Socks5Host::IpAddr(IpAddr::V6(ip)), port)
    }

    fn parse_domain(data: &[u8]) -> Result<Self> {
        let len = data.len();
        debug_assert_eq!(len, 3 + data[0] as usize);
        let domain = match String::from_utf8(data[1..len - 2].into()) {
            Ok(s) => s,
            Err(e) => return Err(format!("Invalid domain: {e}!").into()),
        };
        let port = u16::from_be_bytes([data[len - 2], data[len - 1]]);
        Ok(Self(Socks5Host::Domain(domain), port))
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
}

impl TryFrom<&[u8]> for Socks5Target {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        Ok(match data[0] {
            1 => Self::parse_ipv4(&data[1..]),
            4 => Self::parse_ipv6(&data[1..]),
            3 => Self::parse_domain(&data[1..])?,
            _ => return Err("Invalid address type!".into()),
        })
    }
}
