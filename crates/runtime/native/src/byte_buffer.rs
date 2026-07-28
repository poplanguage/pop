//! Native ABI 1.25 reusable `Bytes.Buffer` storage.

use pop_runtime_interface::{
    AllocationClass, ArrayElementMap, ManagedReference, RuntimeAdapter, RuntimeTypeId,
    TableAllocationRequest,
};

use crate::state::{abi_byte_buffers, lock_abi_runtime};

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_create(capacity: u64) -> u64 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return 0;
    };
    let mut bytes = Vec::new();
    if bytes.try_reserve_exact(capacity).is_err() {
        return 0;
    }
    let Ok(request) = TableAllocationRequest::new(
        RuntimeTypeId::new(0),
        AllocationClass::Mature,
        0,
        ArrayElementMap::Scalar,
        ArrayElementMap::Scalar,
    ) else {
        return 0;
    };
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    let Ok(reference) = runtime.allocate_table(&request) else {
        return 0;
    };
    let Ok(mut buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    buffers.insert(reference.raw(), bytes);
    reference.raw()
}

/// Writes the current byte length through `output`.
///
/// # Safety
///
/// `output` must address one writable `u64` for this call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_byte_buffer_length(reference: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return 0;
    }
    let Ok(buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get(&reference) else {
        return 0;
    };
    let Ok(length) = u64::try_from(buffer.len()) else {
        return 0;
    };
    // SAFETY: the caller supplies one writable output slot.
    unsafe { output.write(length) };
    1
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_reserve(reference: u64, additional: u64) -> u8 {
    let Ok(additional) = usize::try_from(additional) else {
        return 0;
    };
    let Ok(mut buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get_mut(&reference) else {
        return 0;
    };
    u8::from(buffer.try_reserve(additional).is_ok())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_clear(reference: u64) -> u8 {
    let Ok(mut buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get_mut(&reference) else {
        return 0;
    };
    buffer.clear();
    1
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_write_byte(reference: u64, value: u8) -> u8 {
    let Ok(mut buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get_mut(&reference) else {
        return 0;
    };
    if buffer.try_reserve(1).is_err() {
        return 0;
    }
    buffer.push(value);
    1
}

fn append_immutable_range(reference: u64, bytes: u64, offset: u64, length: Option<u64>) -> u8 {
    let Ok(runtime) = lock_abi_runtime() else {
        return 0;
    };
    let bytes = ManagedReference::new(bytes);
    let Ok(owner_length) = runtime.immutable_bytes_length(bytes) else {
        return 0;
    };
    let length = length.unwrap_or(owner_length);
    let Some(end) = offset.checked_add(length) else {
        return 0;
    };
    if end > owner_length {
        return 0;
    }
    let (Ok(offset), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
        return 0;
    };
    let Ok(mut buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get_mut(&reference) else {
        return 0;
    };
    if buffer.try_reserve(length).is_err() {
        return 0;
    }
    let start = buffer.len();
    buffer.resize(start + length, 0);
    if runtime
        .immutable_bytes_read(
            bytes,
            u64::try_from(offset).unwrap_or(u64::MAX),
            &mut buffer[start..],
        )
        .is_err()
    {
        buffer.truncate(start);
        return 0;
    }
    1
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_write_bytes(reference: u64, bytes: u64) -> u8 {
    append_immutable_range(reference, bytes, 0, None)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_write_view(
    reference: u64,
    bytes: u64,
    offset: u64,
    length: u64,
) -> u8 {
    append_immutable_range(reference, bytes, offset, Some(length))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_write_integer(
    reference: u64,
    value: u64,
    width: u8,
    order: u8,
) -> u8 {
    let bytes = match (width, order) {
        (2 | 4 | 8, 0) => value.to_be_bytes(),
        (2 | 4 | 8, 1) => value.to_le_bytes(),
        _ => return 0,
    };
    let width = usize::from(width);
    let selected = if order == 0 {
        &bytes[bytes.len() - width..]
    } else {
        &bytes[..width]
    };
    let Ok(mut buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get_mut(&reference) else {
        return 0;
    };
    if buffer.try_reserve(width).is_err() {
        return 0;
    }
    buffer.extend_from_slice(selected);
    1
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_byte_buffer_materialize(reference: u64) -> u64 {
    let Ok(mut runtime) = lock_abi_runtime() else {
        return 0;
    };
    let Ok(buffers) = abi_byte_buffers().lock() else {
        return 0;
    };
    let Some(buffer) = buffers.get(&reference) else {
        return 0;
    };
    runtime
        .allocate_immutable_bytes(buffer)
        .map_or(0, ManagedReference::raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::lock_native_runtime_test;
    use crate::{abi_safe_point, request_abi_collection};

    #[allow(unsafe_code)]
    fn buffer_length(buffer: u64, output: &mut u64) -> u8 {
        // SAFETY: `output` is a writable `u64` for the duration of the call.
        unsafe { pop_rt_byte_buffer_length(buffer, output) }
    }

    #[test]
    fn reusable_buffer_appends_atomically_and_materializes_independent_bytes() {
        let _serial = lock_native_runtime_test();
        let source = lock_abi_runtime()
            .expect("runtime")
            .allocate_immutable_bytes(&[10, 20, 30])
            .expect("source Bytes")
            .raw();
        let buffer = pop_rt_byte_buffer_create(1);
        assert_ne!(buffer, 0);
        assert_eq!(pop_rt_byte_buffer_reserve(buffer, 16), 1);
        assert_eq!(pop_rt_byte_buffer_write_byte(buffer, 170), 1);
        assert_eq!(pop_rt_byte_buffer_write_bytes(buffer, source), 1);
        assert_eq!(pop_rt_byte_buffer_write_view(buffer, source, 1, 2), 1);
        assert_eq!(pop_rt_byte_buffer_write_integer(buffer, 0x0102, 2, 0), 1);
        assert_eq!(pop_rt_byte_buffer_write_integer(buffer, 0x0304, 2, 1), 1);

        let mut length = 0;
        assert_eq!(buffer_length(buffer, &mut length), 1);
        assert_eq!(length, 10);
        let snapshot = pop_rt_byte_buffer_materialize(buffer);
        assert_ne!(snapshot, 0);

        assert_eq!(pop_rt_byte_buffer_clear(buffer), 1);
        assert_eq!(pop_rt_byte_buffer_write_view(buffer, source, 2, 2), 0);
        assert_eq!(buffer_length(buffer, &mut length), 1);
        assert_eq!(length, 0);

        let mut bytes = [0_u8; 10];
        lock_abi_runtime()
            .expect("runtime")
            .immutable_bytes_read(ManagedReference::new(snapshot), 0, &mut bytes)
            .expect("snapshot bytes");
        assert_eq!(bytes, [170, 10, 20, 30, 20, 30, 1, 2, 4, 3]);
    }

    #[test]
    fn reusable_buffer_rejects_forged_references_and_invalid_encodings() {
        let _serial = lock_native_runtime_test();
        let forged = u64::MAX;
        let mut length = 99;
        assert_eq!(buffer_length(forged, &mut length), 0);
        assert_eq!(pop_rt_byte_buffer_reserve(forged, 1), 0);
        assert_eq!(pop_rt_byte_buffer_clear(forged), 0);
        assert_eq!(pop_rt_byte_buffer_write_byte(forged, 1), 0);
        assert_eq!(pop_rt_byte_buffer_write_integer(forged, 1, 3, 0), 0);
        assert_eq!(pop_rt_byte_buffer_materialize(forged), 0);
    }

    #[test]
    fn reusable_buffer_storage_is_discarded_after_collection() {
        let _serial = lock_native_runtime_test();
        let buffer = pop_rt_byte_buffer_create(4);
        assert_ne!(buffer, 0);
        assert_eq!(pop_rt_byte_buffer_write_byte(buffer, 42), 1);

        assert!(request_abi_collection());
        assert_eq!(abi_safe_point(117, &[]), 1);

        let mut length = 99;
        assert_eq!(buffer_length(buffer, &mut length), 0);
        assert_eq!(pop_rt_byte_buffer_write_byte(buffer, 7), 0);
        assert_eq!(pop_rt_byte_buffer_materialize(buffer), 0);
    }
}
