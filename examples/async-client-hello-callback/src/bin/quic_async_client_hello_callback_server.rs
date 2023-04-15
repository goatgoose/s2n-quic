// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use moka::sync::Cache;
use rand::{distributions::WeightedIndex, prelude::*};
use s2n_quic::{
    provider::tls::s2n_tls::{
        s2n_tls::{
            callbacks::{ConfigResolver, ConnectionFuture},
            config::Config,
            error::Error as S2nError,
        },
        ClientHelloCallback, Connection,
    },
    Server,
};
use s2n_quic::provider::tls::default::s2n_tls::callbacks::{
    PrivateKeyCallback,
    PrivateKeyOperation
};
use s2n_quic_tls::certificate;
use std::task::{Context, Poll};
use openssl::{ec::EcKey, ecdsa::EcdsaSig};
use pin_project::pin_project;
use std::{error::Error, fmt::Display, pin::Pin, sync::Arc, time::Duration};
use tokio::{fs, sync::OnceCell};

/// NOTE: this certificate is to be used for demonstration purposes only!
pub static CERT_PEM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../quic/s2n-quic-core/certs/cert.pem"
);
/// NOTE: this certificate is to be used for demonstration purposes only!
pub static KEY_PEM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../quic/s2n-quic-core/certs/key.pem"
);

type Sni = String;

// A Config cache associated with as SNI (server name indication).
//
// Implements ClientHelloCallback, loading the certificates asynchronously,
// and caching the s2n_tls::config::Config for subsequent calls with the
// same SNI.
//
// An SNI, indicates which hostname the client is attempting to connect to.
// Some deployments could require configuring the s2n_tls::config::Config
// based on the SNI (certificate).
struct ConfigCache {
    cache: Cache<Sni, Arc<OnceCell<Config>>>,
}

impl ConfigCache {
    fn new() -> Self {
        ConfigCache {
            // store Config for up to 100 unique SNI
            cache: Cache::new(100),
        }
    }
}

impl ClientHelloCallback for ConfigCache {
    fn on_client_hello(
        &self,
        connection: &mut Connection,
    ) -> Result<Option<Pin<Box<dyn ConnectionFuture>>>, S2nError> {
        let sni = connection
            .server_name()
            .ok_or_else(|| S2nError::application(Box::new(CustomError)))?
            .to_string();

        let once_cell_config = self
            .cache
            .get_with(sni.clone(), || Arc::new(OnceCell::new()));
        if let Some(config) = once_cell_config.get() {
            eprintln!("Config already cached for SNI: {}", sni);
            connection.set_config(config.clone())?;
            // return `None` if the Config is already in the cache
            return Ok(None);
        }

        // simulate failure 75% of times and success 25% of the times
        let choices = [true, false];
        let weights = [3, 1];
        let dist = WeightedIndex::new(&weights).unwrap();
        let mut rng = thread_rng();

        let fut = async move {
            let fut = once_cell_config.get_or_try_init(|| async {
                let simulated_network_call_failed = choices[dist.sample(&mut rng)];

                if simulated_network_call_failed {
                    eprintln!("simulated network call failed");
                    return Err(S2nError::application(Box::new(CustomError)));
                }

                eprintln!("resolving certificate for SNI: {}", sni);

                // load the cert and key file asynchronously.
                let cert = {
                    // the SNI can be used to load the appropriate cert file
                    let _sni = sni;
                    let cert = fs::read_to_string(CERT_PEM_PATH)
                        .await
                        .map_err(|_| S2nError::application(Box::new(CustomError)))?;
                    cert
                };

                // sleep(async tokio task which doesn't block thread) to mimic delay
                tokio::time::sleep(Duration::from_secs(3)).await;

                let config = s2n_quic::provider::tls::s2n_tls::Server::builder()
                    .with_certificate(cert, certificate::OFFLOAD_PRIVATE_KEY)?
                    .with_private_key_handler(PrivateKeyCallbackHandler {})?
                    .build()?
                    .into();
                Ok(config)
            });
            fut.await.map(|config| config.clone())
        };

        // return `Some(ConnectionFuture)` if the Config wasn't found in the
        // cache and we need to load it asynchronously
        Ok(Some(Box::pin(ConfigResolver::new(fut))))
    }
}

struct PrivateKeyCallbackHandler {}

impl PrivateKeyCallback for PrivateKeyCallbackHandler {
    fn handle_operation(
        &self,
        _connection: &mut Connection,
        operation: PrivateKeyOperation
    ) -> Result<Option<Pin<Box<dyn ConnectionFuture>>>, S2nError> {
        let pkey_future = PrivateKeyFuture {
            op: Some(operation)
        };
        Ok(Some(Box::pin(pkey_future)))
    }
}

#[pin_project]
struct PrivateKeyFuture {
    #[pin]
    op: Option<PrivateKeyOperation>
}

impl ConnectionFuture for PrivateKeyFuture {
    fn poll(
        self: Pin<&mut Self>,
        connection: &mut Connection,
        _ctx: &mut Context
    ) -> Poll<Result<(), S2nError>> {
        let mut this = self.project();

        let op = this.op.take().expect("Missing pkey operation");
        let in_buf_size = op.input_size()?;
        let mut in_buf = vec![0; in_buf_size];
        op.input(&mut in_buf)?;

        let key = std::fs::read(KEY_PEM_PATH).unwrap();
        let key = EcKey::private_key_from_pem(key.as_slice())
            .expect("Failed to create EcKey from pem");
        let sig = EcdsaSig::sign(&in_buf, &key).expect("Failed to sign input");
        let out = sig.to_der().expect("Failed to convert signature to der");

        op.set_output(connection, &out)?;
        Poll::Ready(Ok(()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let tls = s2n_quic::provider::tls::s2n_tls::Server::builder()
        .with_client_hello_handler(ConfigCache::new())?
        .build()?;

    let mut server = Server::builder()
        .with_tls(tls)?
        .with_io("127.0.0.1:4433")?
        .start()?;

    while let Some(mut connection) = server.accept().await {
        // spawn a new task for the connection
        tokio::spawn(async move {
            eprintln!("Connection accepted from {:?}", connection.remote_addr());

            while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                // spawn a new task for the stream
                tokio::spawn(async move {
                    eprintln!("Stream opened from {:?}", stream.connection().remote_addr());

                    // echo any data back to the stream
                    while let Ok(Some(data)) = stream.receive().await {
                        stream.send(data).await.expect("stream should be open");
                    }
                });
            }
        });
    }

    Ok(())
}

#[derive(Debug)]
struct CustomError;

impl Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "custom error")?;
        Ok(())
    }
}

impl Error for CustomError {}
