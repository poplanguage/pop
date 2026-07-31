use std::sync::Arc;
use std::thread;

use pop_runtime_interface::{
    AtomicBoolean, AtomicCompareExchangeOrder, AtomicInt, AtomicLoadOrder,
    AtomicReadModifyWriteOrder, AtomicStoreOrder,
};

#[test]
fn typed_orders_reject_invalid_compare_exchange_failures() {
    use AtomicLoadOrder::{Acquire, Relaxed, SequentiallyConsistent};
    use AtomicReadModifyWriteOrder::{
        Acquire as ReadAcquire, AcquireRelease, Relaxed as ReadRelaxed, Release,
        SequentiallyConsistent as ReadSequentiallyConsistent,
    };

    for (success, failure) in [
        (ReadRelaxed, Relaxed),
        (ReadAcquire, Relaxed),
        (ReadAcquire, Acquire),
        (Release, Relaxed),
        (AcquireRelease, Acquire),
        (ReadSequentiallyConsistent, SequentiallyConsistent),
    ] {
        assert!(AtomicCompareExchangeOrder::new(success, failure).is_some());
    }
    for (success, failure) in [
        (ReadRelaxed, Acquire),
        (ReadRelaxed, SequentiallyConsistent),
        (Release, Acquire),
        (AcquireRelease, SequentiallyConsistent),
    ] {
        assert!(AtomicCompareExchangeOrder::new(success, failure).is_none());
    }
}

#[test]
fn integer_and_boolean_operations_return_exact_observed_values() {
    let order = AtomicCompareExchangeOrder::new(
        AtomicReadModifyWriteOrder::AcquireRelease,
        AtomicLoadOrder::Acquire,
    )
    .expect("valid order");
    let integer = AtomicInt::new(7);
    assert_eq!(integer.load(AtomicLoadOrder::Relaxed), 7);
    integer.store(11, AtomicStoreOrder::Release);
    assert_eq!(
        integer.swap(13, AtomicReadModifyWriteOrder::AcquireRelease),
        11
    );
    let success = integer.compare_exchange(13, 17, order);
    assert!(success.exchanged());
    assert_eq!(success.previous(), 13);
    let failure = integer.compare_exchange(13, 19, order);
    assert!(!failure.exchanged());
    assert_eq!(failure.previous(), 17);

    let boolean = AtomicBoolean::new(false);
    boolean.store(true, AtomicStoreOrder::Release);
    assert!(boolean.load(AtomicLoadOrder::Acquire));
    assert!(boolean.swap(false, AtomicReadModifyWriteOrder::SequentiallyConsistent));
}

#[test]
fn release_acquire_publication_is_visible_after_join() {
    let value = Arc::new(AtomicInt::new(0));
    let published = Arc::new(AtomicBoolean::new(false));
    let writer_value = Arc::clone(&value);
    let writer_published = Arc::clone(&published);
    thread::spawn(move || {
        writer_value.store(42, AtomicStoreOrder::Relaxed);
        writer_published.store(true, AtomicStoreOrder::Release);
    })
    .join()
    .expect("writer");

    assert!(published.load(AtomicLoadOrder::Acquire));
    assert_eq!(value.load(AtomicLoadOrder::Relaxed), 42);
}
