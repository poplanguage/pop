use pop_library_bridge::NativeExport;
use pop_runtime_interface::{GarbageCollectorContract, GarbageCollectorStage, RuntimeOperation};
use pop_runtime_native_abi::symbol;

/// Trusted native adapters owned by this runtime-service module.
///
/// The current string bridge consumes PLRI and does not introduce a second
/// exported native entry, so its reviewed inventory is empty.
pub const NATIVE_EXPORTS: &[NativeExport] = &[];

#[must_use]
pub const fn garbage_collector_stage() -> GarbageCollectorStage {
    GarbageCollectorContract::bootstrap_stage1().stage()
}

#[must_use]
pub const fn runtime_symbol(operation: RuntimeOperation) -> Option<&'static str> {
    symbol(operation)
}

#[allow(unsafe_code)]
unsafe extern "C" {
    fn pop_rt_string_read(reference: u64, target: *mut u8, capacity: u64) -> u64;
    fn pop_rt_byte_buffer_clear(reference: u64) -> u8;
    fn pop_rt_byte_buffer_write_byte(reference: u64, value: u8) -> u8;
}

/// Copies a bootstrap managed `String` through the trusted runtime ABI.
#[must_use]
#[allow(unsafe_code)]
pub fn string_bytes(reference: u64) -> Option<Vec<u8>> {
    // SAFETY: A null target requests only the validated byte length.
    let encoded_length = unsafe { pop_rt_string_read(reference, std::ptr::null_mut(), 0) };
    let length = encoded_length.checked_sub(1)?;
    let length = usize::try_from(length).ok()?;
    let mut bytes = vec![0_u8; length];
    // SAFETY: `bytes` exposes exactly `length` writable bytes.
    let copied =
        unsafe { pop_rt_string_read(reference, bytes.as_mut_ptr(), u64::try_from(length).ok()?) };
    (copied == encoded_length).then_some(bytes)
}

/// Clears one caller-owned native byte buffer.
#[must_use]
#[allow(unsafe_code)]
pub fn byte_buffer_clear(reference: u64) -> bool {
    // SAFETY: the runtime owns validation of the opaque buffer token.
    unsafe { pop_rt_byte_buffer_clear(reference) != 0 }
}

/// Appends one byte to one caller-owned native byte buffer.
#[must_use]
#[allow(unsafe_code)]
pub fn byte_buffer_write_byte(reference: u64, value: u8) -> bool {
    // SAFETY: the runtime owns validation of the opaque buffer token.
    unsafe { pop_rt_byte_buffer_write_byte(reference, value) != 0 }
}
