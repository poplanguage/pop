//! Explicit live monotonic clocks and deadline capabilities.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static NEXT_TIME: AtomicU64 = AtomicU64::new(1);

fn clocks() -> &'static Mutex<BTreeMap<u64, Instant>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, Instant>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn deadlines() -> &'static Mutex<BTreeMap<u64, (u64, Instant)>> {
    static VALUES: OnceLock<Mutex<BTreeMap<u64, (u64, Instant)>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_handle() -> u64 {
    NEXT_TIME.fetch_add(1, Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_monotonic_clock_create() -> u64 {
    let handle = next_handle();
    let Ok(mut values) = clocks().lock() else {
        return 0;
    };
    values.insert(handle, Instant::now());
    handle
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_monotonic_clock_now(
    clock: u64,
    seconds: *mut u64,
    nanoseconds: *mut u32,
) -> u8 {
    if seconds.is_null() || nanoseconds.is_null() {
        return 0;
    }
    let Ok(values) = clocks().lock() else {
        return 0;
    };
    let Some(origin) = values.get(&clock) else {
        return 0;
    };
    let elapsed = origin.elapsed();
    unsafe {
        seconds.write(elapsed.as_secs());
        nanoseconds.write(elapsed.subsec_nanos());
    }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_deadline_after(clock: u64, seconds: u64, nanoseconds: u32) -> u64 {
    if nanoseconds >= 1_000_000_000 {
        return 0;
    }
    let Ok(values) = clocks().lock() else {
        return 0;
    };
    if !values.contains_key(&clock) {
        return 0;
    }
    let Some(target) = Instant::now().checked_add(Duration::new(seconds, nanoseconds)) else {
        return 0;
    };
    drop(values);
    let handle = next_handle();
    let Ok(mut values) = deadlines().lock() else {
        return 0;
    };
    values.insert(handle, (clock, target));
    handle
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_deadline_expired(clock: u64, deadline: u64, output: *mut u8) -> u8 {
    if output.is_null()
        || !clocks()
            .lock()
            .is_ok_and(|values| values.contains_key(&clock))
    {
        return 0;
    }
    let Ok(values) = deadlines().lock() else {
        return 0;
    };
    let Some((owner, target)) = values.get(&deadline) else {
        return 0;
    };
    if *owner != clock {
        return 0;
    }
    unsafe { output.write(u8::from(Instant::now() >= *target)) };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_deadline_close(handle: u64) -> u8 {
    let Ok(mut values) = deadlines().lock() else {
        return 0;
    };
    u8::from(values.remove(&handle).is_some())
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_monotonic_clock_close(handle: u64) -> u8 {
    let Ok(mut values) = clocks().lock() else {
        return 0;
    };
    if values.remove(&handle).is_none() {
        return 0;
    }
    drop(values);
    if let Ok(mut values) = deadlines().lock() {
        values.retain(|_, (clock, _)| *clock != handle);
    }
    1
}

#[allow(
    dead_code,
    reason = "consumed by the next deadline-aware transport ABI slice"
)]
pub(crate) fn deadline_remaining(handle: u64) -> Option<Duration> {
    let values = deadlines().lock().ok()?;
    let (_, target) = values.get(&handle)?;
    Some(target.saturating_duration_since(Instant::now()))
}
