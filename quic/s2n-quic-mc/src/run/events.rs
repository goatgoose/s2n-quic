use core::time::Duration;
use once_cell::sync::Lazy;
use s2n_quic::{
    connection,
    provider::{event, io::testing::time},
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

static IDS: Lazy<Arc<AtomicUsize>> = Lazy::new(Default::default);

#[derive(Clone, Debug)]
pub struct Events {
    params: Arc<DumpOnDrop<Parameters>>,
}

fn now() -> Duration {
    unsafe { time::now().as_duration() }
}

impl event::Subscriber for Events {
    type ConnectionContext = DumpOnDrop<Connection>;

    fn create_connection_context(
        &mut self,
        meta: &event::ConnectionMeta,
        _info: &event::ConnectionInfo,
    ) -> Self::ConnectionContext {
        let seed = self.params.seed;
        let id = IDS.fetch_add(1, Ordering::Relaxed);

        let mut conn = Connection {
            seed,
            start_time: now(),
            ..Default::default()
        };

        if matches!(
            meta.endpoint_type,
            event::events::EndpointType::Server { .. }
        ) {
            conn.server_id = Some(id);
        } else {
            conn.client_id = Some(id);
        }

        DumpOnDrop(conn)
    }

    #[inline]
    fn on_packet_sent(
        &mut self,
        context: &mut Self::ConnectionContext,
        _meta: &event::ConnectionMeta,
        event: &event::events::PacketSent,
    ) {
        context.tx.inc(&event.packet_header);
    }

    #[inline]
    fn on_packet_received(
        &mut self,
        context: &mut Self::ConnectionContext,
        _meta: &event::ConnectionMeta,
        event: &event::events::PacketReceived,
    ) {
        context.rx.inc(&event.packet_header);
    }

    #[inline]
    fn on_packet_lost(
        &mut self,
        context: &mut Self::ConnectionContext,
        _meta: &event::ConnectionMeta,
        event: &event::events::PacketLost,
    ) {
        context.loss.inc(&event.packet_header);
    }

    #[inline]
    fn on_congestion(
        &mut self,
        context: &mut Self::ConnectionContext,
        _meta: &event::ConnectionMeta,
        _event: &event::events::Congestion,
    ) {
        context.congestion += 1;
    }

    #[inline]
    fn on_handshake_status_updated(
        &mut self,
        context: &mut Self::ConnectionContext,
        _meta: &event::ConnectionMeta,
        event: &event::events::HandshakeStatusUpdated,
    ) {
        match event.status {
            event::events::HandshakeStatus::Complete { .. } => context.handshake.complete = now(),
            event::events::HandshakeStatus::Confirmed { .. } => context.handshake.confirmed = now(),
            _ => {}
        }
    }

    #[inline]
    fn on_connection_closed(
        &mut self,
        context: &mut Self::ConnectionContext,
        meta: &event::ConnectionMeta,
        event: &event::events::ConnectionClosed,
    ) {
        context.end_time = meta.timestamp.duration_since_start();

        match event.error {
            connection::Error::Closed { .. } => {}
            connection::Error::Transport { code, .. } => {
                context.transport_error = Some(code.as_u64());
            }
            connection::Error::Application { error, .. } => {
                context.application_error = Some(*error);
            }
            connection::Error::IdleTimerExpired { .. } => context.idle_timer_error = true,
            _ => context.unspecified_error = true,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Connection {
    #[serde(rename = "c", skip_serializing_if = "is_default", default)]
    pub client_id: Option<usize>,
    #[serde(rename = "s", skip_serializing_if = "is_default", default)]
    pub server_id: Option<usize>,
    #[serde(rename = "S")]
    pub seed: u64,
    #[serde(
        rename = "srt",
        skip_serializing_if = "Duration::is_zero",
        default,
        with = "duration_format"
    )]
    pub start_time: Duration,
    #[serde(
        rename = "end",
        skip_serializing_if = "Duration::is_zero",
        default,
        with = "duration_format"
    )]
    pub end_time: Duration,
    #[serde(rename = "hnd", skip_serializing_if = "is_default", default)]
    pub handshake: Handshake,
    #[serde(rename = "te", skip_serializing_if = "Option::is_none", default)]
    pub transport_error: Option<u64>,
    #[serde(rename = "ae", skip_serializing_if = "Option::is_none", default)]
    pub application_error: Option<u64>,
    #[serde(rename = "ie", skip_serializing_if = "is_default", default)]
    pub idle_timer_error: bool,
    #[serde(rename = "ue", skip_serializing_if = "is_default", default)]
    pub unspecified_error: bool,
    #[serde(skip_serializing_if = "is_default", default)]
    pub tx: PacketCounts,
    #[serde(skip_serializing_if = "is_default", default)]
    pub rx: PacketCounts,
    #[serde(rename = "lss", skip_serializing_if = "is_default", default)]
    pub loss: PacketCounts,
    #[serde(rename = "cgs", skip_serializing_if = "is_default", default)]
    pub congestion: u64,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct PacketCounts {
    #[serde(rename = "i", skip_serializing_if = "is_default", default)]
    pub initial: u64,
    #[serde(rename = "h", skip_serializing_if = "is_default", default)]
    pub handshake: u64,
    #[serde(rename = "r", skip_serializing_if = "is_default", default)]
    pub retry: u64,
    #[serde(rename = "o", skip_serializing_if = "is_default", default)]
    pub one_rtt: u64,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Handshake {
    #[serde(
        rename = "c",
        skip_serializing_if = "Duration::is_zero",
        default,
        with = "duration_format"
    )]
    pub complete: Duration,
    #[serde(
        rename = "C",
        skip_serializing_if = "Duration::is_zero",
        default,
        with = "duration_format"
    )]
    pub confirmed: Duration,
}

