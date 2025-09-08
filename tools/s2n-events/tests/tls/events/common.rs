// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

enum Subject {
    Endpoint,
    Connection {
        id: u64,
    },
}

struct ConnectionMeta {
    id: u64,
    timestamp: Timestamp,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct s2n_event_connection_meta {
    id: u64,
    timestamp: u64,
}

impl IntoEvent<builder::ConnectionMeta> for *const c_ffi::s2n_event_connection_meta {
    fn into_event(self) -> builder::ConnectionMeta {
        let event = unsafe { &*self };
        let duration = Duration::from_nanos(event.timestamp);
        let timestamp = unsafe {
            s2n_quic_core::time::Timestamp::from_duration(duration).into_event()
        };
        builder::ConnectionMeta { id: event.id, timestamp }
    }
}

struct EndpointMeta {}

struct ConnectionInfo {}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct s2n_event_connection_info {}

impl IntoEvent<builder::ConnectionInfo> for *const c_ffi::s2n_event_connection_info {
    fn into_event(self) -> builder::ConnectionInfo {
        builder::ConnectionInfo {}
    }
}
