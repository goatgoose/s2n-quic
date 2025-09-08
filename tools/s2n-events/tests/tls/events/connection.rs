// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(non_camel_case_types)]
#[repr(C)]
struct s2n_event_byte_array {
    pub data: *mut u8,
    pub data_len: u32,
}

impl<'a> IntoEvent<builder::ByteArrayEvent<'a>> for *const c_ffi::s2n_event_byte_array {
    fn into_event(self) -> builder::ByteArrayEvent<'a> {
        unsafe {
            let event = &*self;
            builder::ByteArrayEvent {
                data: std::slice::from_raw_parts(event.data, event.data_len.try_into().unwrap())
            }
        }
    }
}

#[event("byte_array_event")]
#[c_argument(s2n_event_byte_array)]
struct ByteArrayEvent<'a> {
    data: &'a [u8],
}
