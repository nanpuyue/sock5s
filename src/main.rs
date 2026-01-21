use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use std::io::{ErrorKind, IoSlice};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::pin::Pin;
use std::task::{Context, Poll};

use clap::Parser;
use indoc::indoc;
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
    target::{Socks5Host, Socks5Target},
    util::{IntoResult, PutSocks5Addr, Split},
};

pub type Socks5Stream = TcpStream;

mod acceptor;
mod connector;
mod error;
mod listener;
mod target;
mod udp;
mod util;

#[derive(Parser, Debug)]
#[command(
    version,
    author,
    about,
    help_template = indoc! {"
        {before-help}{name} {version}
        {author}
        {about}

        {usage-heading} {usage}

        {all-args}{after-help}
    "}
)]
struct Cli {
    #[arg(
        short = 'l',
        long = "listen",
        value_name = "HOST:PORT",
        help = "Listen address",
        required = true
    )]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut listener = Socks5Listener::listen(cli.listen).await?;

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
