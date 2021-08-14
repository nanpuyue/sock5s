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
