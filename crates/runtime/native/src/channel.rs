//! Native ABI storage for bounded typed channels.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use pop_runtime_interface::{
    ChannelId, ChannelLifecycle, ChannelReceive, ChannelSendError, ChannelState, ManagedReference,
    RootHandle, RuntimeAdapter,
};
use pop_runtime_native_abi::{ChannelReceiveStatus, ChannelSendStatus};

use crate::state::lock_abi_runtime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeChannelValue {
    Scalar(u64),
    Managed(RootHandle),
}

static NEXT_CHANNEL: AtomicU64 = AtomicU64::new(1);

fn channels() -> &'static Mutex<BTreeMap<u64, ChannelLifecycle<NativeChannelValue>>> {
    static CHANNELS: OnceLock<Mutex<BTreeMap<u64, ChannelLifecycle<NativeChannelValue>>>> =
        OnceLock::new();
    CHANNELS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_channel() -> Option<u64> {
    NEXT_CHANNEL
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|next| *next != 0)
        })
        .ok()
}

fn release_values(values: impl IntoIterator<Item = NativeChannelValue>) -> bool {
    let roots: Vec<_> = values
        .into_iter()
        .filter_map(|value| match value {
            NativeChannelValue::Scalar(_) => None,
            NativeChannelValue::Managed(root) => Some(root),
        })
        .collect();
    if roots.is_empty() {
        return true;
    }
    let Ok(mut runtime) = lock_abi_runtime() else {
        return false;
    };
    let mut released = true;
    for root in roots {
        released &= runtime.release_root(root).is_ok();
    }
    released
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_create(capacity: u64) -> u64 {
    let Some(handle) = next_channel() else {
        return 0;
    };
    let Ok(mut registry) = channels().lock() else {
        return 0;
    };
    registry.insert(
        handle,
        ChannelLifecycle::bounded(ChannelId::new(handle), capacity),
    );
    handle
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_retain_sender(channel: u64) -> u8 {
    let Ok(mut registry) = channels().lock() else {
        return 0;
    };
    let Some(channel) = registry.get_mut(&channel) else {
        return 0;
    };
    u8::from(channel.retain_sender().is_ok())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_release_sender(channel: u64) -> u8 {
    let Ok(mut registry) = channels().lock() else {
        return 0;
    };
    let Some(record) = registry.get_mut(&channel) else {
        return 0;
    };
    if record.sender_count() == 0 {
        return 0;
    }
    record.release_sender();
    let remove = record.sender_count() == 0 && record.receiver_count() == 0;
    if remove {
        registry.remove(&channel);
    }
    1
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_retain_receiver(channel: u64) -> u8 {
    let Ok(mut registry) = channels().lock() else {
        return 0;
    };
    let Some(channel) = registry.get_mut(&channel) else {
        return 0;
    };
    u8::from(channel.retain_receiver().is_ok())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_release_receiver(channel: u64) -> u8 {
    let values = {
        let Ok(mut registry) = channels().lock() else {
            return 0;
        };
        let Some(record) = registry.get_mut(&channel) else {
            return 0;
        };
        if record.receiver_count() == 0 {
            return 0;
        }
        let values = record.release_receiver();
        let remove = record.state() == ChannelState::Closed;
        if remove {
            registry.remove(&channel);
        }
        values
    };
    u8::from(release_values(values))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_close(channel: u64) -> u8 {
    let Ok(mut registry) = channels().lock() else {
        return 0;
    };
    let Some(record) = registry.get_mut(&channel) else {
        return 0;
    };
    let closed = record.close();
    let remove = record.state() == ChannelState::Closed;
    if remove {
        registry.remove(&channel);
    }
    u8::from(closed)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_channel_try_send(channel: u64, value: u64, managed: u8) -> u8 {
    let stored = match managed {
        0 => NativeChannelValue::Scalar(value),
        1 => {
            let Ok(mut runtime) = lock_abi_runtime() else {
                return ChannelSendStatus::Failure as u8;
            };
            let Ok(root) = runtime.retain_root(ManagedReference::new(value)) else {
                return ChannelSendStatus::Failure as u8;
            };
            NativeChannelValue::Managed(root)
        }
        _ => return ChannelSendStatus::Failure as u8,
    };
    let result = channels().lock().ok().and_then(|mut registry| {
        registry
            .get_mut(&channel)
            .map(|record| record.try_send(stored))
    });
    match result {
        Some(Ok(())) => ChannelSendStatus::Sent as u8,
        Some(Err(ChannelSendError::Full(value))) => {
            if release_values([value]) {
                ChannelSendStatus::Full as u8
            } else {
                ChannelSendStatus::Failure as u8
            }
        }
        Some(Err(ChannelSendError::Closed(value))) => {
            if release_values([value]) {
                ChannelSendStatus::Closed as u8
            } else {
                ChannelSendStatus::Failure as u8
            }
        }
        None => {
            let _ = release_values([stored]);
            ChannelSendStatus::Failure as u8
        }
    }
}

/// Receives one value without suspension.
///
/// # Safety
///
/// `output` must address one writable `u64` for the duration of this call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_channel_try_receive(channel: u64, output: *mut u64) -> u8 {
    if output.is_null() {
        return ChannelReceiveStatus::Failure as u8;
    }
    let result = channels().lock().ok().and_then(|mut registry| {
        registry
            .get_mut(&channel)
            .map(ChannelLifecycle::try_receive)
    });
    let Some(result) = result else {
        return ChannelReceiveStatus::Failure as u8;
    };
    let value = match result {
        ChannelReceive::Item(NativeChannelValue::Scalar(value)) => value,
        ChannelReceive::Item(NativeChannelValue::Managed(root)) => {
            let Ok(mut runtime) = lock_abi_runtime() else {
                return ChannelReceiveStatus::Failure as u8;
            };
            let Ok(reference) = runtime.resolve_root(root) else {
                return ChannelReceiveStatus::Failure as u8;
            };
            if runtime.release_root(root).is_err() {
                return ChannelReceiveStatus::Failure as u8;
            }
            reference.raw()
        }
        ChannelReceive::Empty => return ChannelReceiveStatus::Empty as u8,
        ChannelReceive::Closed => return ChannelReceiveStatus::Closed as u8,
    };
    // SAFETY: the caller provides one writable output slot.
    unsafe { output.write(value) };
    ChannelReceiveStatus::Item as u8
}
