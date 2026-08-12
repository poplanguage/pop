//! Address-family-specific TCP endpoint construction.
#![allow(unsafe_code)]

use std::net::{Ipv6Addr, SocketAddrV6, TcpListener, TcpStream};

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_listen_ipv6(
    first: u32,
    second: u32,
    third: u32,
    fourth: u32,
    port: u16,
    scope: u32,
) -> u64 {
    let address = SocketAddrV6::new(ipv6_address(first, second, third, fourth), port, 0, scope);
    let Ok(listener) = TcpListener::bind(address) else {
        return 0;
    };
    if listener.set_nonblocking(true).is_err() {
        return 0;
    }
    crate::tcp::insert_listener(listener)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_tcp_connect_ipv6(
    first: u32,
    second: u32,
    third: u32,
    fourth: u32,
    port: u16,
    scope: u32,
) -> u64 {
    let address = SocketAddrV6::new(ipv6_address(first, second, third, fourth), port, 0, scope);
    let Ok(stream) = TcpStream::connect(address) else {
        return 0;
    };
    if stream.set_nonblocking(true).is_err() {
        return 0;
    }
    crate::tcp::insert_stream(stream)
}

fn ipv6_address(first: u32, second: u32, third: u32, fourth: u32) -> Ipv6Addr {
    let mut octets = [0_u8; 16];
    for (index, word) in [first, second, third, fourth].into_iter().enumerate() {
        octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    Ipv6Addr::from(octets)
}
