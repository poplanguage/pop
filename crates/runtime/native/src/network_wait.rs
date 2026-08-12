//! Deadline- and cancellation-aware nonblocking network transfer adapters.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::thread;
use std::time::Duration;

use pop_runtime_native_abi::SocketIoStatus;

const WAIT_TIMED_OUT: u8 = 4;
const WAIT_CANCELLED: u8 = 5;
const RETRY_QUANTUM: Duration = Duration::from_millis(1);

fn run_until(deadline: u64, cancel: u64, mut attempt: impl FnMut() -> u8) -> u8 {
    loop {
        if crate::pop_rt_task_cancellation_requested(cancel) == 1 {
            return WAIT_CANCELLED;
        }
        let status = attempt();
        if status != SocketIoStatus::WouldBlock as u8 {
            return status;
        }
        let Some(remaining) = crate::monotonic_time::deadline_remaining(deadline) else {
            return SocketIoStatus::Failure as u8;
        };
        if remaining.is_zero() {
            return WAIT_TIMED_OUT;
        }
        thread::sleep(remaining.min(RETRY_QUANTUM));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_send_bytes_until(
    stream: u64,
    bytes: u64,
    deadline: u64,
    cancel: u64,
    written: *mut u64,
) -> u8 {
    run_until(deadline, cancel, || unsafe {
        crate::pop_rt_tcp_send_bytes(stream, bytes, written)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_tcp_receive_buffer_until(
    stream: u64,
    buffer: u64,
    capacity: u64,
    deadline: u64,
    cancel: u64,
    received: *mut u64,
) -> u8 {
    run_until(deadline, cancel, || unsafe {
        crate::pop_rt_tcp_receive_buffer(stream, buffer, capacity, received)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_send_bytes_to_until(
    socket: u64,
    address: u32,
    port: u16,
    bytes: u64,
    deadline: u64,
    cancel: u64,
    written: *mut u64,
) -> u8 {
    run_until(deadline, cancel, || unsafe {
        crate::pop_rt_udp_send_bytes_to(socket, address, port, bytes, written)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_udp_receive_buffer_until(
    socket: u64,
    buffer: u64,
    capacity: u64,
    deadline: u64,
    cancel: u64,
    address: *mut u32,
    port: *mut u16,
    received: *mut u64,
) -> u8 {
    run_until(deadline, cancel, || unsafe {
        crate::pop_rt_udp_receive_buffer(socket, buffer, capacity, address, port, received)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_unix_send_bytes_until(
    stream: u64,
    bytes: u64,
    deadline: u64,
    cancel: u64,
    written: *mut u64,
) -> u8 {
    run_until(deadline, cancel, || unsafe {
        crate::pop_rt_unix_send_bytes(stream, bytes, written)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_unix_receive_buffer_until(
    stream: u64,
    buffer: u64,
    capacity: u64,
    deadline: u64,
    cancel: u64,
    received: *mut u64,
) -> u8 {
    run_until(deadline, cancel, || unsafe {
        crate::pop_rt_unix_receive_buffer(stream, buffer, capacity, received)
    })
}
