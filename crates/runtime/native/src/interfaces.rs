//! Immutable host network-interface snapshots.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::ffi::CStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct InterfaceAddress {
    family: u8,
    words: [u32; 4],
    prefix: u8,
    scope: u32,
}

#[derive(Clone)]
struct Interface {
    name: String,
    index: u32,
    flags: u32,
    addresses: Vec<InterfaceAddress>,
}

static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(1);

fn snapshots() -> &'static Mutex<BTreeMap<u64, Vec<Interface>>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, Vec<Interface>>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn ipv4_prefix(mask: *const libc::sockaddr) -> u8 {
    if mask.is_null() {
        return 0;
    }
    let mask = unsafe { std::ptr::read_unaligned(mask.cast::<libc::sockaddr_in>()) };
    u8::try_from(u32::from_be(mask.sin_addr.s_addr).leading_ones()).unwrap_or(0)
}

fn ipv6_prefix(mask: *const libc::sockaddr) -> u8 {
    if mask.is_null() {
        return 0;
    }
    let mask = unsafe { std::ptr::read_unaligned(mask.cast::<libc::sockaddr_in6>()) };
    let mut prefix = 0_u8;
    for byte in mask.sin6_addr.s6_addr {
        let ones = byte.leading_ones();
        prefix = prefix.saturating_add(u8::try_from(ones).unwrap_or(0));
        if ones != 8 {
            break;
        }
    }
    prefix
}

fn address(entry: &libc::ifaddrs) -> Option<InterfaceAddress> {
    if entry.ifa_addr.is_null() {
        return None;
    }
    let family = unsafe { i32::from((*entry.ifa_addr).sa_family) };
    match family {
        libc::AF_INET => {
            let socket =
                unsafe { std::ptr::read_unaligned(entry.ifa_addr.cast::<libc::sockaddr_in>()) };
            Some(InterfaceAddress {
                family: 4,
                words: [u32::from_be(socket.sin_addr.s_addr), 0, 0, 0],
                prefix: ipv4_prefix(entry.ifa_netmask),
                scope: 0,
            })
        }
        libc::AF_INET6 => {
            let socket =
                unsafe { std::ptr::read_unaligned(entry.ifa_addr.cast::<libc::sockaddr_in6>()) };
            let mut words = [0_u32; 4];
            for (index, octets) in socket.sin6_addr.s6_addr.chunks_exact(4).enumerate() {
                words[index] = u32::from_be_bytes([octets[0], octets[1], octets[2], octets[3]]);
            }
            Some(InterfaceAddress {
                family: 6,
                words,
                prefix: ipv6_prefix(entry.ifa_netmask),
                scope: socket.sin6_scope_id,
            })
        }
        _ => None,
    }
}

fn capture() -> Option<Vec<Interface>> {
    let mut head = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&raw mut head) } != 0 {
        return None;
    }
    let mut by_index = BTreeMap::<u32, Interface>::new();
    let mut current = head;
    while !current.is_null() {
        let entry = unsafe { &*current };
        if !entry.ifa_name.is_null() {
            let name = unsafe { CStr::from_ptr(entry.ifa_name) }
                .to_string_lossy()
                .into_owned();
            let index = unsafe { libc::if_nametoindex(entry.ifa_name) };
            if index != 0 {
                let interface = by_index.entry(index).or_insert_with(|| Interface {
                    name,
                    index,
                    flags: 0,
                    addresses: Vec::new(),
                });
                interface.flags |= entry.ifa_flags;
                if let Some(address) = address(entry) {
                    interface.addresses.push(address);
                }
            }
        }
        current = entry.ifa_next;
    }
    unsafe { libc::freeifaddrs(head) };
    Some(by_index.into_values().collect())
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_net_interfaces_snapshot() -> u64 {
    let Some(interfaces) = capture() else {
        return 0;
    };
    let handle = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
    let Ok(mut values) = snapshots().lock() else {
        return 0;
    };
    values.insert(handle, interfaces);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_net_interfaces_close(handle: u64) -> u8 {
    let Ok(mut values) = snapshots().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle).is_some())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_net_interface_count(handle: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(values) = snapshots().lock() else {
        return 0;
    };
    let Some(interfaces) = values.get(&handle) else {
        return 0;
    };
    let Ok(count) = u64::try_from(interfaces.len()) else {
        return 0;
    };
    unsafe { output.write(count) };
    1
}

fn with_interface<T>(handle: u64, index: u64, read: impl FnOnce(&Interface) -> T) -> Option<T> {
    let values = snapshots().lock().ok()?;
    let index = usize::try_from(index).ok()?;
    values.get(&handle)?.get(index).map(read)
}

fn with_address<T>(
    handle: u64,
    interface: u64,
    address: u64,
    read: impl FnOnce(&InterfaceAddress) -> T,
) -> Option<T> {
    with_interface(handle, interface, |entry| {
        usize::try_from(address)
            .ok()
            .and_then(|index| entry.addresses.get(index))
            .map(read)
    })?
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_net_interface_name(
    handle: u64,
    index: u64,
    output: *mut u64,
) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Some(name) = with_interface(handle, index, |entry| entry.name.clone()) else {
        return 0;
    };
    let value = crate::allocate_utf8_string_literal(name.as_bytes());
    if value == 0 {
        return 0;
    }
    unsafe { output.write(value) };
    1
}

macro_rules! interface_scalar {
    ($name:ident, $type:ty, $field:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(handle: u64, index: u64, output: *mut $type) -> u8 {
            if output.is_null() {
                return 0;
            }
            let Some(value) = with_interface(handle, index, |entry| entry.$field) else {
                return 0;
            };
            unsafe { output.write(value) };
            1
        }
    };
}

interface_scalar!(pop_rt_net_interface_index, u32, index);
interface_scalar!(pop_rt_net_interface_flags, u32, flags);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_net_interface_address_count(
    handle: u64,
    index: u64,
    output: *mut u64,
) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Some(count) = with_interface(handle, index, |entry| entry.addresses.len()) else {
        return 0;
    };
    let Ok(count) = u64::try_from(count) else {
        return 0;
    };
    unsafe { output.write(count) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_net_interface_address_part(
    handle: u64,
    interface: u64,
    address: u64,
    part: u8,
    word: u8,
    output: *mut u32,
) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Some(value) = with_address(handle, interface, address, |entry| match part {
        0 => Some(u32::from(entry.family)),
        1 => entry.words.get(usize::from(word)).copied(),
        2 => Some(u32::from(entry.prefix)),
        3 => Some(entry.scope),
        _ => None,
    }) else {
        return 0;
    };
    let Some(value) = value else { return 0 };
    unsafe { output.write(value) };
    1
}
