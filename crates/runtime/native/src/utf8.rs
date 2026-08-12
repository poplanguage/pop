//! Checked UTF-8 transcoding adapters.

use pop_runtime_interface::{ManagedReference, RuntimeAdapter};

use crate::state::{abi_byte_buffers, lock_abi_runtime};
use crate::text::{allocate_utf8_string, utf8_string_bytes};

/// Encodes one checked byte range from a valid managed UTF-8 string.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_text_view_encode_utf8(
    reference: u64,
    byte_offset: u64,
    byte_length: u64,
) -> u64 {
    let (Ok(byte_offset), Ok(byte_length)) =
        (usize::try_from(byte_offset), usize::try_from(byte_length))
    else {
        return 0;
    };
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    let Some(bytes) = utf8_string_bytes(&runtime, ManagedReference::new(reference)) else {
        return 0;
    };
    let Some(selected) = byte_offset
        .checked_add(byte_length)
        .and_then(|end| bytes.get(byte_offset..end))
    else {
        return 0;
    };
    if std::str::from_utf8(selected).is_err() {
        return 0;
    }
    runtime
        .allocate_immutable_bytes(selected)
        .map_or(0, ManagedReference::raw)
}

/// Decodes one checked immutable byte range as UTF-8.
///
/// Status `0` is a runtime failure, `1` is malformed UTF-8, and `2` is a
/// successfully allocated string.
///
/// # Safety
///
/// `output` must address one writable `u64` for this call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_bytes_view_decode_utf8(
    reference: u64,
    byte_offset: u64,
    byte_length: u64,
    output: *mut u64,
) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(byte_length) = usize::try_from(byte_length) else {
        return 0;
    };
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    let mut bytes = vec![0_u8; byte_length];
    if runtime
        .immutable_bytes_read(ManagedReference::new(reference), byte_offset, &mut bytes)
        .is_err()
    {
        return 0;
    }
    if std::str::from_utf8(&bytes).is_err() {
        return 1;
    }
    let Ok(string) = allocate_utf8_string(&mut runtime, &bytes) else {
        return 0;
    };
    // SAFETY: the caller supplies one writable output slot.
    unsafe { output.write(string.raw()) };
    2
}

/// Decodes the complete reusable byte accumulator as UTF-8 without consuming it.
///
/// Status `0` is a runtime failure, `1` is malformed UTF-8, and `2` is a
/// successfully allocated string.
///
/// # Safety
///
/// `output` must address one writable `u64` for this call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_byte_buffer_decode_utf8(reference: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    let bytes = {
        let Ok(buffers) = abi_byte_buffers().lock() else {
            return 0;
        };
        let Some(buffer) = buffers.get(&reference) else {
            return 0;
        };
        buffer.clone()
    };
    if std::str::from_utf8(&bytes).is_err() {
        return 1;
    }
    let Ok(string) = allocate_utf8_string(&mut runtime, &bytes) else {
        return 0;
    };
    // SAFETY: the caller supplies one writable output slot.
    unsafe { output.write(string.raw()) };
    2
}
