use humantime::Duration;
use rayon::prelude::*;
use s2n_quic::{
    client::Connect,
    provider::io::testing::{rand, spawn, spawn_primary, test_seed, time, Handle, Model, Result},
    Client, Server,
};
use s2n_quic_core::{crypto::tls::testing::certificates, stream::testing::Data};
use std::net::SocketAddr;
use structopt::StructOpt;

mod events;

mod range;
use range::CliRange;

#[derive(Debug, StructOpt)]
pub struct Run {
    #[structopt(long, default_value = "0.0")]
    drop_rate: CliRange<f64>,

    #[structopt(long, default_value = "0.0")]
    corrupt_rate: CliRange<f64>,

    #[structopt(long, default_value = "0ms")]
    jitter: CliRange<Duration>,

    #[structopt(long, default_value = "0ms")]
    network_jitter: CliRange<Duration>,

    #[structopt(long, default_value = "100ms")]
    delay: CliRange<Duration>,

    #[structopt(long)]
    transmit_rate: Option<CliRange<u64>>,

    #[structopt(long, default_value = "0.0")]
    retransmit_rate: CliRange<f64>,

    #[structopt(long, default_value = "1450")]
    max_udp_payload: CliRange<u16>,

    #[structopt(long)]
    max_inflight: Option<CliRange<u64>>,

    #[structopt(long, default_value = "1")]
    clients: CliRange<usize>,

    #[structopt(long, default_value = "1")]
    servers: CliRange<usize>,

    #[structopt(long, default_value = "0ms")]
    connect_delay: CliRange<Duration>,

    #[structopt(long, default_value = "1")]
    connections: CliRange<usize>,

    #[structopt(long, default_value = "1")]
    streams: CliRange<usize>,

    #[structopt(long, default_value = "4096")]
    stream_data: CliRange<u64>,

    #[structopt(long)]
    iterations: Option<u64>,
}

impl Run {
    pub fn run(&self) {
        assert_ne!(self.servers.start, 0);
        assert_ne!(self.clients.start, 0);
        assert_ne!(self.connections.start, 0);

        let iterations = 0..self.iterations.unwrap_or(u64::MAX);

        iterations.into_par_iter().for_each(|_| {
            let network = Model::default();
            let seed = {
                use ::rand::prelude::*;
                thread_rng().gen()
            };

            test_seed(network.clone(), seed, |handle| {
                let server_len = self.servers.gen();
                let client_len = self.clients.gen();

                let events = self.gen_network(seed, server_len, client_len, &network);

                let mut servers = vec![];
                for _ in 0..server_len {
                    servers.push(server(handle, events.clone())?);
                }

                for _ in 0..client_len {
                    // pick a random server to connect to
                    let server = rand::gen_range(0..servers.len());
                    let server = servers[server];
                    let count = self.connections.gen();
                    let delay = self.connect_delay;
                    let streams = self.streams;
                    let stream_data = self.stream_data;
                    client(
                        handle,
                        events.clone(),
                        server,
                        count,
                        delay,
                        streams,
                        stream_data,
                    )?;
                }

                Ok(())
            })
            .unwrap();
        });
    }

    fn gen_network(
        &self,
        seed: u64,
        servers: usize,
        clients: usize,
        model: &Model,
    ) -> events::Events {
        let mut events = events::Parameters {
            seed,
            servers,
            clients,
            ..Default::default()
        };

        macro_rules! param {
            ($name:ident, $set:ident, $gen:ident) => {{
                let value = self.$name.$gen();
                model.$set(value);
                events.$name = value;
            }};
        }

        param!(drop_rate, set_drop_rate, gen);
        param!(corrupt_rate, set_corrupt_rate, gen);
        param!(jitter, set_jitter, gen_duration);
        param!(network_jitter, set_network_jitter, gen_duration);
        param!(delay, set_delay, gen_duration);
        param!(retransmit_rate, set_retransmit_rate, gen);
        param!(max_udp_payload, set_max_udp_payload, gen);

        if let Some(value) = self.transmit_rate.as_ref() {
            let value = value.gen();
            model.set_transmit_rate(value);
            events.transmit_rate = value;
        } else {
            events.transmit_rate = u64::MAX;
        }

        if let Some(value) = self.max_inflight.as_ref() {
            let value = value.gen();
            model.set_max_inflight(value);
            events.max_inflight = value;
        } else {
            events.max_inflight = u64::MAX;
        }

        events.into()
    }
}

fn server(handle: &Handle, events: events::Events) -> Result<SocketAddr> {
    let mut server = Server::builder()
        .with_io(handle.builder().build().unwrap())?
        .with_tls((certificates::CERT_PEM, certificates::KEY_PEM))?
        .with_event(events)?
        .start()?;
    let server_addr = server.local_addr()?;

    // accept connections and echo back
    spawn(async move {
        while let Some(mut connection) = server.accept().await {
            spawn_primary(async move {
                while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                    spawn(async move {
                        while let Ok(Some(chunk)) = stream.receive().await {
                            let _ = stream.send(chunk).await;
                        }
                    });
                }
            });
        }
    });

    Ok(server_addr)
}

fn client(
    handle: &Handle,
    events: events::Events,
    server_addr: SocketAddr,
    count: usize,
    delay: CliRange<Duration>,
    streams: CliRange<usize>,
    stream_data: CliRange<u64>,
) -> Result {
    let client = Client::builder()
        .with_io(handle.builder().build().unwrap())?
        .with_tls(certificates::CERT_PEM)?
        .with_event(events)?
        .start()?;

    for _ in 0..count {
        let delay = delay.gen_duration();
        let connect = Connect::new(server_addr).with_server_name("localhost");
        let connection = client.connect(connect);
        spawn_primary(async move {
            if !delay.is_zero() {
                time::delay(delay).await;
            }

            let mut connection = connection.await?;

            for _ in 0..streams.gen() {
                let stream = connection.open_bidirectional_stream().await?;
                spawn_primary(async move {
                    let (mut recv, mut send) = stream.split();

                    let mut send_data = Data::new(stream_data.gen());

                    let mut recv_data = send_data;
                    spawn_primary(async move {
                        while let Some(chunk) = recv.receive().await? {
                            recv_data.receive(&[chunk]);
                        }

                        <s2n_quic::stream::Result<()>>::Ok(())
                    });

                    while let Some(chunk) = send_data.send_one(usize::MAX) {
                        send.send(chunk).await?;
                    }

                    <s2n_quic::stream::Result<()>>::Ok(())
                });
            }

            <s2n_quic::stream::Result<()>>::Ok(())
        });
    }

    Ok(())
}
