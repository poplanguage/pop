//! Rustls-backed typed TLS configuration and stream capabilities.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread;
use std::time::Duration;

use pop_runtime_interface::{ManagedReference, RuntimeAdapter};
use pop_runtime_native_abi::SocketIoStatus;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection};
use rustls_platform_verifier::ConfigVerifierExt;

use crate::state::lock_abi_runtime;

enum TlsConfig {
    Client(Arc<ClientConfig>),
    Server(Arc<ServerConfig>),
}

enum TlsStream {
    Client(rustls::StreamOwned<ClientConnection, std::net::TcpStream>),
    Server(rustls::StreamOwned<ServerConnection, std::net::TcpStream>),
}

static NEXT_TLS: AtomicU64 = AtomicU64::new(1);

fn configs() -> &'static Mutex<BTreeMap<u64, TlsConfig>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, TlsConfig>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn streams() -> &'static Mutex<BTreeMap<u64, TlsStream>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, TlsStream>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_handle() -> Option<u64> {
    let handle = NEXT_TLS.fetch_add(1, Ordering::Relaxed);
    (handle != 0).then_some(handle)
}

fn install_crypto_provider() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn immutable_bytes(reference: u64) -> Option<Vec<u8>> {
    let runtime = lock_abi_runtime().ok()?;
    let reference = ManagedReference::new(reference);
    let length = runtime.immutable_bytes_length(reference).ok()?;
    let mut bytes = vec![0; usize::try_from(length).ok()?];
    runtime
        .immutable_bytes_read(reference, 0, &mut bytes)
        .ok()?;
    Some(bytes)
}

