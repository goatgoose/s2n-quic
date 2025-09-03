// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use proc_macro2::TokenStream;
use quote::quote;
use s2n_events::{parser, validation, Output, OutputConfig, OutputMode, PublisherTarget, Result};

struct EventInfo<'a> {
    input_path: &'a str,
    output_path: &'a str,
    crate_name: &'a str,
    s2n_quic_core_path: TokenStream,
    tracing_subscriber_def: TokenStream,
    config: OutputConfig,
}

impl EventInfo<'_> {
    fn test_tls_events() -> Self {
        let tracing_subscriber_def = quote!(
        /// Emits events with [`tracing`](https://docs.rs/tracing)
        #[derive(Clone, Debug)]
        pub struct Subscriber {
            root: tracing::Span,
        }

        impl Default for Subscriber {
            fn default() -> Self {
                let root = tracing::span!(target: "tls_test", tracing::Level::DEBUG, "tls_test");

                Self {
                    root,
                }
            }
        }

        impl Subscriber {
            fn parent<M: crate::event::Meta>(&self, _meta: &M) -> Option<tracing::Id> {
                self.root.id()
            }
        }
        );

        EventInfo {
            crate_name: "tls",
            input_path: concat!(env!("CARGO_MANIFEST_DIR"), "/tests/tls/events/**/*.rs"),
            output_path: concat!(env!("CARGO_MANIFEST_DIR"), "/tests/tls/event"),
            s2n_quic_core_path: quote!(s2n_quic_core),
            tracing_subscriber_def,
            config: OutputConfig {
                mode: OutputMode::Mut,
                publisher: PublisherTarget::C,
            },
        }
    }
}

fn main() -> Result<()> {
    let event_paths = [EventInfo::test_tls_events()];

    for event_info in event_paths {
        let mut files = vec![];

        let input_path = event_info.input_path;

        for path in glob::glob(input_path)? {
            let path = path?;
            eprintln!("loading {}", path.canonicalize().unwrap().display());
            let file = std::fs::read_to_string(&path)?;
            files.push(parser::parse(&file, path).unwrap());
        }

        // make sure events are in a deterministic order
        files.sort_by(|a, b| a.path.as_os_str().cmp(b.path.as_os_str()));

        // validate the events
        validation::validate(&files);

        let root = std::path::Path::new(event_info.output_path);
        let _ = std::fs::create_dir_all(root);
        let root = root.canonicalize()?;

        let mut output = Output {
            s2n_quic_core_path: event_info.s2n_quic_core_path,
            tracing_subscriber_def: event_info.tracing_subscriber_def,
            crate_name: event_info.crate_name,
            root,
            config: event_info.config,
            ..Default::default()
        };

        output.generate(&files);
    }

    Ok(())
}
