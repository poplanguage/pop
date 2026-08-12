//! Keepalive and close-linger controls for native TCP stream capabilities.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

#[cfg(unix)]
use std::os::fd::AsRawFd;

use crate::tcp::{TcpResource, resources};

#[cfg(unix)]
fn set_socket_i32(stream: &std::net::TcpStream, level: i32, option: i32, value: i32) -> bool {
    let length = libc::socklen_t::try_from(std::mem::size_of_val(&value)).unwrap_or(0);
    unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            level,
            option,
            (&raw const value).cast(),
            length,
        ) == 0
    }
}

#[cfg(unix)]
fn socket_i32(stream: &std::net::TcpStream, level: i32, option: i32) -> Option<i32> {
    let mut value = 0_i32;
    let mut length = libc::socklen_t::try_from(std::mem::size_of_val(&value)).ok()?;
    let accepted = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            level,
            option,
            (&raw mut value).cast(),
            &raw mut length,
        ) == 0
    };
    accepted.then_some(value)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_set_keepalive(handle: u64, enabled: u8) -> u8 {
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
    #[cfg(unix)]
    return u8::from(set_socket_i32(
        stream,
        libc::SOL_SOCKET,
        libc::SO_KEEPALIVE,
        i32::from(enabled),
    ));
    #[cfg(not(unix))]
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_keepalive(handle: u64, output: *mut u8) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    #[cfg(unix)]
    let Some(enabled) = socket_i32(stream, libc::SOL_SOCKET, libc::SO_KEEPALIVE) else {
        return 0;
    };
    #[cfg(not(unix))]
    let enabled = 0;
    unsafe { output.write(u8::from(enabled != 0)) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_set_keepalive_idle(handle: u64, milliseconds: u64) -> u8 {
    if milliseconds == 0 {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    let seconds = milliseconds.saturating_add(999) / 1_000;
    let Ok(seconds) = i32::try_from(seconds) else {
        return 0;
    };
    #[cfg(any(target_os = "linux", target_os = "android"))]
    return u8::from(set_socket_i32(
        stream,
        libc::IPPROTO_TCP,
        libc::TCP_KEEPIDLE,
        seconds,
    ));
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    return u8::from(set_socket_i32(
        stream,
        libc::IPPROTO_TCP,
        libc::TCP_KEEPALIVE,
        seconds,
    ));
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    )))]
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_set_linger(handle: u64, milliseconds: u64) -> u8 {
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    #[cfg(unix)]
    {
        let seconds = milliseconds.saturating_add(999) / 1_000;
        let Ok(seconds) = i32::try_from(seconds) else {
            return 0;
        };
        let linger = libc::linger {
            l_onoff: i32::from(milliseconds != 0),
            l_linger: seconds,
        };
        let length = libc::socklen_t::try_from(std::mem::size_of_val(&linger)).unwrap_or(0);
        u8::from(unsafe {
            libc::setsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                (&raw const linger).cast(),
                length,
            ) == 0
        })
    }
    #[cfg(not(unix))]
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_linger(handle: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = resources().lock() else {
        return 0;
    };
    let Some(TcpResource::Stream(stream)) = values.get(&handle) else {
        return 0;
    };
    #[cfg(unix)]
    let milliseconds = {
        let mut linger = libc::linger {
            l_onoff: 0,
            l_linger: 0,
        };
        let mut length = libc::socklen_t::try_from(std::mem::size_of_val(&linger)).unwrap_or(0);
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                (&raw mut linger).cast(),
                &raw mut length,
            )
        } != 0
        {
            return 0;
        }
        if linger.l_onoff == 0 {
            0
        } else {
            u64::try_from(linger.l_linger)
                .ok()
                .and_then(|seconds| seconds.checked_mul(1_000))
                .unwrap_or(0)
        }
    };
    #[cfg(not(unix))]
    let milliseconds = 0;
    unsafe { output.write(milliseconds) };
    1
}
