use std::io::IoSlice;
#[cfg(target_family = "unix")]
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::sync::Arc;

use socket2::{Domain, Protocol, SockAddr, SockRef, Socket, Type};
use tokio::io::Interest;
use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::net::UdpSocket;

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
    pub async fn send_to(&mut self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        self.0.send_to(buf, target).await
    }

    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf).await
    }

    pub async fn send_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.send_vectored(bufs).await
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

pub fn udp_bind_v6<A: Into<SockAddr>>(addr: A) -> Result<UdpSocket> {
    let socket = Socket::new_raw(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_only_v6(false)?;
    socket.bind(&addr.into())?;
    socket.set_nonblocking(true)?;
    Ok(UdpSocket::from_std(socket.into())?)
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
