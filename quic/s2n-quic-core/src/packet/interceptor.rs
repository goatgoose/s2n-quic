// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    event::api::{SocketAddress, Subject},
    havoc,
    packet::number::{PacketNumber, PacketNumberSpace},
    path::{LocalAddress, RemoteAddress},
    time::Timestamp,
    varint::VarInt,
};
use core::ops::RangeInclusive;
use s2n_codec::encoder::scatter;

pub use s2n_codec::{DecoderBufferMut, EncoderBuffer};
pub mod loss;
pub use loss::Loss;

/// TODO add `non_exhaustive` once/if this feature is stable
#[derive(Debug)]
pub struct Packet {
    pub number: PacketNumber,
    pub timestamp: Timestamp,
}

/// TODO add `non_exhaustive` once/if this feature is stable
#[derive(Debug)]
pub struct Datagram<'a> {
    pub remote_address: SocketAddress<'a>,
    pub local_address: SocketAddress<'a>,
    pub timestamp: Timestamp,
}

pub trait Ack {
    fn space(&self) -> PacketNumberSpace;

    fn insert_range(&mut self, range: RangeInclusive<VarInt>);
}

/// Trait which enables an application to intercept packets that are transmitted and received
pub trait Interceptor: 'static + Send {
    #[inline(always)]
    fn intercept_rx_ack<A: Ack>(&mut self, subject: &Subject, ack: &mut A) {
        let _ = subject;
        let _ = ack;
    }

    #[inline(always)]
    fn intercept_rx_local_address(&mut self, subject: &Subject, addr: &mut LocalAddress) {
        let _ = subject;
        let _ = addr;
    }

    #[inline(always)]
    fn intercept_rx_remote_address(&mut self, subject: &Subject, addr: &mut RemoteAddress) {
        let _ = subject;
        let _ = addr;
    }

    #[inline(always)]
    fn intercept_rx_datagram<'a>(
        &mut self,
        subject: &Subject,
        datagram: &Datagram,
        payload: DecoderBufferMut<'a>,
    ) -> DecoderBufferMut<'a> {
        let _ = subject;
        let _ = datagram;
        payload
    }

    #[inline(always)]
    fn intercept_rx_payload<'a>(
        &mut self,
        subject: &Subject,
        packet: &Packet,
        payload: DecoderBufferMut<'a>,
    ) -> DecoderBufferMut<'a> {
        let _ = subject;
        let _ = packet;
        payload
    }

    #[inline(always)]
    fn intercept_tx_datagram(
        &mut self,
        subject: &Subject,
        datagram: &Datagram,
        payload: &mut EncoderBuffer,
    ) {
        let _ = subject;
        let _ = datagram;
        let _ = payload;
    }

    #[inline(always)]
    fn intercept_tx_payload(
        &mut self,
        subject: &Subject,
        packet: &Packet,
        payload: &mut scatter::Buffer,
    ) {
        let _ = subject;
        let _ = packet;
        let _ = payload;
    }
}

#[derive(Debug, Default)]
pub struct Disabled(());

impl Interceptor for Disabled {}

impl<X, Y> Interceptor for (X, Y)
where
    X: Interceptor,
    Y: Interceptor,
{
    #[inline(always)]
    fn intercept_rx_ack<A: Ack>(&mut self, subject: &Subject, ack: &mut A) {
        self.0.intercept_rx_ack(subject, ack);
        self.1.intercept_rx_ack(subject, ack);
    }

    #[inline(always)]
    fn intercept_rx_local_address(&mut self, subject: &Subject, addr: &mut LocalAddress) {
        self.0.intercept_rx_local_address(subject, addr);
        self.1.intercept_rx_local_address(subject, addr);
    }

    #[inline(always)]
    fn intercept_rx_remote_address(&mut self, subject: &Subject, addr: &mut RemoteAddress) {
        self.0.intercept_rx_remote_address(subject, addr);
        self.1.intercept_rx_remote_address(subject, addr);
    }

    #[inline(always)]
    fn intercept_rx_datagram<'a>(
        &mut self,
        subject: &Subject,
        datagram: &Datagram,
        payload: DecoderBufferMut<'a>,
    ) -> DecoderBufferMut<'a> {
        let payload = self.0.intercept_rx_datagram(subject, datagram, payload);
        self.1.intercept_rx_datagram(subject, datagram, payload)
    }

    #[inline(always)]
    fn intercept_rx_payload<'a>(
        &mut self,
        subject: &Subject,
        packet: &Packet,
        payload: DecoderBufferMut<'a>,
    ) -> DecoderBufferMut<'a> {
        let payload = self.0.intercept_rx_payload(subject, packet, payload);
        self.1.intercept_rx_payload(subject, packet, payload)
    }

    #[inline(always)]
    fn intercept_tx_datagram(
        &mut self,
        subject: &Subject,
        datagram: &Datagram,
        payload: &mut EncoderBuffer,
    ) {
        self.0.intercept_tx_datagram(subject, datagram, payload);
        self.1.intercept_tx_datagram(subject, datagram, payload);
    }

    #[inline(always)]
    fn intercept_tx_payload(
        &mut self,
        subject: &Subject,
        packet: &Packet,
        payload: &mut scatter::Buffer,
    ) {
        self.0.intercept_tx_payload(subject, packet, payload);
        self.1.intercept_tx_payload(subject, packet, payload);
    }
}

