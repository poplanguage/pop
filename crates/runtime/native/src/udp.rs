//! Explicit numeric-IPv4 UDP capability handles for the native runtime.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

static NEXT_UDP: AtomicU64 = AtomicU64::new(1);

fn sockets() -> &'static Mutex<BTreeMap<u64, UdpSocket>> {
    static SOCKETS: OnceLock<Mutex<BTreeMap<u64, UdpSocket>>> = OnceLock::new();
    SOCKETS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_bind(port: u16) -> u64 {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::LOCALHOST, port)) else {
        return 0;
    };
    let _ = socket.set_nonblocking(true);
    let Ok(mut values) = sockets().lock() else {
        return 0;
    };
    let handle = NEXT_UDP.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    values.insert(handle, socket);
    handle
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_local_port(handle: u64, output: *mut u16) -> u8 {
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
    // SAFETY: the caller supplied a non-null scalar output slot.
    unsafe { output.write(address.port()) };
    1
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_send_to(
    handle: u64,
    address: u32,
    port: u16,
    bytes: *const u8,
    length: u64,
) -> u64 {
    if bytes.is_null() {
        return 0;
    }
    let Ok(length) = usize::try_from(length) else {
        return 0;
    };
    // SAFETY: the caller owns a readable byte span for this call.
    let input = unsafe { std::slice::from_raw_parts(bytes, length) };
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    let destination = SocketAddrV4::new(Ipv4Addr::from(address), port);
    socket
        .send_to(input, destination)
        .ok()
        .and_then(|count| u64::try_from(count).ok())
        .unwrap_or(0)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_receive(
    handle: u64,
    bytes: *mut u8,
    capacity: u64,
    address: *mut u32,
    port: *mut u16,
) -> u64 {
    if bytes.is_null() || address.is_null() || port.is_null() {
        return 0;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return 0;
    };
    // SAFETY: the caller owns a writable byte span for this call.
    let output = unsafe { std::slice::from_raw_parts_mut(bytes, capacity) };
    let Ok(values) = sockets().lock() else {
        return 0;
    };
    let Some(socket) = values.get(&handle) else {
        return 0;
    };
    let Ok((count, peer)) = socket.recv_from(output) else {
        return 0;
    };
    let std::net::SocketAddr::V4(peer) = peer else {
        return 0;
    };
    // SAFETY: callers provide three writable scalar outputs.
    unsafe {
        address.write(u32::from(*peer.ip()));
        port.write(peer.port());
    }
    u64::try_from(count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_close(handle: u64) -> u8 {
    let Ok(mut values) = sockets().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle).is_some())
}
