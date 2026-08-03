//! Explicit bounded system DNS resolver capabilities.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::pop_rt_string_read;

static NEXT_DNS: AtomicU64 = AtomicU64::new(1);

fn resolvers() -> &'static Mutex<BTreeSet<u64>> {
    static VALUES: OnceLock<Mutex<BTreeSet<u64>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn answers() -> &'static Mutex<BTreeMap<u64, Vec<IpAddr>>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, Vec<IpAddr>>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_handle() -> u64 {
    NEXT_DNS.fetch_add(1, Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_dns_resolver_create() -> u64 {
    let handle = next_handle();
    let Ok(mut values) = resolvers().lock() else {
        return 0;
    };
    values.insert(handle);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_dns_resolver_close(handle: u64) -> u8 {
    let Ok(mut values) = resolvers().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_dns_resolve(resolver: u64, name: u64, limit: u16) -> u64 {
    if limit == 0
        || !resolvers()
            .lock()
            .is_ok_and(|values| values.contains(&resolver))
    {
        return 0;
    }
    let encoded = unsafe { pop_rt_string_read(name, std::ptr::null_mut(), 0) };
    let Some(length) = encoded
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
    else {
        return 0;
    };
    let mut bytes = vec![0; length];
    if unsafe { pop_rt_string_read(name, bytes.as_mut_ptr(), length as u64) } == 0 {
        return 0;
    }
    let Ok(name) = String::from_utf8(bytes) else {
        return 0;
    };
    let Ok(addresses) = (name.as_str(), 0).to_socket_addrs() else {
        return 0;
    };
    let mut values = Vec::new();
    for address in addresses.map(|entry| entry.ip()) {
        if !values.contains(&address) {
            values.push(address);
            if values.len() == usize::from(limit) {
                break;
            }
        }
    }
    let handle = next_handle();
    let Ok(mut stored) = answers().lock() else {
        return 0;
    };
    stored.insert(handle, values);
    handle
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_dns_answer_count(handle: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = answers().lock() else {
        return 0;
    };
    let Some(values) = values.get(&handle) else {
        return 0;
    };
    let Ok(count) = u64::try_from(values.len()) else {
        return 0;
    };
    unsafe { output.write(count) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_dns_answer_family(handle: u64, index: u64, output: *mut u8) -> u8 {
    let Some(value) = answer(handle, index) else {
        return 0;
    };
    if output.is_null() {
        return 0;
    }
    unsafe { output.write(if value.is_ipv4() { 4 } else { 6 }) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_dns_answer_ipv4(handle: u64, index: u64, output: *mut u32) -> u8 {
    let Some(IpAddr::V4(value)) = answer(handle, index) else {
        return 0;
    };
    if output.is_null() {
        return 0;
    }
    unsafe { output.write(u32::from(value)) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_dns_answer_ipv6_word(
    handle: u64,
    index: u64,
    word: u8,
    output: *mut u32,
) -> u8 {
    let Some(IpAddr::V6(value)) = answer(handle, index) else {
        return 0;
    };
    if output.is_null() || word >= 4 {
        return 0;
    }
    let octets = value.octets();
    let start = usize::from(word) * 4;
    unsafe {
        output.write(u32::from_be_bytes(
            octets[start..start + 4].try_into().unwrap_or([0; 4]),
        ));
    };
    1
}

fn answer(handle: u64, index: u64) -> Option<IpAddr> {
    let index = usize::try_from(index).ok()?;
    answers().lock().ok()?.get(&handle)?.get(index).copied()
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_dns_answers_close(handle: u64) -> u8 {
    let Ok(mut values) = answers().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle).is_some())
}
