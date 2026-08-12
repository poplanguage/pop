//! Unix-domain stream capability handles for the native runtime.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use pop_runtime_interface::RuntimeAdapter;
use pop_runtime_native_abi::SocketIoStatus;

use crate::byte_buffer::append_bytes;
use crate::pop_rt_string_read;
use crate::state::lock_abi_runtime;

enum UnixResource {
    Listener(UnixListener),
    Stream(UnixStream),
}

static NEXT_UNIX_SOCKET: AtomicU64 = AtomicU64::new(1);

fn resources() -> &'static Mutex<BTreeMap<u64, UnixResource>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, UnixResource>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn insert(resource: UnixResource) -> u64 {
    let Ok(mut values) = resources().lock() else {
        return 0;
    };
    let handle = NEXT_UNIX_SOCKET.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    values.insert(handle, resource);
    handle
}

unsafe fn read_path(reference: u64) -> Option<PathBuf> {
    let encoded = unsafe { pop_rt_string_read(reference, std::ptr::null_mut(), 0) };
    let length = usize::try_from(encoded.checked_sub(1)?).ok()?;
    let mut bytes = vec![0; length];
    if unsafe { pop_rt_string_read(reference, bytes.as_mut_ptr(), length as u64) } == 0
        || bytes.contains(&0)
    {
        return None;
    }
    Some(PathBuf::from(String::from_utf8(bytes).ok()?))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_unix_listen(path: u64) -> u64 {
    let Some(path) = (unsafe { read_path(path) }) else {
        return 0;
    };
    let Ok(listener) = UnixListener::bind(path) else {
        return 0;
    };
    if listener.set_nonblocking(true).is_err() {
        return 0;
    }
    insert(UnixResource::Listener(listener))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_unix_connect(path: u64) -> u64 {
    let Some(path) = (unsafe { read_path(path) }) else {
        return 0;
    };
    let Ok(stream) = UnixStream::connect(path) else {
        return 0;
    };
    if stream.set_nonblocking(true).is_err() {
        return 0;
    }
    insert(UnixResource::Stream(stream))
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_unix_accept(listener: u64) -> u64 {
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(UnixResource::Listener(listener)) = values.get(&listener) else {
        return 0;
    };
    let Ok((stream, _)) = listener.accept() else {
        return 0;
    };
    if stream.set_nonblocking(true).is_err() {
        return 0;
    }
    drop(values);
    insert(UnixResource::Stream(stream))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_unix_send_bytes(handle: u64, bytes: u64, written: *mut u64) -> u8 {
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
    let Ok(mut values) = resources().lock() else {
        return SocketIoStatus::Failure as u8;
    };
    let Some(UnixResource::Stream(stream)) = values.get_mut(&handle) else {
        return SocketIoStatus::Failure as u8;
    };
    match stream.write(&input) {
        Ok(0) if !input.is_empty() => SocketIoStatus::Closed as u8,
        Ok(count) => {
            let Ok(count) = u64::try_from(count) else {
                return SocketIoStatus::Failure as u8;
            };
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_unix_receive_buffer(
    handle: u64,
    buffer: u64,
    capacity: u64,
    received: *mut u64,
) -> u8 {
    if buffer == 0 || capacity == 0 || received.is_null() {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut output = vec![0; capacity];
    let Ok(mut values) = resources().lock() else {
        return SocketIoStatus::Failure as u8;
    };
    let Some(UnixResource::Stream(stream)) = values.get_mut(&handle) else {
        return SocketIoStatus::Failure as u8;
    };
    let status = match stream.read(&mut output) {
        Ok(0) => return SocketIoStatus::Closed as u8,
        Ok(count) => count,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            return SocketIoStatus::WouldBlock as u8;
        }
        Err(error) if is_closed(&error) => return SocketIoStatus::Closed as u8,
        Err(_) => return SocketIoStatus::Failure as u8,
    };
    drop(values);
    if append_bytes(buffer, &output[..status]) == 0 {
        return SocketIoStatus::Failure as u8;
    }
    let Ok(count) = u64::try_from(status) else {
        return SocketIoStatus::Failure as u8;
    };
    unsafe { received.write(count) };
    SocketIoStatus::Progress as u8
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_unix_shutdown(handle: u64, direction: u8) -> u8 {
    let direction = match direction {
        0 => Shutdown::Read,
        1 => Shutdown::Write,
        2 => Shutdown::Both,
        _ => return 0,
    };
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(UnixResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    u8::from(stream.shutdown(direction).is_ok())
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_unix_close(handle: u64) -> u8 {
    let Ok(mut values) = resources().lock() else {
        return 0;
    };
    let Some(resource) = values.remove(&handle) else {
        return 0;
    };
    if let UnixResource::Stream(stream) = resource {
        let _ = stream.shutdown(Shutdown::Both);
    }
    1
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
