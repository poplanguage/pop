//! Explicit loopback TCP capability handles for the native runtime.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

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
    let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
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
    let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) else {
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
pub unsafe extern "C" fn pop_rt_tcp_send(handle: u64, bytes: *const u8, length: u64) -> u64 {
    if bytes.is_null() {
        return 0;
    }
    let Ok(length) = usize::try_from(length) else {
        return 0;
    };
    // SAFETY: the caller owns a readable byte span for this call.
    let input = unsafe { std::slice::from_raw_parts(bytes, length) };
    let Ok(mut values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get_mut(&handle) else {
        return 0;
    };
    stream
        .write(input)
        .ok()
        .and_then(|count| u64::try_from(count).ok())
        .unwrap_or(0)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_receive(handle: u64, bytes: *mut u8, capacity: u64) -> u64 {
    if bytes.is_null() {
        return 0;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return 0;
    };
    // SAFETY: the caller owns a writable byte span for this call.
    let output = unsafe { std::slice::from_raw_parts_mut(bytes, capacity) };
    let Ok(mut values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get_mut(&handle) else {
        return 0;
    };
    stream
        .read(output)
        .ok()
        .and_then(|count| u64::try_from(count).ok())
        .unwrap_or(0)
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
