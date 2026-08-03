//! Explicit numeric-IPv4 UDP capability handles for the native runtime.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use pop_runtime_interface::RuntimeAdapter;
use pop_runtime_native_abi::SocketIoStatus;

use crate::allocate_immutable_bytes;
use crate::byte_buffer::append_bytes;
use crate::state::lock_abi_runtime;

static NEXT_UDP: AtomicU64 = AtomicU64::new(1);

pub(crate) fn sockets() -> &'static Mutex<BTreeMap<u64, UdpSocket>> {
    static SOCKETS: OnceLock<Mutex<BTreeMap<u64, UdpSocket>>> = OnceLock::new();
    SOCKETS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_bind(port: u16) -> u64 {
    pop_rt_udp_bind_ipv4(u32::from(Ipv4Addr::LOCALHOST), port)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_bind_ipv4(address: u32, port: u16) -> u64 {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::from(address), port)) else {
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

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_bind_ipv6(
    first: u32,
    second: u32,
    third: u32,
    fourth: u32,
    port: u16,
    scope: u32,
) -> u64 {
    let mut octets = [0_u8; 16];
    for (index, word) in [first, second, third, fourth].into_iter().enumerate() {
        octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    let address = SocketAddrV6::new(Ipv6Addr::from(octets), port, 0, scope);
    let Ok(socket) = UdpSocket::bind(address) else {
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
    written: *mut u64,
) -> u8 {
    if bytes.is_null() || written.is_null() {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(length) = usize::try_from(length) else {
        return SocketIoStatus::Failure as u8;
    };
    // SAFETY: the caller owns a readable byte span for this call.
    let input = unsafe { std::slice::from_raw_parts(bytes, length) };
    let Ok(values) = sockets().lock() else {
        return SocketIoStatus::Failure as u8;
    };
    let Some(socket) = values.get(&handle) else {
        return SocketIoStatus::Failure as u8;
    };
    let destination = SocketAddrV4::new(Ipv4Addr::from(address), port);
    match socket.send_to(input, destination) {
        Ok(count) => {
            let Ok(count) = u64::try_from(count) else {
                return SocketIoStatus::Failure as u8;
            };
            // SAFETY: the caller supplied a non-null scalar output slot.
            unsafe { written.write(count) };
            SocketIoStatus::Progress as u8
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            SocketIoStatus::WouldBlock as u8
        }
        Err(_) => SocketIoStatus::Failure as u8,
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_receive(
    handle: u64,
    bytes: *mut u8,
    capacity: u64,
    address: *mut u32,
    port: *mut u16,
    received: *mut u64,
) -> u8 {
    if bytes.is_null() || address.is_null() || port.is_null() || received.is_null() || capacity == 0
    {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    // SAFETY: the caller owns a writable byte span for this call.
    let output = unsafe { std::slice::from_raw_parts_mut(bytes, capacity) };
    let Ok(values) = sockets().lock() else {
        return SocketIoStatus::Failure as u8;
    };
    let Some(socket) = values.get(&handle) else {
        return SocketIoStatus::Failure as u8;
    };
    let (count, peer) = match socket.recv_from(output) {
        Ok(received) => received,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            return SocketIoStatus::WouldBlock as u8;
        }
        Err(_) => return SocketIoStatus::Failure as u8,
    };
    let std::net::SocketAddr::V4(peer) = peer else {
        return SocketIoStatus::Failure as u8;
    };
    let Ok(count) = u64::try_from(count) else {
        return SocketIoStatus::Failure as u8;
    };
    // SAFETY: callers provide four writable scalar outputs.
    unsafe {
        address.write(u32::from(*peer.ip()));
        port.write(peer.port());
        received.write(count);
    }
    SocketIoStatus::Progress as u8
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_send_bytes_to(
    handle: u64,
    address: u32,
    port: u16,
    bytes: u64,
    written: *mut u64,
) -> u8 {
    if bytes == 0 || written.is_null() {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(runtime) = lock_abi_runtime() else {
        return SocketIoStatus::Failure as u8;
    };
    let reference = pop_runtime_interface::ManagedReference::new(bytes);
    let Ok(length) = runtime.immutable_bytes_length(reference) else {
        return SocketIoStatus::Failure as u8;
    };
    let Ok(capacity) = usize::try_from(length) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut input = vec![0; capacity];
    if runtime
        .immutable_bytes_read(reference, 0, &mut input)
        .is_err()
    {
        return SocketIoStatus::Failure as u8;
    }
    drop(runtime);
    unsafe { pop_rt_udp_send_to(handle, address, port, input.as_ptr(), length, written) }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_receive_bytes(
    handle: u64,
    capacity: u64,
    bytes: *mut u64,
    address: *mut u32,
    port: *mut u16,
    received: *mut u64,
) -> u8 {
    if bytes.is_null() || address.is_null() || port.is_null() || received.is_null() || capacity == 0
    {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(length) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut output = vec![0; length];
    let status = unsafe {
        pop_rt_udp_receive(
            handle,
            output.as_mut_ptr(),
            capacity,
            address,
            port,
            received,
        )
    };
    if status != SocketIoStatus::Progress as u8 {
        return status;
    }
    let count = unsafe { received.read() };
    let Ok(count) = usize::try_from(count) else {
        return SocketIoStatus::Failure as u8;
    };
    let reference = allocate_immutable_bytes(&output[..count]);
    if reference == 0 {
        return SocketIoStatus::Failure as u8;
    }
    unsafe { bytes.write(reference) };
    status
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_receive_buffer(
    handle: u64,
    buffer: u64,
    capacity: u64,
    address: *mut u32,
    port: *mut u16,
    received: *mut u64,
) -> u8 {
    if buffer == 0
        || address.is_null()
        || port.is_null()
        || received.is_null()
        || capacity == 0
        || capacity > u64::from(u16::MAX)
    {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(length) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut output = vec![0; length];
    let status = unsafe {
        pop_rt_udp_receive(
            handle,
            output.as_mut_ptr(),
            capacity,
            address,
            port,
            received,
        )
    };
    if status != SocketIoStatus::Progress as u8 {
        return status;
    }
    let count = unsafe { received.read() };
    let Ok(count) = usize::try_from(count) else {
        return SocketIoStatus::Failure as u8;
    };
    if append_bytes(buffer, &output[..count]) == 0 {
        return SocketIoStatus::Failure as u8;
    }
    status
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_udp_close(handle: u64) -> u8 {
    let Ok(mut values) = sockets().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle).is_some())
}
