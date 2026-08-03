//! Native opaque handles for the bounded local-actor mailbox contract.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use pop_runtime_interface::{
    ActorExit, ActorId, ActorIncarnation, ActorLifecycle, ActorLifecycleError, ActorReceive,
    ActorReference, ActorSendError, ManagedReference, RootHandle, RuntimeAdapter,
};
use pop_runtime_native_abi::{ActorLifecycleStatus, ActorReceiveStatus, ActorSendStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeActorValue {
    Scalar(u64),
    Managed(RootHandle),
}

static NEXT_ACTOR: AtomicU64 = AtomicU64::new(1);

fn actors() -> &'static Mutex<BTreeMap<u64, ActorLifecycle<NativeActorValue>>> {
    static ACTORS: OnceLock<Mutex<BTreeMap<u64, ActorLifecycle<NativeActorValue>>>> =
        OnceLock::new();
    ACTORS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_actor() -> Option<u64> {
    NEXT_ACTOR
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|next| *next != 0)
        })
        .ok()
}

fn release_values(values: impl IntoIterator<Item = NativeActorValue>) -> bool {
    let roots: Vec<_> = values
        .into_iter()
        .filter_map(|value| match value {
            NativeActorValue::Scalar(_) => None,
            NativeActorValue::Managed(root) => Some(root),
        })
        .collect();
    if roots.is_empty() {
        return true;
    }
    let Ok(mut runtime) = crate::state::lock_abi_runtime() else {
        return false;
    };
    roots
        .into_iter()
        .all(|root| runtime.release_root(root).is_ok())
}

fn lifecycle_status(error: ActorLifecycleError) -> u8 {
    match error {
        ActorLifecycleError::AlreadyActive(_) => ActorLifecycleStatus::AlreadyActive as u8,
        ActorLifecycleError::AlreadyStopping(_) => ActorLifecycleStatus::AlreadyStopping as u8,
        ActorLifecycleError::AlreadyExited(_) => ActorLifecycleStatus::AlreadyExited as u8,
        ActorLifecycleError::NotStopping(_) => ActorLifecycleStatus::NotStopping as u8,
    }
}

