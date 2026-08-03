//! Explicit loopback TCP capability handles for the native runtime.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use pop_runtime_interface::RuntimeAdapter;
use pop_runtime_native_abi::SocketIoStatus;

use crate::allocate_immutable_bytes;
use crate::byte_buffer::append_bytes;
use crate::state::lock_abi_runtime;

enum TcpResource {
    Listener(TcpListener),
    Stream(TcpStream),
}

static NEXT_TCP: AtomicU64 = AtomicU64::new(1);

fn resources() -> &'static Mutex<BTreeMap<u64, TcpResource>> {
    static RESOURCES: OnceLock<Mutex<BTreeMap<u64, TcpResource>>> = OnceLock::new();
    RESOURCES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn insert(resource: TcpResource) -> u64 {
    let Ok(mut values) = resources().lock() else {
        return 0;
    };
    let handle = NEXT_TCP.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    values.insert(handle, resource);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_listen(port: u16) -> u64 {
    pop_rt_tcp_listen_ipv4(u32::from(Ipv4Addr::LOCALHOST), port)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_listen_ipv4(address: u32, port: u16) -> u64 {
    let Ok(listener) = TcpListener::bind((Ipv4Addr::from(address), port)) else {
        return 0;
    };
    let _ = listener.set_nonblocking(true);
    insert(TcpResource::Listener(listener))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_local_port(handle: u64, output: *mut u16) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(resource) = values.get(&handle) else {
        return 0;
    };
    let address = match resource {
        TcpResource::Listener(listener) => listener.local_addr().ok(),
        TcpResource::Stream(stream) => stream.local_addr().ok(),
    };
    let Some(address) = address else { return 0 };
    let std::net::SocketAddr::V4(address) = address else {
        return 0;
    };
    // SAFETY: the caller supplied a non-null scalar output slot.
    unsafe { output.write(address.port()) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_connect(port: u16) -> u64 {
    pop_rt_tcp_connect_ipv4(u32::from(Ipv4Addr::LOCALHOST), port)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_connect_ipv4(address: u32, port: u16) -> u64 {
    let Ok(stream) = TcpStream::connect((Ipv4Addr::from(address), port)) else {
        return 0;
    };
    if stream.set_nonblocking(true).is_err() {
        return 0;
    }
    insert(TcpResource::Stream(stream))
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_accept(listener: u64) -> u64 {
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Listener(listener)) = values.get(&listener) else {
        return 0;
    };
    let Ok((stream, _)) = listener.accept() else {
        return 0;
    };
    if stream.set_nonblocking(true).is_err() {
        return 0;
    }
    drop(values);
    insert(TcpResource::Stream(stream))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_send(
    handle: u64,
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
    let Ok(mut values) = resources().lock() else {
        return SocketIoStatus::Failure as u8;
    };
    let Some(TcpResource::Stream(stream)) = values.get_mut(&handle) else {
        return SocketIoStatus::Failure as u8;
    };
    match stream.write(input) {
        Ok(0) if !input.is_empty() => SocketIoStatus::Closed as u8,
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
        Err(error) if is_closed(&error) => SocketIoStatus::Closed as u8,
        Err(_) => SocketIoStatus::Failure as u8,
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_receive(
    handle: u64,
    bytes: *mut u8,
    capacity: u64,
    received: *mut u64,
) -> u8 {
    if bytes.is_null() || received.is_null() || capacity == 0 {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    // SAFETY: the caller owns a writable byte span for this call.
    let output = unsafe { std::slice::from_raw_parts_mut(bytes, capacity) };
    let Ok(mut values) = resources().lock() else {
        return SocketIoStatus::Failure as u8;
    };
    let Some(TcpResource::Stream(stream)) = values.get_mut(&handle) else {
        return SocketIoStatus::Failure as u8;
    };
    match stream.read(output) {
        Ok(0) => SocketIoStatus::Closed as u8,
        Ok(count) => {
            let Ok(count) = u64::try_from(count) else {
                return SocketIoStatus::Failure as u8;
            };
            // SAFETY: the caller supplied a non-null scalar output slot.
            unsafe { received.write(count) };
            SocketIoStatus::Progress as u8
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            SocketIoStatus::WouldBlock as u8
        }
        Err(error) if is_closed(&error) => SocketIoStatus::Closed as u8,
        Err(_) => SocketIoStatus::Failure as u8,
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_send_bytes(handle: u64, bytes: u64, written: *mut u64) -> u8 {
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
    // SAFETY: the temporary input and caller output remain valid for this call.
    unsafe { pop_rt_tcp_send(handle, input.as_ptr(), length, written) }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_receive_bytes(
    handle: u64,
    capacity: u64,
    bytes: *mut u64,
    received: *mut u64,
) -> u8 {
    if bytes.is_null() || received.is_null() || capacity == 0 {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(length) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut output = vec![0; length];
    let status = unsafe { pop_rt_tcp_receive(handle, output.as_mut_ptr(), capacity, received) };
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
pub unsafe extern "C" fn pop_rt_tcp_receive_buffer(
    handle: u64,
    buffer: u64,
    capacity: u64,
    received: *mut u64,
) -> u8 {
    if buffer == 0 || received.is_null() || capacity == 0 {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(length) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut output = vec![0; length];
    // SAFETY: the temporary output and caller count slot remain valid for this call.
    let status = unsafe { pop_rt_tcp_receive(handle, output.as_mut_ptr(), capacity, received) };
    if status != SocketIoStatus::Progress as u8 {
        return status;
    }
    // SAFETY: the successful receive initialized the non-null count slot.
    let count = unsafe { received.read() };
    let Ok(count) = usize::try_from(count) else {
        return SocketIoStatus::Failure as u8;
    };
    if append_bytes(buffer, &output[..count]) == 0 {
        return SocketIoStatus::Failure as u8;
    }
    status
}

fn is_closed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::NotConnected
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_close(handle: u64) -> u8 {
    let Ok(mut values) = resources().lock() else {
        return 0;
    };
    let Some(resource) = values.remove(&handle) else {
        return 0;
    };
    if let TcpResource::Stream(stream) = resource {
        let _ = stream.shutdown(Shutdown::Both);
    }
    1
}
