//! Backend-neutral typed atomic ordering and scalar-state contract.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicLoadOrder {
    Relaxed,
    Acquire,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicStoreOrder {
    Relaxed,
    Release,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicReadModifyWriteOrder {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicCompareExchangeOrder {
    success: AtomicReadModifyWriteOrder,
    failure: AtomicLoadOrder,
}

impl AtomicCompareExchangeOrder {
    #[must_use]
    pub const fn new(
        success: AtomicReadModifyWriteOrder,
        failure: AtomicLoadOrder,
    ) -> Option<Self> {
        let valid = match success {
            AtomicReadModifyWriteOrder::Relaxed | AtomicReadModifyWriteOrder::Release => {
                matches!(failure, AtomicLoadOrder::Relaxed)
            }
            AtomicReadModifyWriteOrder::Acquire | AtomicReadModifyWriteOrder::AcquireRelease => {
                matches!(failure, AtomicLoadOrder::Relaxed | AtomicLoadOrder::Acquire)
            }
            AtomicReadModifyWriteOrder::SequentiallyConsistent => true,
        };
        if valid {
            Some(Self { success, failure })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn success(self) -> AtomicReadModifyWriteOrder {
        self.success
    }

    #[must_use]
    pub const fn failure(self) -> AtomicLoadOrder {
        self.failure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicCompareExchange<T> {
    previous: T,
    exchanged: bool,
}

impl<T: Copy> AtomicCompareExchange<T> {
    #[must_use]
    pub const fn previous(self) -> T {
        self.previous
    }

    #[must_use]
    pub const fn exchanged(self) -> bool {
        self.exchanged
    }
}

#[derive(Debug)]
pub struct AtomicInt {
    value: AtomicI64,
}

impl AtomicInt {
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self {
            value: AtomicI64::new(value),
        }
    }

    #[must_use]
    pub fn load(&self, order: AtomicLoadOrder) -> i64 {
        self.value.load(load_order(order))
    }

    pub fn store(&self, value: i64, order: AtomicStoreOrder) {
        self.value.store(value, store_order(order));
    }

    pub fn swap(&self, value: i64, order: AtomicReadModifyWriteOrder) -> i64 {
        self.value.swap(value, read_modify_write_order(order))
    }

    pub fn compare_exchange(
        &self,
        current: i64,
        new: i64,
        order: AtomicCompareExchangeOrder,
    ) -> AtomicCompareExchange<i64> {
        match self.value.compare_exchange(
            current,
            new,
            read_modify_write_order(order.success),
            load_order(order.failure),
        ) {
            Ok(previous) => AtomicCompareExchange {
                previous,
                exchanged: true,
            },
            Err(previous) => AtomicCompareExchange {
                previous,
                exchanged: false,
            },
        }
    }
}

#[derive(Debug)]
pub struct AtomicBoolean {
    value: AtomicBool,
}

impl AtomicBoolean {
    #[must_use]
    pub const fn new(value: bool) -> Self {
        Self {
            value: AtomicBool::new(value),
        }
    }

    #[must_use]
    pub fn load(&self, order: AtomicLoadOrder) -> bool {
        self.value.load(load_order(order))
    }

    pub fn store(&self, value: bool, order: AtomicStoreOrder) {
        self.value.store(value, store_order(order));
    }

    pub fn swap(&self, value: bool, order: AtomicReadModifyWriteOrder) -> bool {
        self.value.swap(value, read_modify_write_order(order))
    }

    pub fn compare_exchange(
        &self,
        current: bool,
        new: bool,
        order: AtomicCompareExchangeOrder,
    ) -> AtomicCompareExchange<bool> {
        match self.value.compare_exchange(
            current,
            new,
            read_modify_write_order(order.success),
            load_order(order.failure),
        ) {
            Ok(previous) => AtomicCompareExchange {
                previous,
                exchanged: true,
            },
            Err(previous) => AtomicCompareExchange {
                previous,
                exchanged: false,
            },
        }
    }
}

const fn load_order(order: AtomicLoadOrder) -> Ordering {
    match order {
        AtomicLoadOrder::Relaxed => Ordering::Relaxed,
        AtomicLoadOrder::Acquire => Ordering::Acquire,
        AtomicLoadOrder::SequentiallyConsistent => Ordering::SeqCst,
    }
}

const fn store_order(order: AtomicStoreOrder) -> Ordering {
    match order {
        AtomicStoreOrder::Relaxed => Ordering::Relaxed,
        AtomicStoreOrder::Release => Ordering::Release,
        AtomicStoreOrder::SequentiallyConsistent => Ordering::SeqCst,
    }
}

const fn read_modify_write_order(order: AtomicReadModifyWriteOrder) -> Ordering {
    match order {
        AtomicReadModifyWriteOrder::Relaxed => Ordering::Relaxed,
        AtomicReadModifyWriteOrder::Acquire => Ordering::Acquire,
        AtomicReadModifyWriteOrder::Release => Ordering::Release,
        AtomicReadModifyWriteOrder::AcquireRelease => Ordering::AcqRel,
        AtomicReadModifyWriteOrder::SequentiallyConsistent => Ordering::SeqCst,
    }
}