impl PacketCounts {
    #[inline]
    fn inc(&mut self, header: &event::events::PacketHeader) {
        match header {
            event::events::PacketHeader::Initial { .. } => self.initial += 1,
            event::events::PacketHeader::Handshake { .. } => self.handshake += 1,
            event::events::PacketHeader::OneRtt { .. } => self.one_rtt += 1,
            _ => {}
        }
    }
}

impl Dump for Connection {
    fn dump(&mut self) {
        if self.seed == 0 {
            return;
        }

        if self.end_time.is_zero() {
            self.end_time = now();
        }

        dump(&self);
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Parameters {
    #[serde(rename = "S")]
    pub seed: u64,
    #[serde(skip_serializing_if = "is_default", default)]
    pub drop_rate: f64,
    #[serde(skip_serializing_if = "is_default", default)]
    pub corrupt_rate: f64,
    #[serde(skip_serializing_if = "is_default", default, with = "duration_format")]
    pub jitter: Duration,
    #[serde(skip_serializing_if = "is_default", default, with = "duration_format")]
    pub network_jitter: Duration,
    #[serde(
        rename = "dly",
        skip_serializing_if = "is_default",
        default,
        with = "duration_format"
    )]
    pub delay: Duration,
    #[serde(skip_serializing_if = "is_default", default)]
    pub retransmit_rate: f64,
    #[serde(skip_serializing_if = "is_default_udp_payload", default)]
    pub max_udp_payload: u16,
    #[serde(skip_serializing_if = "is_max", default)]
    pub transmit_rate: u64,
    #[serde(skip_serializing_if = "is_max", default)]
    pub max_inflight: u64,
    #[serde(skip_serializing_if = "is_one", default)]
    pub servers: usize,
    #[serde(skip_serializing_if = "is_one", default)]
    pub clients: usize,
    #[serde(
        rename = "end",
        skip_serializing_if = "Duration::is_zero",
        default,
        with = "duration_format"
    )]
    pub end_time: Duration,
}

impl From<Parameters> for Events {
    fn from(s: Parameters) -> Self {
        Self {
            params: Arc::new(DumpOnDrop(s)),
        }
    }
}

impl Dump for Parameters {
    fn dump(&mut self) {
        if self.seed == 0 {
            return;
        }

        if self.end_time.is_zero() {
            self.end_time = now();
        }

        dump(&self);
    }
}

pub trait Dump {
    fn dump(&mut self);
}

#[derive(Debug)]
pub struct DumpOnDrop<T: Dump>(T);

impl<T: Dump> core::ops::Deref for DumpOnDrop<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Dump> core::ops::DerefMut for DumpOnDrop<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Dump> Drop for DumpOnDrop<T> {
    fn drop(&mut self) {
        self.0.dump();
    }
}

fn dump<T: Serialize>(v: &T) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, v).unwrap();
    let _ = writeln!(stdout);
}

#[inline]
fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    T::default().eq(v)
}

#[inline]
fn is_max(v: &u64) -> bool {
    u64::MAX.eq(v)
}

#[inline]
fn is_default_udp_payload(v: &u16) -> bool {
    1450.eq(v)
}

#[inline]
fn is_one(v: &usize) -> bool {
    1usize.eq(v)
}

pub(crate) mod duration_format {
    use core::time::Duration;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_nanos() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nanos = u64::deserialize(deserializer)?;
        Ok(Duration::from_nanos(nanos))
    }
}
