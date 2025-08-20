// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

enum TestEnum {
    TestValue1,
    TestValue2,
}

#[event("enum_event")]
struct EnumEvent {
    #[nominal_counter("value")]
    value: TestEnum,
}
