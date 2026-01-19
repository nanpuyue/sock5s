use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use std::io::IoSlice;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::pin::Pin;
use std::task::{Context, Poll};

use clap::{Arg, Command};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use tokio_stream::{Stream, StreamExt};

#[cfg(target_family = "unix")]
use self::util::set_rlimit_nofile;
use self::{
    acceptor::Socks5Acceptor,
    connector::Socks5TcpConnector,
    error::{Error, Result},
    listener::Socks5Listener,
    target::Socks5Target,
    util::{ExtendFromTarget, IntoResult, Split},
};

pub type Socks5Stream = TcpStream;

mod acceptor;
mod connector;
mod error;
mod listener;
mod target;
mod udp;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("sock5s")
        .version("0.2.2")
        .author("南浦月 <nanpuyue@gmail.com>")
        .about("A Simple Socks5 Proxy Server")
        .arg(
            Arg::new("listen")
                .short('l')
                .long("listen")
                .value_name("LISTEN ADDR")
                .help("Specify the listen addr")
                .num_args(1)
                .required(true),
        )
        .get_matches();

    let listen = matches.get_one::<String>("listen").unwrap();

    let mut listener = Socks5Listener::listen(listen).await?;

    #[cfg(target_family = "unix")]
    let _ = set_rlimit_nofile(4096);

    while let Some((acceptor, client)) = listener.next().await.transpose()? {
        tokio::spawn(async move {
            if let Err(e) = acceptor.accept().await {
                eprintln!("{} => {}", client, e)
            }
        });
    }

    Ok(())
}
