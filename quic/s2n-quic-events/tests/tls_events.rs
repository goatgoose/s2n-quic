// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod tls;

pub use s2n_quic_core::{application, inet};
pub use tls::event;

use s2n_quic_core::{
    event::IntoEvent,
    time::{testing::Clock as MockClock, Clock},
};
use tls::event::ConnectionPublisher;

#[test]
fn publish_test_enum_event() {
    struct MySubscriber {
        received_value: Option<event::api::TestEnum>,
    }

    impl event::Subscriber for MySubscriber {
        type ConnectionContext = ();

        fn create_connection_context(
            &mut self,
            _meta: &event::api::ConnectionMeta,
            _info: &event::api::ConnectionInfo,
        ) -> Self::ConnectionContext {
            ()
        }

        fn on_enum_event(
            &mut self,
            _context: &mut Self::ConnectionContext,
            _meta: &event::api::ConnectionMeta,
            event: &event::api::EnumEvent,
        ) {
            self.received_value = Some(event.value.clone());
        }
    }

    let mut subscriber = MySubscriber {
        received_value: None,
    };

    let timestamp = MockClock::default().get_time().into_event();
    let mut context = ();
    let mut publisher = event::ConnectionPublisherSubscriber::new(
        event::builder::ConnectionMeta { id: 0, timestamp },
        0,
        &mut subscriber,
        &mut context,
    );

    publisher.on_enum_event(event::builder::EnumEvent {
        value: event::builder::TestEnum::TestValue1,
    });

    assert!(matches!(
        subscriber.received_value.unwrap(),
        event::api::TestEnum::TestValue1 {}
    ));
}
