// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#[event("byte_array_event")]
struct ByteArrayEvent<'a> {
    data: &'a [u8],
}
