//! Closed native exports for Atomic integer fetch operations.
#![allow(unsafe_code, clippy::missing_safety_doc)]

use crate::atomic::{AtomicIntFetchOperation, atomic_int_fetch};

macro_rules! atomic_int_fetch_export {
    ($name:ident, $operation:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(handle: u64, value: i64, order: u8, output: *mut u64) -> u8 {
            // SAFETY: the closed helper validates the handle, order, and output pointer.
            unsafe { atomic_int_fetch(handle, value, order, output, $operation) }
        }
    };
}

atomic_int_fetch_export!(pop_rt_atomic_int_fetch_add, AtomicIntFetchOperation::Add);
atomic_int_fetch_export!(
    pop_rt_atomic_int_fetch_subtract,
    AtomicIntFetchOperation::Subtract
);
atomic_int_fetch_export!(pop_rt_atomic_int_fetch_and, AtomicIntFetchOperation::And);
atomic_int_fetch_export!(pop_rt_atomic_int_fetch_or, AtomicIntFetchOperation::Or);
atomic_int_fetch_export!(pop_rt_atomic_int_fetch_xor, AtomicIntFetchOperation::Xor);
