//! Immutable Linux route-table snapshots.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct Route {
    family: u8,
    destination: [u32; 4],
    prefix: u8,
    gateway: [u32; 4],
    interface: u32,
    metric: u32,
    flags: u32,
}

static NEXT_ROUTES: AtomicU64 = AtomicU64::new(1);

fn snapshots() -> &'static Mutex<BTreeMap<u64, Vec<Route>>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, Vec<Route>>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn interface_index(name: &str) -> u32 {
    CString::new(name)
        .ok()
        .map_or(0, |name| unsafe { libc::if_nametoindex(name.as_ptr()) })
}

fn parse_hex_u32(value: &str) -> Option<u32> {
    u32::from_str_radix(value, 16).ok()
}

fn ipv6_words(value: &str) -> Option<[u32; 4]> {
    if value.len() != 32 {
        return None;
    }
    let mut words = [0_u32; 4];
    for (index, word) in words.iter_mut().enumerate() {
        *word = parse_hex_u32(&value[index * 8..index * 8 + 8])?;
    }
    Some(words)
}

fn capture_ipv4(routes: &mut Vec<Route>) {
    let Ok(text) = std::fs::read_to_string("/proc/net/route") else {
        return;
    };
    for line in text.lines().skip(1) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 8 {
            continue;
        }
        let Some(destination) = parse_hex_u32(fields[1]) else {
            continue;
        };
        let Some(gateway) = parse_hex_u32(fields[2]) else {
            continue;
        };
        let Some(flags) = parse_hex_u32(fields[3]) else {
            continue;
        };
        let Some(metric) = fields[6].parse::<u32>().ok() else {
            continue;
        };
        let Some(mask) = parse_hex_u32(fields[7]) else {
            continue;
        };
        let mask = mask.swap_bytes();
        routes.push(Route {
            family: 4,
            destination: [destination.swap_bytes(), 0, 0, 0],
            prefix: u8::try_from(mask.leading_ones()).unwrap_or(0),
            gateway: [gateway.swap_bytes(), 0, 0, 0],
            interface: interface_index(fields[0]),
            metric,
            flags,
        });
    }
}

fn capture_ipv6(routes: &mut Vec<Route>) {
    let Ok(text) = std::fs::read_to_string("/proc/net/ipv6_route") else {
        return;
    };
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 {
            continue;
        }
        let Some(destination) = ipv6_words(fields[0]) else {
            continue;
        };
        let Some(prefix) = u8::from_str_radix(fields[1], 16).ok() else {
            continue;
        };
        let Some(gateway) = ipv6_words(fields[4]) else {
            continue;
        };
        let Some(metric) = parse_hex_u32(fields[5]) else {
            continue;
        };
        let Some(flags) = parse_hex_u32(fields[8]) else {
            continue;
        };
        routes.push(Route {
            family: 6,
            destination,
            prefix,
            gateway,
            interface: interface_index(fields[9]),
            metric,
            flags,
        });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_net_routes_snapshot() -> u64 {
    let mut routes = Vec::new();
    capture_ipv4(&mut routes);
    capture_ipv6(&mut routes);
    let handle = NEXT_ROUTES.fetch_add(1, Ordering::Relaxed);
    let Ok(mut values) = snapshots().lock() else {
        return 0;
    };
    values.insert(handle, routes);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_net_routes_close(handle: u64) -> u8 {
    let Ok(mut values) = snapshots().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle).is_some())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_net_route_count(handle: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = snapshots().lock() else {
        return 0;
    };
    let Some(routes) = values.get(&handle) else {
        return 0;
    };
    let Ok(count) = u64::try_from(routes.len()) else {
        return 0;
    };
    unsafe { output.write(count) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_net_route_part(
    handle: u64,
    index: u64,
    part: u8,
    word: u8,
    output: *mut u32,
) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = snapshots().lock() else {
        return 0;
    };
    let Some(route) = usize::try_from(index)
        .ok()
        .and_then(|index| values.get(&handle)?.get(index))
    else {
        return 0;
    };
    let value = match part {
        0 => Some(u32::from(route.family)),
        1 => route.destination.get(usize::from(word)).copied(),
        2 => Some(u32::from(route.prefix)),
        3 => route.gateway.get(usize::from(word)).copied(),
        4 => Some(route.interface),
        5 => Some(route.metric),
        6 => Some(route.flags),
        _ => None,
    };
    let Some(value) = value else { return 0 };
    unsafe { output.write(value) };
    1
}
