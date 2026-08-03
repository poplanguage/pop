//! Typed native handles for the backend-neutral Atomic contract.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use pop_runtime_interface::{
    AtomicBoolean, AtomicCompareExchangeOrder, AtomicInt, AtomicLoadOrder,
    AtomicReadModifyWriteOrder, AtomicStoreOrder,
};

const STATUS_FAILURE: u8 = 0;
const STATUS_SUCCESS: u8 = 1;
const STATUS_MISMATCH: u8 = 2;

enum NativeAtomic {
    Integer(AtomicInt),
    Boolean(AtomicBoolean),
}

static NEXT_ATOMIC: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Mutex<BTreeMap<u64, NativeAtomic>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<u64, NativeAtomic>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_handle() -> Option<u64> {
    NEXT_ATOMIC
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|next| *next != 0)
        })
        .ok()
}

fn insert(value: NativeAtomic) -> u64 {
    let Some(handle) = next_handle() else {
        return 0;
    };
    let Ok(mut values) = registry().lock() else {
        return 0;
    };
    values.insert(handle, value);
    handle
}

const fn load_order(raw: u8) -> Option<AtomicLoadOrder> {
    match raw {
        0 => Some(AtomicLoadOrder::Relaxed),
        1 => Some(AtomicLoadOrder::Acquire),
        2 => Some(AtomicLoadOrder::SequentiallyConsistent),
        _ => None,
    }
}

const fn store_order(raw: u8) -> Option<AtomicStoreOrder> {
    match raw {
        0 => Some(AtomicStoreOrder::Relaxed),
        1 => Some(AtomicStoreOrder::Release),
        2 => Some(AtomicStoreOrder::SequentiallyConsistent),
        _ => None,
    }
}

const fn read_modify_write_order(raw: u8) -> Option<AtomicReadModifyWriteOrder> {
    match raw {
        0 => Some(AtomicReadModifyWriteOrder::Relaxed),
        1 => Some(AtomicReadModifyWriteOrder::Acquire),
        2 => Some(AtomicReadModifyWriteOrder::Release),
        3 => Some(AtomicReadModifyWriteOrder::AcquireRelease),
        4 => Some(AtomicReadModifyWriteOrder::SequentiallyConsistent),
        _ => None,
    }
}

fn compare_order(success: u8, failure: u8) -> Option<AtomicCompareExchangeOrder> {
    AtomicCompareExchangeOrder::new(read_modify_write_order(success)?, load_order(failure)?)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_atomic_int_create(value: i64) -> u64 {
    insert(NativeAtomic::Integer(AtomicInt::new(value)))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_atomic_int_load(handle: u64, order: u8, output: *mut u64) -> u8 {
    let Some(order) = load_order(order) else {
        return STATUS_FAILURE;
    };
    if output.is_null() {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Integer(value)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    // SAFETY: the caller supplied a non-null scalar output pointer.
    unsafe { output.write(value.load(order).cast_unsigned()) };
    STATUS_SUCCESS
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_atomic_int_store(handle: u64, value: i64, order: u8) -> u8 {
    let Some(order) = store_order(order) else {
        return STATUS_FAILURE;
    };
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Integer(value_ref)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    value_ref.store(value, order);
    STATUS_SUCCESS
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_atomic_int_swap(
    handle: u64,
    value: i64,
    order: u8,
    output: *mut u64,
) -> u8 {
    let Some(order) = read_modify_write_order(order) else {
        return STATUS_FAILURE;
    };
    if output.is_null() {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Integer(value_ref)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    // SAFETY: the caller supplied a non-null scalar output pointer.
    unsafe { output.write(value_ref.swap(value, order).cast_unsigned()) };
    STATUS_SUCCESS
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_atomic_int_compare_exchange(
    handle: u64,
    current: i64,
    new: i64,
    success: u8,
    failure: u8,
    output: *mut u64,
) -> u8 {
    let Some(order) = compare_order(success, failure) else {
        return STATUS_FAILURE;
    };
    if output.is_null() {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Integer(value_ref)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    let result = value_ref.compare_exchange(current, new, order);
    // SAFETY: the caller supplied a non-null scalar output pointer.
    unsafe { output.write(result.previous().cast_unsigned()) };
    if result.exchanged() {
        STATUS_SUCCESS
    } else {
        STATUS_MISMATCH
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_atomic_bool_create(value: u8) -> u64 {
    if value > 1 {
        return 0;
    }
    insert(NativeAtomic::Boolean(AtomicBoolean::new(value == 1)))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_atomic_bool_load(handle: u64, order: u8, output: *mut u8) -> u8 {
    let Some(order) = load_order(order) else {
        return STATUS_FAILURE;
    };
    if output.is_null() {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Boolean(value)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    // SAFETY: the caller supplied a non-null scalar output pointer.
    unsafe { output.write(u8::from(value.load(order))) };
    STATUS_SUCCESS
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_atomic_bool_store(handle: u64, value: u8, order: u8) -> u8 {
    let Some(order) = store_order(order) else {
        return STATUS_FAILURE;
    };
    if value > 1 {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Boolean(value_ref)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    value_ref.store(value == 1, order);
    STATUS_SUCCESS
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_atomic_bool_swap(
    handle: u64,
    value: u8,
    order: u8,
    output: *mut u8,
) -> u8 {
    let Some(order) = read_modify_write_order(order) else {
        return STATUS_FAILURE;
    };
    if value > 1 || output.is_null() {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Boolean(value_ref)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    // SAFETY: the caller supplied a non-null scalar output pointer.
    unsafe { output.write(u8::from(value_ref.swap(value == 1, order))) };
    STATUS_SUCCESS
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_atomic_bool_compare_exchange(
    handle: u64,
    current: u8,
    new: u8,
    success: u8,
    failure: u8,
    output: *mut u8,
) -> u8 {
    let Some(order) = compare_order(success, failure) else {
        return STATUS_FAILURE;
    };
    if current > 1 || new > 1 || output.is_null() {
        return STATUS_FAILURE;
    }
    let Ok(values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    let Some(NativeAtomic::Boolean(value_ref)) = values.get(&handle) else {
        return STATUS_FAILURE;
    };
    let result = value_ref.compare_exchange(current == 1, new == 1, order);
    // SAFETY: the caller supplied a non-null scalar output pointer.
    unsafe { output.write(u8::from(result.previous())) };
    if result.exchanged() {
        STATUS_SUCCESS
    } else {
        STATUS_MISMATCH
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_atomic_release(handle: u64) -> u8 {
    let Ok(mut values) = registry().lock() else {
        return STATUS_FAILURE;
    };
    u8::from(values.remove(&handle).is_some())
}
