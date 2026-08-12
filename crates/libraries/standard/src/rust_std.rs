//! Direct Rust-standard-library implementations for Pop Standard host APIs.

use std::io::IsTerminal;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

use pop_library_bridge::{NativeExport, poplib};

#[poplib(
    bubble = Standard,
    namespace = "Pop.Process",
    name = "id",
    parameters(),
    results(Int),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_process_id() -> i64 {
    i64::from(std::process::id())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Process",
    name = "availableParallelism",
    parameters(),
    results(Int),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_available_parallelism() -> i64 {
    std::thread::available_parallelism()
        .ok()
        .and_then(|count| i64::try_from(count.get()).ok())
        .unwrap_or(1)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Terminal",
    name = "stdoutIsTerminal",
    parameters(),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Terminal",
    name = "stderrIsTerminal",
    parameters(),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_stderr_is_terminal() -> bool {
    std::io::stderr().is_terminal()
}

fn managed_string(reference: u64) -> Option<String> {
    let bytes = pop_internal::runtime::string_bytes(reference)?;
    String::from_utf8(bytes).ok()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Environment",
    name = "has",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_environment_has(name: u64) -> bool {
    managed_string(name).is_some_and(|name| std::env::var_os(name).is_some())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "exists",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_file_exists(path: u64) -> bool {
    managed_string(path).is_some_and(|path| Path::new(&path).exists())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "isFile",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_file_is_file(path: u64) -> bool {
    managed_string(path).is_some_and(|path| Path::new(&path).is_file())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "exists",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_directory_exists(path: u64) -> bool {
    managed_string(path).is_some_and(|path| Path::new(&path).is_dir())
}

fn ipv4(bits: u64) -> Option<Ipv4Addr> {
    u32::try_from(bits).ok().map(Ipv4Addr::from)
}

fn ipv6(first: u64, second: u64, third: u64, fourth: u64) -> Option<Ipv6Addr> {
    let words = [
        u32::try_from(first).ok()?,
        u32::try_from(second).ok()?,
        u32::try_from(third).ok()?,
        u32::try_from(fourth).ok()?,
    ];
    let mut octets = [0_u8; 16];
    for (index, word) in words.into_iter().enumerate() {
        octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    Some(Ipv6Addr::from(octets))
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsLinkLocal",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_link_local(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_link_local())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsMulticast",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_multicast(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_multicast())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsBroadcast",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_broadcast(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_broadcast())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsDocumentation",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_documentation(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_documentation())
}

macro_rules! ipv6_classifier {
    ($function:ident, $binding:literal, $method:ident) => {
        #[poplib(
                                                    bubble = Standard,
                                                    namespace = "Pop.RustNet",
                                                    name = $binding,
                                                    parameters(UInt64, UInt64, UInt64, UInt64),
                                                    results(Boolean),
                                                    effects(),
                                                )]
        pub extern "C" fn $function(first: u64, second: u64, third: u64, fourth: u64) -> bool {
            ipv6(first, second, third, fourth).is_some_and(|address| address.$method())
        }
    };
}

ipv6_classifier!(
    pop_std_rust_net_ipv6_is_multicast,
    "ipv6IsMulticast",
    is_multicast
);
ipv6_classifier!(
    pop_std_rust_net_ipv6_is_unique_local,
    "ipv6IsUniqueLocal",
    is_unique_local
);
ipv6_classifier!(
    pop_std_rust_net_ipv6_is_unicast_link_local,
    "ipv6IsUnicastLinkLocal",
    is_unicast_link_local
);

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv6IsDocumentation",
    parameters(UInt64, UInt64, UInt64, UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv6_is_documentation(
    first: u64,
    second: u64,
    third: u64,
    fourth: u64,
) -> bool {
    ipv6(first, second, third, fourth)
        .is_some_and(|address| address.segments()[..2] == [0x2001, 0x0db8])
}

pub const RUST_STD_EXPORTS: &[NativeExport] = &[
    POP_STD_RUST_PROCESS_ID_POPLIB_EXPORT,
    POP_STD_RUST_AVAILABLE_PARALLELISM_POPLIB_EXPORT,
    POP_STD_RUST_STDOUT_IS_TERMINAL_POPLIB_EXPORT,
    POP_STD_RUST_STDERR_IS_TERMINAL_POPLIB_EXPORT,
    POP_STD_RUST_ENVIRONMENT_HAS_POPLIB_EXPORT,
    POP_STD_RUST_FILE_EXISTS_POPLIB_EXPORT,
    POP_STD_RUST_FILE_IS_FILE_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_EXISTS_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_LINK_LOCAL_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_MULTICAST_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_BROADCAST_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_DOCUMENTATION_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_MULTICAST_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_UNIQUE_LOCAL_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_UNICAST_LINK_LOCAL_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_DOCUMENTATION_POPLIB_EXPORT,
];
