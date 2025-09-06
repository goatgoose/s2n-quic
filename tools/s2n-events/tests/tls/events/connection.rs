// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(non_camel_case_types)]
#[repr(C)]
struct s2n_event_byte_array {
    pub data: *mut u8,
    pub data_len: u32,
}

impl<'a> IntoEvent<ByteArrayEvent<'a>> for *const s2n_event_byte_array {
    fn into_event(self) -> ByteArrayEvent<'a> {
        unsafe {
            let event = &*self;
            api::ByteArrayEvent {
                data: std::slice::from_raw_parts(event.data, event.data_len.try_into().unwrap())
            }
        }
    }
}

#[event("byte_array_event")]
#[repr_c_variant(s2n_event_byte_array)]
struct ByteArrayEvent<'a> {
    data: &'a [u8],
}
