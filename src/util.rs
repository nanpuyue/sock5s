#[cfg(target_family = "unix")]
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::net::UdpSocket;

use crate::error::{Error, Result};

pub trait ToHex {
    fn to_hex(&self) -> String;
}

impl ToHex for [u8] {
    fn to_hex(&self) -> String {
        const CHARS: &[u8] = b"0123456789abcdef";

        let mut v = Vec::with_capacity(self.len() * 2);
        for &b in self {
            v.push(CHARS[(b >> 4) as usize]);
            v.push(CHARS[(b & 0xf) as usize]);
        }

        unsafe { String::from_utf8_unchecked(v) }
    }
}

pub trait IntoResult<T> {
    fn into_result(self) -> Result<T>;
}

impl<T> IntoResult<T> for Option<T> {
    fn into_result(self) -> Result<T> {
        self.ok_or_else(|| Error::from("NoneError"))
    }
}

pub async fn link_stream<A: AsyncRead + AsyncWrite, B: AsyncRead + AsyncWrite>(
    a: A,
    b: B,
) -> Result<()> {
    let (ar, aw) = &mut io::split(a);
    let (br, bw) = &mut io::split(b);

    let r = tokio::select! {
        r1 = io::copy(ar, bw) => {
            r1
        },
        r2 = io::copy(br, aw) => {
            r2
        }
    };

    Ok(r.map(drop)?)
}

#[cfg(target_family = "unix")]
pub fn set_rlimit_nofile(limit: libc::rlim_t) -> Result<()> {
    unsafe {
        let mut rlimit = MaybeUninit::uninit();
        if libc::getrlimit(libc::RLIMIT_NOFILE, rlimit.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error().into());
        }
        let mut rlimit = rlimit.assume_init();

        if rlimit.rlim_cur < limit {
            rlimit.rlim_cur = limit;
            if libc::setrlimit(libc::RLIMIT_NOFILE, &rlimit) != 0 {
                return Err(io::Error::last_os_error().into());
            }
        }
    }

    Ok(())
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

impl Split for UdpSocket {
    fn split(self) -> (RecvHalf<UdpSocket>, SendHalf<UdpSocket>) {
        let shared = Arc::new(self);
        let send = shared.clone();
        let recv = shared;
        (RecvHalf(recv), SendHalf(send))
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

impl SendHalf<UdpSocket> {
    pub async fn send_to(&mut self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        self.0.send_to(buf, target).await
    }

    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf).await
    }
}