fn managed_string(reference: u64) -> Option<String> {
    let encoded_length = unsafe { crate::pop_rt_string_read(reference, std::ptr::null_mut(), 0) };
    let length = encoded_length.checked_sub(1)?;
    let mut bytes = vec![0; usize::try_from(length).ok()?];
    if length != 0
        && unsafe { crate::pop_rt_string_read(reference, bytes.as_mut_ptr(), length) } == 0
    {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn wait_handshake(
    deadline: u64,
    cancel: u64,
    mut complete: impl FnMut() -> std::io::Result<bool>,
) -> bool {
    loop {
        if crate::pop_rt_task_cancellation_requested(cancel) == 1 {
            return false;
        }
        match complete() {
            Ok(false) => return true,
            Ok(true) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return false,
        }
        let Some(remaining) = crate::monotonic_time::deadline_remaining(deadline) else {
            return false;
        };
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(Duration::from_millis(1)));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_client_system_config() -> u64 {
    install_crypto_provider();
    let Ok(config) = ClientConfig::with_platform_verifier() else {
        return 0;
    };
    let Some(handle) = next_handle() else {
        return 0;
    };
    let Ok(mut values) = configs().lock() else {
        return 0;
    };
    values.insert(handle, TlsConfig::Client(Arc::new(config)));
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_client_root_config(certificate: u64) -> u64 {
    install_crypto_provider();
    let Some(certificate) = immutable_bytes(certificate) else {
        return 0;
    };
    let mut roots = RootCertStore::empty();
    if roots.add(CertificateDer::from(certificate)).is_err() {
        return 0;
    }
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let Some(handle) = next_handle() else {
        return 0;
    };
    let Ok(mut values) = configs().lock() else {
        return 0;
    };
    values.insert(handle, TlsConfig::Client(Arc::new(config)));
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_server_config(certificate: u64, private_key: u64) -> u64 {
    install_crypto_provider();
    let (Some(certificate), Some(private_key)) =
        (immutable_bytes(certificate), immutable_bytes(private_key))
    else {
        return 0;
    };
    let Ok(config) = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(certificate)],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(private_key)),
        )
    else {
        return 0;
    };
    let Some(handle) = next_handle() else {
        return 0;
    };
    let Ok(mut values) = configs().lock() else {
        return 0;
    };
    values.insert(handle, TlsConfig::Server(Arc::new(config)));
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_config_close(handle: u64) -> u8 {
    configs()
        .lock()
        .ok()
        .map_or(0, |mut values| u8::from(values.remove(&handle).is_some()))
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_client_handshake(
    config: u64,
    stream: u64,
    server_name: u64,
    deadline: u64,
    cancel: u64,
) -> u64 {
    let config = configs()
        .lock()
        .ok()
        .and_then(|values| match values.get(&config) {
            Some(TlsConfig::Client(config)) => Some(Arc::clone(config)),
            _ => None,
        });
    let (Some(config), Some(server_name), Some(stream)) = (
        config,
        managed_string(server_name).and_then(|name| ServerName::try_from(name).ok()),
        crate::tcp::take_stream(stream),
    ) else {
        return 0;
    };
    let Ok(connection) = ClientConnection::new(config, server_name) else {
        return 0;
    };
    let mut stream = rustls::StreamOwned::new(connection, stream);
    if !wait_handshake(deadline, cancel, || {
        stream
            .conn
            .complete_io(&mut stream.sock)
            .map(|_| stream.conn.is_handshaking())
    }) {
        return 0;
    }
    let Some(handle) = next_handle() else {
        return 0;
    };
    streams().lock().ok().map_or(0, |mut values| {
        values.insert(handle, TlsStream::Client(stream));
        handle
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_server_handshake(
    config: u64,
    stream: u64,
    deadline: u64,
    cancel: u64,
) -> u64 {
    let config = configs()
        .lock()
        .ok()
        .and_then(|values| match values.get(&config) {
            Some(TlsConfig::Server(config)) => Some(Arc::clone(config)),
            _ => None,
        });
    let (Some(config), Some(stream)) = (config, crate::tcp::take_stream(stream)) else {
        return 0;
    };
    let Ok(connection) = ServerConnection::new(config) else {
        return 0;
    };
    let mut stream = rustls::StreamOwned::new(connection, stream);
    if !wait_handshake(deadline, cancel, || {
        stream
            .conn
            .complete_io(&mut stream.sock)
            .map(|_| stream.conn.is_handshaking())
    }) {
        return 0;
    }
    let Some(handle) = next_handle() else {
        return 0;
    };
    streams().lock().ok().map_or(0, |mut values| {
        values.insert(handle, TlsStream::Server(stream));
        handle
    })
}

fn with_stream<T>(handle: u64, apply: impl FnOnce(&mut dyn ReadWrite) -> T) -> Option<T> {
    let mut values = streams().lock().ok()?;
    match values.get_mut(&handle)? {
        TlsStream::Client(stream) => Some(apply(stream)),
        TlsStream::Server(stream) => Some(apply(stream)),
    }
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tls_send(
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
    let input = unsafe { std::slice::from_raw_parts(bytes, length) };
    with_stream(handle, |stream| match stream.write(input) {
        Ok(0) if !input.is_empty() => SocketIoStatus::Closed as u8,
        Ok(count) => {
            unsafe { written.write(u64::try_from(count).unwrap_or(0)) };
            SocketIoStatus::Progress as u8
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            SocketIoStatus::WouldBlock as u8
        }
        Err(_) => SocketIoStatus::Failure as u8,
    })
    .unwrap_or(SocketIoStatus::Failure as u8)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tls_receive(
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
    let output = unsafe { std::slice::from_raw_parts_mut(bytes, capacity) };
    with_stream(handle, |stream| match stream.read(output) {
        Ok(0) => SocketIoStatus::Closed as u8,
        Ok(count) => {
            unsafe { received.write(u64::try_from(count).unwrap_or(0)) };
            SocketIoStatus::Progress as u8
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            SocketIoStatus::WouldBlock as u8
        }
        Err(_) => SocketIoStatus::Failure as u8,
    })
    .unwrap_or(SocketIoStatus::Failure as u8)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tls_send_bytes(handle: u64, bytes: u64, written: *mut u64) -> u8 {
    let Some(input) = immutable_bytes(bytes) else {
        return SocketIoStatus::Failure as u8;
    };
    unsafe {
        pop_rt_tls_send(
            handle,
            input.as_ptr(),
            u64::try_from(input.len()).unwrap_or(0),
            written,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tls_receive_buffer(
    handle: u64,
    buffer: u64,
    capacity: u64,
    received: *mut u64,
) -> u8 {
    let Ok(length) = usize::try_from(capacity) else {
        return SocketIoStatus::Failure as u8;
    };
    let mut output = vec![0; length];
    let status = unsafe { pop_rt_tls_receive(handle, output.as_mut_ptr(), capacity, received) };
    if status != SocketIoStatus::Progress as u8 {
        return status;
    }
    let count = unsafe { received.read() };
    let Ok(count) = usize::try_from(count) else {
        return SocketIoStatus::Failure as u8;
    };
    if crate::byte_buffer::append_bytes(buffer, &output[..count]) == 1 {
        status
    } else {
        SocketIoStatus::Failure as u8
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tls_close(handle: u64) -> u8 {
    streams()
        .lock()
        .ok()
        .map_or(0, |mut values| u8::from(values.remove(&handle).is_some()))
}
