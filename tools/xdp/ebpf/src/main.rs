// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![no_std]
#![no_main]

use aya_bpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::HashMap,
    programs::XdpContext,
};
use s2n_quic_core::xdp::{
    bpf::DecoderBufferMut,
    decoder::{decode_packet_validator, Validator},
};

mod xskmap;

#[map(name = "S2N_QUIC_XDP_SOCKETS")]
static SOCKETS: xskmap::XskMap = xskmap::XskMap::with_max_entries(1024, 0);

#[map(name = "S2N_QUIC_XDP_PORTS")]
static PORTS: HashMap<u16, u8> = HashMap::with_max_entries(1024, 0);

#[xdp(name = "s2n_quic_xdp")]
pub fn s2n_quic_xdp(ctx: XdpContext) -> u32 {
    let action = handle_packet(&ctx);

    #[cfg(feature = "trace")]
    {
        use aya_log_ebpf as log;
        match action {
            xdp_action::XDP_DROP => log::trace!(&ctx, "ACTION: DROP"),
            xdp_action::XDP_PASS => log::trace!(&ctx, "ACTION: PASS"),
            xdp_action::XDP_REDIRECT => log::trace!(&ctx, "ACTION: REDIRECT"),
            xdp_action::XDP_ABORTED => log::trace!(&ctx, "ACTION: ABORTED"),
            _ => (),
        }
    }

    action
}

#[inline(always)]
fn handle_packet(ctx: &XdpContext) -> u32 {
    let start = ctx.data() as *mut u8;
    let end = ctx.data_end() as *mut u8;
    let buffer = unsafe {
        // Safety: start and end come from the caller and have been validated
        DecoderBufferMut::new(start, end)
    };
    match decode_packet_validator(buffer, &PortValidator) {
        Ok(Some((_tuple, payload))) => {
            // if the payload is empty there isn't much we can do with it
            if payload.is_empty() {
                return xdp_action::XDP_DROP;
            }

            // if the packet is valid forward it on to the associated AF_XDP socket
            let queue_id = unsafe { (*ctx.ctx).rx_queue_index };
            let not_found_action = xdp_action::XDP_PASS as _;
            SOCKETS.redirect(queue_id, not_found_action)
        }
        Ok(None) => xdp_action::XDP_PASS,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

struct PortValidator;

impl Validator for PortValidator {
    #[inline(always)]
    fn validate_local_port(&self, port: u16) -> bool {
        // Make sure the port is in the port map. Otherwise, forward the packet to the OS.
        PORTS.get_ptr(&port).is_some()
    }
}

/// Define a no-op panic handler
///
/// The implementation shouldn't panic at all. But we do need to define one in
/// `#[no_std]` builds.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}