// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod tls;

use s2n_quic_core::{
    event::IntoEvent,
    time::{testing::Clock as MockClock, Clock},
};
pub use tls::event;
use tls::event::ConnectionPublisher;
use crate::event::api::{ByteArrayEvent, ConnectionInfo, ConnectionMeta};
use crate::event::Subscriber;

struct ByteArraySubscriber {
    received_data: Vec<u8>,
}

impl event::Subscriber for ByteArraySubscriber {
    type ConnectionContext = ();

    fn create_connection_context(
        &mut self,
        _meta: &event::api::ConnectionMeta,
        _info: &event::api::ConnectionInfo,
    ) -> Self::ConnectionContext {
    }

    fn on_byte_array_event(
        &mut self,
        _context: &mut Self::ConnectionContext,
        _meta: &event::api::ConnectionMeta,
        event: &event::api::ByteArrayEvent,
    ) {
        self.received_data.extend_from_slice(event.data);
    }
}

#[test]
fn publish_byte_array_event() {
    let mut subscriber = ByteArraySubscriber {
        received_data: Vec::new(),
    };

    let timestamp = MockClock::default().get_time().into_event();
    let mut context = ();
    let mut publisher = event::ConnectionPublisherSubscriber::new(
        event::builder::ConnectionMeta { id: 0, timestamp },
        0,
        &mut subscriber,
        &mut context,
    );

    publisher.on_byte_array_event(event::builder::ByteArrayEvent { data: &[1, 2, 3] });

    assert_eq!(subscriber.received_data, vec![1, 2, 3]);
}

#[test]
fn publish_byte_array_event_with_c_ffi() {
    let subscriber = ByteArraySubscriber {
        received_data: Vec::new(),
    };

    let c_subscriber = event::c_ffi::s2n_event_subscriber::new(subscriber);
}
