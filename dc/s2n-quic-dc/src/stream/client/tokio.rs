// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    either::Either,
    event, msg,
    path::secret,
    stream::{
        application::Stream,
        endpoint,
        environment::tokio::{self as env, Environment},
        recv,
        socket::Protocol,
    },
};
use std::{io, net::SocketAddr, time::Duration};
use tokio::net::TcpStream;

/// Connects using the UDP transport layer
///
/// Callers should send data immediately after calling this to ensure minimal
/// credential reordering.
#[inline]
pub async fn connect_udp<H, Sub>(
    handshake: H,
    acceptor_addr: SocketAddr,
    env: &Environment<Sub>,
) -> io::Result<Stream<Sub>>
where
    H: core::future::Future<Output = io::Result<secret::map::Peer>>,
    Sub: event::Subscriber + Clone,
{
    // ensure we have a secret for the peer
    let entry = handshake.await?;

    // TODO potentially branch on not using the recv pool if we're under a certain concurrency?
    let stream = if env.has_recv_pool() {
        let peer = env::udp::Pooled(acceptor_addr.into());
        endpoint::open_stream(env, entry, peer, None)?
    } else {
        let peer = env::udp::Owned(acceptor_addr.into(), recv_buffer());
        endpoint::open_stream(env, entry, peer, None)?
    };

    // build the stream inside the application context
    let stream = stream.connect()?;

    debug_assert_eq!(stream.protocol(), Protocol::Udp);

    Ok(stream)
}

/// Connects using the TCP transport layer
///
/// Callers should send data immediately after calling this to ensure minimal
/// credential reordering.
#[inline]
pub async fn connect_tcp<H, Sub>(
    handshake: H,
    acceptor_addr: SocketAddr,
    env: &Environment<Sub>,
    linger: Option<Duration>,
) -> io::Result<Stream<Sub>>
where
    H: core::future::Future<Output = io::Result<secret::map::Peer>>,
    Sub: event::Subscriber + Clone,
{
    // Race TCP handshake with the TLS handshake
    let (socket, entry) = tokio::try_join!(TcpStream::connect(acceptor_addr), handshake,)?;

    // Make sure TCP_NODELAY is set
    let _ = socket.set_nodelay(true);

    if linger.is_some() {
        let _ = socket.set_linger(linger);
    }

    // if the acceptor_ip isn't known, then ask the socket to resolve it for us
    let peer_addr = if acceptor_addr.ip().is_unspecified() {
        socket.peer_addr()?
    } else {
        acceptor_addr
    }
    .into();
    let local_port = socket.local_addr()?.port();

    let peer = env::tcp::Registered {
        socket,
        peer_addr,
        local_port,
        recv_buffer: recv_buffer(),
    };

    let stream = endpoint::open_stream(env, entry, peer, None)?;

    // build the stream inside the application context
    let stream = stream.connect()?;

    debug_assert_eq!(stream.protocol(), Protocol::Tcp);

    Ok(stream)
}

/// Connects with a pre-existing TCP stream
///
/// Callers should send data immediately after calling this to ensure minimal
/// credential reordering.
///
/// # Note
///
/// The provided `map` must contain a shared secret for the `handshake_addr`
#[inline]
pub async fn connect_tcp_with<Sub>(
    entry: secret::map::Peer,
    socket: TcpStream,
    env: &Environment<Sub>,
) -> io::Result<Stream<Sub>>
where
    Sub: event::Subscriber + Clone,
{
    let local_port = socket.local_addr()?.port();
    let peer_addr = socket.peer_addr()?.into();

    let peer = env::tcp::Registered {
        socket,
        peer_addr,
        local_port,
        recv_buffer: recv_buffer(),
    };

    let stream = endpoint::open_stream(env, entry, peer, None)?;

    // build the stream inside the application context
    let stream = stream.connect()?;

    debug_assert_eq!(stream.protocol(), Protocol::Tcp);

    Ok(stream)
}

#[inline]
fn recv_buffer() -> recv::shared::RecvBuffer {
    // TODO replace this with a parameter once everything is in place
    let recv_buffer = recv::buffer::Local::new(msg::recv::Message::new(9000), None);
    Either::A(recv_buffer)
}
