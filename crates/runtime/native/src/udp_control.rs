//! Portable controls and endpoint facts for native UDP capability handles.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::udp::sockets;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_endpoint_part(
    handle: u64,
    field: u8,
    index: u8,
    output: *mut u32,
) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    let Ok(address) = socket.local_addr() else {
        return 0;
    };
    let value = match field {
        0 if index == 0 => u32::from(if address.is_ipv4() { 4_u8 } else { 6_u8 }),
        1 => match address.ip() {
            IpAddr::V4(value) if index == 0 => u32::from(value),
            IpAddr::V6(value) if index < 4 => {
                let octets = value.octets();
                let start = usize::from(index) * 4;
                u32::from_be_bytes([
                    octets[start],
                    octets[start + 1],
                    octets[start + 2],
                    octets[start + 3],
                ])
            }
            _ => return 0,
        },
        2 if index == 0 => match address {
            SocketAddr::V4(_) => 0,
            SocketAddr::V6(value) => value.scope_id(),
        },
        _ => return 0,
    };
    // SAFETY: the caller supplied a non-null scalar output slot.
    unsafe { output.write(value) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_set_broadcast(handle: u64, enabled: u8) -> u8 {
    let enabled = match enabled {
        0 => false,
        1 => true,
        _ => return 0,
    };
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    u8::from(socket.set_broadcast(enabled).is_ok())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_broadcast(handle: u64, output: *mut u8) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    let Ok(enabled) = socket.broadcast() else {
        return 0;
    };
    unsafe { output.write(u8::from(enabled)) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_set_ttl(handle: u64, ttl: u32) -> u8 {
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    u8::from(socket.set_ttl(ttl).is_ok())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_ttl(handle: u64, output: *mut u32) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    let Ok(ttl) = socket.ttl() else { return 0 };
    unsafe { output.write(ttl) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_join_multicast_ipv4(handle: u64, group: u32, interface: u32) -> u8 {
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    u8::from(
        socket
            .join_multicast_v4(&Ipv4Addr::from(group), &Ipv4Addr::from(interface))
            .is_ok(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_leave_multicast_ipv4(handle: u64, group: u32, interface: u32) -> u8 {
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    u8::from(
        socket
            .leave_multicast_v4(&Ipv4Addr::from(group), &Ipv4Addr::from(interface))
            .is_ok(),
    )
}
