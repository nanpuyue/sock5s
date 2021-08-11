use std::fmt::{self, Display, Formatter};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::pin::Pin;
use std::task::{Context, Poll};

use clap::{App, Arg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use tokio_stream::{Stream, StreamExt};

#[cfg(target_family = "unix")]
use self::util::set_rlimit_nofile;
use self::{
    acceptor::Socks5Acceptor,
    error::Result,
    listener::Socks5Listener,
    target::{Socks5Connector, Socks5Target},
    udp::Socks5UdpClient,
    util::{link_stream, udp_bind_v6, IntoResult, Split},
};

pub type Socks5Stream = TcpStream;

mod acceptor;
mod error;
mod listener;
mod target;
mod udp;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("sock5s")
        .version("0.2.1")
        .author("南浦月 <nanpuyue@gmail.com>")
        .about("A Simple Socks5 Proxy Server")
        .arg(
            Arg::with_name("listen")
                .short("l")
                .long("listen")
                .value_name("LISTEN ADDR")
                .help("Specify the listen addr")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let listen = matches.value_of("listen").unwrap();

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
