// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(non_camel_case_types)]
struct s2n_event_count {
    count: u32,
}

impl<'a> IntoEvent<builder::CountEvent<'a>> for *const c_ffi::s2n_event_count {
    fn into_event(self) -> builder::CountEvent<'a> {
        unsafe {
            let event = &*self;
            builder::CountEvent {
                count: event.count,
            }
        }
    }
}

#[event("count_event")]
#[subject(endpoint)]
#[c_argument(s2n_event_count)]
struct CountEvent {
    count: u32,
}
