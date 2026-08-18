//! Native ABI 1.17 immutable `Bytes` payload borrowing.

use pop_runtime_interface::{FfiBytesBorrowId, ManagedReference, RuntimeAdapter};

use crate::state::{abi_byte_buffers, lock_abi_runtime};

/// Copies a validated byte-buffer range into caller-owned storage.
///
/// # Safety
///
/// `target` must address at least `capacity` writable bytes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_byte_buffer_read(
    reference: u64,
    offset: u64,
    target: *mut u8,
    capacity: u64,
) -> u64 {
    if capacity > 0 && target.is_null() {
        return 0;
    }
    let Ok(offset) = usize::try_from(offset) else {
        return 0;
    };
    let Ok(capacity) = usize::try_from(capacity) else {
        return 0;
    };
    let Ok(buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get(&reference) else {
        return 0;
    };
    let Some(bytes) = buffer.get(offset..offset.saturating_add(capacity)) else {
        return 0;
    };
    // SAFETY: the caller provided a writable region of exactly `capacity`
    // bytes, and `bytes` is bounded by that capacity.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
    u64::try_from(bytes.len()).unwrap_or(0)
}

/// Allocates the trusted packed immutable-byte representation for native
/// library adapters and tests.
#[must_use]
pub fn allocate_immutable_bytes(bytes: &[u8]) -> u64 {
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    runtime
        .allocate_immutable_bytes(bytes)
        .map_or(0, ManagedReference::raw)
}

/// Borrows only the immutable byte payload and leaves outputs unchanged on
/// failure.
///
/// # Safety
///
/// Both outputs must address writable `u64` values for the duration of this
/// call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_ffi_bytes_borrow(
    bytes: u64,
    out_address: *mut u64,
    out_length: *mut u64,
) -> u64 {
    if bytes == 0 || out_address.is_null() || out_length.is_null() {
        return 0;
    }
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    let Ok(borrow) = runtime.ffi_bytes_borrow(ManagedReference::new(bytes)) else {
        return 0;
    };
    let address = borrow
        .address()
        .map_or(0, pop_runtime_interface::ForeignAddress::raw);
    // SAFETY: The caller contract requires two writable `u64` outputs.
    unsafe {
        out_address.write(address);
        out_length.write(borrow.length());
    }
    borrow.id().raw()
}

/// Ends one exact immutable byte-payload borrow.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_ffi_bytes_end_borrow(bytes: u64, borrow: u64) -> u8 {
    let Some(borrow) = FfiBytesBorrowId::new(borrow) else {
        return 0;
    };
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    u8::from(
        runtime
            .ffi_bytes_end_borrow(ManagedReference::new(bytes), borrow)
            .is_ok(),
    )
}