const fn actor_exit(raw: u8) -> Option<ActorExit> {
    match raw {
        0 => Some(ActorExit::Completed),
        1 => Some(ActorExit::Cancelled),
        2 => Some(ActorExit::Panicked),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_create(actor: u64, incarnation: u64, capacity: u64) -> u64 {
    let Some(handle) = next_actor() else {
        return 0;
    };
    let Ok(mut values) = actors().lock() else {
        return 0;
    };
    values.insert(
        handle,
        ActorLifecycle::starting(
            ActorId::new(actor),
            ActorIncarnation::new(incarnation),
            capacity,
        ),
    );
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_activate(handle: u64) -> u8 {
    let Ok(mut values) = actors().lock() else {
        return ActorLifecycleStatus::Failure as u8;
    };
    let Some(actor) = values.get_mut(&handle) else {
        return ActorLifecycleStatus::Failure as u8;
    };
    actor
        .activate()
        .map_or_else(lifecycle_status, |()| ActorLifecycleStatus::Applied as u8)
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_try_send(
    handle: u64,
    actor: u64,
    incarnation: u64,
    value: u64,
    managed: u8,
) -> u8 {
    let stored = match managed {
        0 => NativeActorValue::Scalar(value),
        1 => {
            let Ok(mut runtime) = crate::state::lock_abi_runtime() else {
                return ActorSendStatus::Failure as u8;
            };
            let Ok(root) = runtime.retain_root(ManagedReference::new(value)) else {
                return ActorSendStatus::Failure as u8;
            };
            NativeActorValue::Managed(root)
        }
        _ => return ActorSendStatus::Failure as u8,
    };
    let reference = ActorReference::new(ActorId::new(actor), ActorIncarnation::new(incarnation));
    let result = actors().lock().ok().and_then(|mut values| {
        values
            .get_mut(&handle)
            .map(|record| record.try_admit(reference, stored))
    });
    match result {
        Some(Ok(())) => ActorSendStatus::Sent as u8,
        Some(Err(ActorSendError::Full(value))) => {
            if release_values([value]) {
                ActorSendStatus::Full as u8
            } else {
                ActorSendStatus::Failure as u8
            }
        }
        Some(Err(ActorSendError::Closed(value))) => {
            if release_values([value]) {
                ActorSendStatus::Closed as u8
            } else {
                ActorSendStatus::Failure as u8
            }
        }
        Some(Err(ActorSendError::Stale(value))) => {
            if release_values([value]) {
                ActorSendStatus::Stale as u8
            } else {
                ActorSendStatus::Failure as u8
            }
        }
        None => {
            let _ = release_values([stored]);
            ActorSendStatus::Failure as u8
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_try_send_handle(handle: u64, value: u64, managed: u8) -> u8 {
    let reference = actors()
        .lock()
        .ok()
        .and_then(|values| values.get(&handle).map(ActorLifecycle::reference));
    let Some(reference) = reference else {
        return ActorSendStatus::Failure as u8;
    };
    pop_rt_actor_try_send(
        handle,
        reference.actor().raw(),
        reference.incarnation().raw(),
        value,
        managed,
    )
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pop_rt_actor_try_receive(
    handle: u64,
    output: *mut u64,
    managed: *mut u8,
) -> u8 {
    if output.is_null() || managed.is_null() {
        return ActorReceiveStatus::Failure as u8;
    }
    let result = actors()
        .lock()
        .ok()
        .and_then(|mut values| values.get_mut(&handle).map(ActorLifecycle::try_receive));
    let Some(result) = result else {
        return ActorReceiveStatus::Failure as u8;
    };
    let (value, is_managed) = match result {
        ActorReceive::Message(NativeActorValue::Scalar(value)) => (value, 0),
        ActorReceive::Message(NativeActorValue::Managed(root)) => {
            let Ok(mut runtime) = crate::state::lock_abi_runtime() else {
                return ActorReceiveStatus::Failure as u8;
            };
            let Ok(reference) = runtime.resolve_root(root) else {
                return ActorReceiveStatus::Failure as u8;
            };
            if runtime.release_root(root).is_err() {
                return ActorReceiveStatus::Failure as u8;
            }
            (reference.raw(), 1)
        }
        ActorReceive::Empty => return ActorReceiveStatus::Empty as u8,
        ActorReceive::Closed => return ActorReceiveStatus::Closed as u8,
    };
    // SAFETY: callers provide two writable scalar output slots.
    unsafe {
        output.write(value);
        managed.write(is_managed);
    }
    ActorReceiveStatus::Item as u8
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_begin_exit(handle: u64, exit: u8) -> u8 {
    let Some(exit) = actor_exit(exit) else {
        return ActorLifecycleStatus::Failure as u8;
    };
    let drained = {
        let Ok(mut values) = actors().lock() else {
            return ActorLifecycleStatus::Failure as u8;
        };
        let Some(actor) = values.get_mut(&handle) else {
            return ActorLifecycleStatus::Failure as u8;
        };
        match actor.begin_exit(exit) {
            Ok(values) => values,
            Err(error) => return lifecycle_status(error),
        }
    };
    if release_values(drained) {
        ActorLifecycleStatus::Applied as u8
    } else {
        ActorLifecycleStatus::Failure as u8
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_complete_exit(handle: u64) -> u8 {
    let Ok(mut values) = actors().lock() else {
        return ActorLifecycleStatus::Failure as u8;
    };
    let Some(actor) = values.get_mut(&handle) else {
        return ActorLifecycleStatus::Failure as u8;
    };
    actor
        .complete_exit()
        .map_or_else(lifecycle_status, |_: ActorExit| {
            ActorLifecycleStatus::Applied as u8
        })
}

#[unsafe(no_mangle)]
pub extern "C" fn pop_rt_actor_release(handle: u64) -> u8 {
    let drained = {
        let Ok(mut values) = actors().lock() else {
            return 0;
        };
        let Some(mut actor) = values.remove(&handle) else {
            return 0;
        };
        actor.begin_exit(ActorExit::Cancelled).unwrap_or_default()
    };
    u8::from(release_values(drained))
}
