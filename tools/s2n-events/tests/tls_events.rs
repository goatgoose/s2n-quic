// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod tls;

use std::sync::{Arc, Mutex};
use s2n_quic_core::{
    event::IntoEvent,
    time::{testing::Clock as MockClock, Clock},
};
pub use tls::event;
use tls::event::ConnectionPublisher;
use crate::event::Subscriber;

#[test]
fn publish_byte_array_event() {
    struct ByteArraySubscriber {
        received_data: Vec<u8>,
    }

    impl Subscriber for ByteArraySubscriber {
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
    use event::c_ffi;

    #[derive(Clone)]
    struct ByteArraySubscriber {
        received_data: Arc<Mutex<Option<Vec<u8>>>>,
    }

    impl Subscriber for ByteArraySubscriber {
        type ConnectionContext = Arc<Mutex<Option<Vec<u8>>>>;

        fn create_connection_context(
            &mut self,
            _meta: &event::api::ConnectionMeta,
            _info: &event::api::ConnectionInfo,
        ) -> Self::ConnectionContext {
            self.received_data.clone()
        }

        fn on_byte_array_event(
            &mut self,
            context: &mut Self::ConnectionContext,
            _meta: &event::api::ConnectionMeta,
            event: &event::api::ByteArrayEvent,
        ) {
            *context.lock().unwrap() = Some(event.data.to_vec());
        }
    }

    let subscriber = ByteArraySubscriber {
        received_data: Arc::new(Mutex::new(None)),
    };

    let c_subscriber = c_ffi::s2n_event_subscriber::new(subscriber.clone());

    unsafe {
        let meta = c_ffi::s2n_event_connection_meta {
            id: 0,
            timestamp: 1142,
        };
        let info = c_ffi::s2n_event_connection_info {};
        let publisher = c_ffi::s2n_event_connection_publisher_new(c_subscriber, &meta, &info);

        let mut data = vec![2, 3, 4];
        let event = c_ffi::s2n_event_byte_array {
            data: data.as_mut_ptr(),
            data_len: 3,
        };
        c_ffi::s2n_connection_publisher_on_byte_array_event(publisher, &event);

        assert_eq!(subscriber.received_data.lock().unwrap().take().unwrap(), vec![2, 3, 4]);

        c_ffi::s2n_event_connection_publisher_free(publisher);
        c_ffi::s2n_event_subscriber_free(c_subscriber);
    }
}
