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
