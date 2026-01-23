use std::io::IoSlice;
use std::net::SocketAddr;
use std::sync::Arc;

use socket2::SockRef;
use tokio::io::{self, Interest};
use tokio::net::{ToSocketAddrs, UdpSocket};

use crate::error::{Error, Result};

pub trait IntoResult<T> {
    fn into_result(self) -> Result<T>;
}

impl<T> IntoResult<T> for Option<T> {
    fn into_result(self) -> Result<T> {
        self.ok_or_else(|| Error::from("NoneError"))
    }
}

#[derive(Debug)]
pub struct SendHalf<T>(Arc<T>);

#[derive(Debug)]
pub struct RecvHalf<T>(Arc<T>);

pub trait Split {
    fn split(self) -> (RecvHalf<Self>, SendHalf<Self>)
    where
        Self: Sized;
}

pub trait Vectored {
    async fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize>;
}

pub trait PutSocks5Addr {
    fn put_socks5_addr(&mut self, addr: SocketAddr);
}

impl Split for UdpSocket {
    fn split(self) -> (RecvHalf<UdpSocket>, SendHalf<UdpSocket>) {
        let shared = Arc::new(self);
        let send = shared.clone();
        let recv = shared;
        (RecvHalf(recv), SendHalf(send))
    }
}

impl Vectored for UdpSocket {
    async fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.async_io(Interest::WRITABLE, || {
            SockRef::from(self).send_vectored(bufs)
        })
        .await
    }
}

impl RecvHalf<UdpSocket> {
    pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf).await
    }
}

#[allow(unused)]
impl SendHalf<UdpSocket> {
    pub async fn send_to<A: ToSocketAddrs>(&mut self, buf: &[u8], addr: A) -> io::Result<usize> {
        self.0.send_to(buf, addr).await
    }

    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf).await
    }

    pub async fn send_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.send_vectored(bufs).await
    }
}

impl PutSocks5Addr for Vec<u8> {
    fn put_socks5_addr(&mut self, mut addr: SocketAddr) {
        if addr.is_ipv6() {
            addr.set_ip(addr.ip().to_canonical());
        }
        match addr {
            SocketAddr::V4(x) => {
                self.push(b'\x01');
                self.extend_from_slice(&x.ip().octets());
                self.extend_from_slice(&x.port().to_be_bytes());
            }
            SocketAddr::V6(x) => {
                self.push(b'\x04');
                self.extend_from_slice(&x.ip().octets());
                self.extend_from_slice(&x.port().to_be_bytes());
            }
        }
    }
}

#[cfg(target_family = "unix")]
pub fn set_rlimit_nofile(limit: libc::rlim_t) -> Result<()> {
    unsafe {
        let mut rlimit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlimit) != 0 {
            return Err(io::Error::last_os_error().into());
        }

        let limit = std::cmp::min(limit, rlimit.rlim_max);
        if rlimit.rlim_cur < limit {
            rlimit.rlim_cur = limit;
            if libc::setrlimit(libc::RLIMIT_NOFILE, &rlimit) != 0 {
                return Err(io::Error::last_os_error().into());
            }
        }
    }

    Ok(())
}
