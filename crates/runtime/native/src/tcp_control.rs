//! Portable controls for native TCP stream capability handles.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::net::Shutdown;

use crate::tcp::{TcpResource, resources};

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_shutdown(handle: u64, direction: u8) -> u8 {
    let direction = match direction {
        0 => Shutdown::Read,
        1 => Shutdown::Write,
        2 => Shutdown::Both,
        _ => return 0,
    };
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    u8::from(stream.shutdown(direction).is_ok())
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_set_no_delay(handle: u64, enabled: u8) -> u8 {
    let enabled = match enabled {
        0 => false,
        1 => true,
        _ => return 0,
    };
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    u8::from(stream.set_nodelay(enabled).is_ok())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_no_delay(handle: u64, output: *mut u8) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    let Ok(enabled) = stream.nodelay() else {
        return 0;
    };
    // SAFETY: the caller supplied a non-null scalar output slot.
    unsafe { output.write(u8::from(enabled)) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_set_ttl(handle: u64, ttl: u32) -> u8 {
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    u8::from(stream.set_ttl(ttl).is_ok())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_ttl(handle: u64, output: *mut u32) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    let Ok(ttl) = stream.ttl() else {
        return 0;
    };
    // SAFETY: the caller supplied a non-null scalar output slot.
    unsafe { output.write(ttl) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_endpoint_part(
    handle: u64,
    peer: u8,
    field: u8,
    index: u8,
    output: *mut u32,
) -> u8 {
    if output.is_null() || peer > 1 {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    let address = if peer == 0 {
        stream.local_addr()
    } else {
        stream.peer_addr()
    };
    let Ok(address) = address else { return 0 };
    let value = match field {
        0 if index == 0 => u32::from(if address.is_ipv4() { 4_u8 } else { 6_u8 }),
        1 => match address.ip() {
            std::net::IpAddr::V4(value) if index == 0 => u32::from(value),
            std::net::IpAddr::V6(value) if index < 4 => {
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
        2 if index == 0 => u32::from(address.port()),
        3 if index == 0 => match address {
            std::net::SocketAddr::V4(_) => 0,
            std::net::SocketAddr::V6(value) => value.scope_id(),
        },
        _ => return 0,
    };
    // SAFETY: the caller supplied a non-null scalar output slot.
    unsafe { output.write(value) };
    1
}