#[derive(Debug, Default)]
pub struct Havoc<Rx, Tx, P, R>
where
    Rx: 'static + Send + havoc::Strategy,
    Tx: 'static + Send + havoc::Strategy,
    P: 'static + Send + havoc::Strategy,
    R: 'static + Send + havoc::Random,
{
    pub rx: Rx,
    pub tx: Tx,
    pub port: P,
    pub random: R,
}

impl<Rx, Tx, P, R> Interceptor for Havoc<Rx, Tx, P, R>
where
    Rx: 'static + Send + havoc::Strategy,
    Tx: 'static + Send + havoc::Strategy,
    P: 'static + Send + havoc::Strategy,
    R: 'static + Send + havoc::Random,
{
    #[inline]
    fn intercept_rx_remote_address(&mut self, _subject: &Subject, addr: &mut RemoteAddress) {
        let port = addr.port();
        let port = self.port.havoc_u16(&mut self.random, port);
        addr.set_port(port);
    }

    #[inline]
    fn intercept_rx_payload<'a>(
        &mut self,
        _subject: &Subject,
        _packet: &Packet,
        payload: DecoderBufferMut<'a>,
    ) -> DecoderBufferMut<'a> {
        let payload = payload.into_less_safe_slice();
        let len = payload.len();

        let len = {
            use s2n_codec::Encoder;
            let mut payload = EncoderBuffer::new(payload);
            payload.set_position(len);
            self.rx.havoc(&mut self.random, &mut payload);
            payload.len()
        };

        let payload = &mut payload[..len];

        DecoderBufferMut::new(payload)
    }

    #[inline]
    fn intercept_tx_payload(
        &mut self,
        _subject: &Subject,
        _packet: &Packet,
        payload: &mut scatter::Buffer,
    ) {
        let payload = payload.flatten();
        self.tx.havoc(&mut self.random, payload);
    }
}

impl<T: Interceptor> Interceptor for Option<T> {
    #[inline]
    fn intercept_rx_local_address(&mut self, subject: &Subject, addr: &mut LocalAddress) {
        if let Some(inner) = self.as_mut() {
            inner.intercept_rx_local_address(subject, addr)
        }
    }

    #[inline]
    fn intercept_rx_remote_address(&mut self, subject: &Subject, addr: &mut RemoteAddress) {
        if let Some(inner) = self.as_mut() {
            inner.intercept_rx_remote_address(subject, addr)
        }
    }

    #[inline]
    fn intercept_rx_payload<'a>(
        &mut self,
        subject: &Subject,
        packet: &Packet,
        payload: DecoderBufferMut<'a>,
    ) -> DecoderBufferMut<'a> {
        if let Some(inner) = self.as_mut() {
            inner.intercept_rx_payload(subject, packet, payload)
        } else {
            payload
        }
    }

    #[inline]
    fn intercept_tx_payload(
        &mut self,
        subject: &Subject,
        packet: &Packet,
        payload: &mut scatter::Buffer,
    ) {
        if let Some(inner) = self.as_mut() {
            inner.intercept_tx_payload(subject, packet, payload)
        }
    }
}
