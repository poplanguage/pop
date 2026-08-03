use std::collections::BTreeSet;

use pop_runtime_interface::RuntimeOperation;
use pop_runtime_native_abi::{
    ABI_SUPPORT_SYMBOL, ACTOR_ACTIVATE_SYMBOL, ACTOR_BEGIN_EXIT_SYMBOL, ACTOR_COMPLETE_EXIT_SYMBOL,
    ACTOR_CREATE_SYMBOL, ACTOR_RELEASE_SYMBOL, ACTOR_TRY_RECEIVE_SYMBOL, ACTOR_TRY_SEND_SYMBOL,
    ALLOCATE_INITIALIZED_OBJECT_AT_SITE_AND_STORE_ARRAY_SYMBOL,
    ALLOCATE_INITIALIZED_SELF_REFERENTIAL_OBJECT_AT_SITE_SYMBOL,
    ARRAY_GET_OBJECT_FIELD_CHECKED_SYMBOL, ATOMIC_BOOL_COMPARE_EXCHANGE_SYMBOL,
    ATOMIC_BOOL_CREATE_SYMBOL, ATOMIC_BOOL_LOAD_SYMBOL, ATOMIC_BOOL_STORE_SYMBOL,
    ATOMIC_BOOL_SWAP_SYMBOL, ATOMIC_INT_COMPARE_EXCHANGE_SYMBOL, ATOMIC_INT_CREATE_SYMBOL,
    ATOMIC_INT_LOAD_SYMBOL, ATOMIC_INT_STORE_SYMBOL, ATOMIC_INT_SWAP_SYMBOL, ATOMIC_RELEASE_SYMBOL,
    ActorLifecycleStatus, ActorReceiveStatus, ActorSendStatus, AllocationSiteDescriptorAbi,
    ChannelReceiveStatus, ChannelSendStatus, CodecEventStatus, CodecEventTag, CodecReadEventAbi,
    CodecWriteEventAbi, GC_SAFE_POINT_V2_SYMBOL, INVALID_HANDLE, ITERATION_MAKE_SYMBOL,
    IterationCollectionKind, NATIVE_ABI_1_VERSION, NATIVE_ABI_2_VERSION, TCP_ACCEPT_SYMBOL,
    TCP_CLOSE_SYMBOL, TCP_CONNECT_SYMBOL, TCP_LISTEN_SYMBOL, TCP_LOCAL_PORT_SYMBOL,
    TCP_RECEIVE_SYMBOL, TCP_SEND_SYMBOL, TEXT_VIEW_GET_RUNE_SYMBOL, TextViewGetRuneAbi, symbol,
};

#[test]
fn abi_version_and_invalid_handle_are_explicit() {
    assert_eq!(NATIVE_ABI_1_VERSION.major(), 1);
    assert_eq!(NATIVE_ABI_1_VERSION.minor(), 27);
    assert_eq!(NATIVE_ABI_2_VERSION.major(), 2);
    assert_eq!(NATIVE_ABI_2_VERSION.minor(), 5);
    assert_ne!(NATIVE_ABI_1_VERSION, NATIVE_ABI_2_VERSION);
    assert_eq!(ABI_SUPPORT_SYMBOL, "pop_rt_supports_abi");
    assert_eq!(GC_SAFE_POINT_V2_SYMBOL, "pop_rt_gc_safe_point_v2");
    assert_eq!(INVALID_HANDLE, 0);
}

#[test]
fn atomic_symbols_are_typed_and_closed() {
    let symbols = [
        ATOMIC_INT_CREATE_SYMBOL,
        ATOMIC_INT_LOAD_SYMBOL,
        ATOMIC_INT_STORE_SYMBOL,
        ATOMIC_INT_SWAP_SYMBOL,
        ATOMIC_INT_COMPARE_EXCHANGE_SYMBOL,
        ATOMIC_BOOL_CREATE_SYMBOL,
        ATOMIC_BOOL_LOAD_SYMBOL,
        ATOMIC_BOOL_STORE_SYMBOL,
        ATOMIC_BOOL_SWAP_SYMBOL,
        ATOMIC_BOOL_COMPARE_EXCHANGE_SYMBOL,
        ATOMIC_RELEASE_SYMBOL,
    ];
    assert_eq!(symbols.len(), symbols.iter().collect::<BTreeSet<_>>().len());
    assert!(
        symbols
            .iter()
            .all(|symbol| symbol.starts_with("pop_rt_atomic_"))
    );
}

#[test]
fn actor_symbols_and_statuses_are_closed() {
    let symbols = [
        ACTOR_CREATE_SYMBOL,
        ACTOR_ACTIVATE_SYMBOL,
        ACTOR_TRY_SEND_SYMBOL,
        ACTOR_TRY_RECEIVE_SYMBOL,
        ACTOR_BEGIN_EXIT_SYMBOL,
        ACTOR_COMPLETE_EXIT_SYMBOL,
        ACTOR_RELEASE_SYMBOL,
    ];
    assert_eq!(symbols.len(), symbols.iter().collect::<BTreeSet<_>>().len());
    assert!(
        symbols
            .iter()
            .all(|symbol| symbol.starts_with("pop_rt_actor_"))
    );
    assert_eq!(ActorSendStatus::from_raw(4), Some(ActorSendStatus::Stale));
    assert_eq!(ActorSendStatus::from_raw(5), None);
    assert_eq!(
        ActorReceiveStatus::from_raw(3),
        Some(ActorReceiveStatus::Closed)
    );
    assert_eq!(
        ActorLifecycleStatus::from_raw(5),
        Some(ActorLifecycleStatus::NotStopping)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the complete closed ABI symbol inventory is intentionally one uniqueness assertion"
)]
fn supported_symbols_are_unique_and_native() {
    let operations = [
        RuntimeOperation::AllocateObject,
        RuntimeOperation::AllocateObjectInitialized,
        RuntimeOperation::AllocateObjectInitializedAtSite,
        RuntimeOperation::AllocateArray,
        RuntimeOperation::AllocateArrayFilled,
        RuntimeOperation::AllocateTable,
        RuntimeOperation::TupleMake,
        RuntimeOperation::TableGet,
        RuntimeOperation::TableSet,
        RuntimeOperation::ArrayGet,
        RuntimeOperation::ArrayLength,
        RuntimeOperation::ArrayGetChecked,
        RuntimeOperation::ArraySet,
        RuntimeOperation::ArrayFill,
        RuntimeOperation::ListCreate,
        RuntimeOperation::ListLength,
        RuntimeOperation::ListGet,
        RuntimeOperation::ListGetChecked,
        RuntimeOperation::ListSet,
        RuntimeOperation::ListAdd,
        RuntimeOperation::ByteBufferCreate,
        RuntimeOperation::ByteBufferLength,
        RuntimeOperation::ByteBufferReserve,
        RuntimeOperation::ByteBufferClear,
        RuntimeOperation::ByteBufferWriteByte,
        RuntimeOperation::ByteBufferWriteBytes,
        RuntimeOperation::ByteBufferWriteView,
        RuntimeOperation::ByteBufferWriteInteger,
        RuntimeOperation::ByteBufferMaterialize,
        RuntimeOperation::Utf8Encode,
        RuntimeOperation::Utf8DecodeView,
        RuntimeOperation::Utf8DecodeBuffer,
        RuntimeOperation::RangeCreate,
        RuntimeOperation::IterationAcquire,
        RuntimeOperation::IterationNext,
        RuntimeOperation::FieldGet,
        RuntimeOperation::FieldSet,
        RuntimeOperation::StringConcat,
        RuntimeOperation::StringFormat,
        RuntimeOperation::FfiBufferOpen,
        RuntimeOperation::FfiBufferLength,
        RuntimeOperation::FfiBufferRead,
        RuntimeOperation::FfiBufferWrite,
        RuntimeOperation::FfiBufferBorrow,
        RuntimeOperation::FfiBufferEndBorrow,
        RuntimeOperation::FfiBufferClose,
        RuntimeOperation::FfiBytesBorrow,
        RuntimeOperation::FfiBytesEndBorrow,
        RuntimeOperation::FfiCallbackOpen,
        RuntimeOperation::FfiCallbackEnter,
        RuntimeOperation::FfiCallbackLeave,
        RuntimeOperation::FfiCallbackClose,
        RuntimeOperation::RetainRoot,
        RuntimeOperation::ResolveRoot,
        RuntimeOperation::ReleaseRoot,
        RuntimeOperation::Pin,
        RuntimeOperation::Unpin,
        RuntimeOperation::AttachManagedThread,
        RuntimeOperation::DetachManagedThread,
        RuntimeOperation::EnterForeign,
        RuntimeOperation::LeaveForeign,
        RuntimeOperation::GcSafePoint,
        RuntimeOperation::SatbWriteBarrier,
        RuntimeOperation::Trap,
        RuntimeOperation::ContinueUnwind,
        RuntimeOperation::CancelSourceCreate,
        RuntimeOperation::CancelSourceToken,
        RuntimeOperation::CancelSourceRelease,
        RuntimeOperation::CancelTokenRelease,
        RuntimeOperation::TaskFrameCreate,
        RuntimeOperation::TaskFrameRelease,
        RuntimeOperation::TaskFrameLoad,
        RuntimeOperation::TaskFrameStore,
        RuntimeOperation::TaskFrameSetLiveMap,
        RuntimeOperation::TaskCreate,
        RuntimeOperation::TaskStartDirect,
        RuntimeOperation::TaskStartGroup,
        RuntimeOperation::TaskAwait,
        RuntimeOperation::TaskCompletionStore,
        RuntimeOperation::TaskRelease,
        RuntimeOperation::TaskGroupCreate,
        RuntimeOperation::TaskGroupWrap,
        RuntimeOperation::TaskGroupClose,
        RuntimeOperation::TaskGroupJoin,
        RuntimeOperation::Suspend,
        RuntimeOperation::Resume,
        RuntimeOperation::TaskCancel,
        RuntimeOperation::TaskCancellationRequested,
        RuntimeOperation::ChannelCreate,
        RuntimeOperation::ChannelRetainSender,
        RuntimeOperation::ChannelReleaseSender,
        RuntimeOperation::ChannelRetainReceiver,
        RuntimeOperation::ChannelReleaseReceiver,
        RuntimeOperation::ChannelClose,
        RuntimeOperation::ChannelTrySend,
        RuntimeOperation::ChannelTryReceive,
        RuntimeOperation::AtomicIntCreate,
        RuntimeOperation::AtomicIntLoad,
        RuntimeOperation::AtomicIntStore,
        RuntimeOperation::AtomicIntSwap,
        RuntimeOperation::AtomicIntCompareExchange,
        RuntimeOperation::AtomicBoolCreate,
        RuntimeOperation::AtomicBoolLoad,
        RuntimeOperation::AtomicBoolStore,
        RuntimeOperation::AtomicBoolSwap,
        RuntimeOperation::AtomicBoolCompareExchange,
        RuntimeOperation::AtomicRelease,
        RuntimeOperation::ActorCreate,
        RuntimeOperation::ActorActivate,
        RuntimeOperation::ActorTrySend,
        RuntimeOperation::ActorTryReceive,
        RuntimeOperation::ActorBeginExit,
        RuntimeOperation::ActorCompleteExit,
        RuntimeOperation::ActorRelease,
        RuntimeOperation::TcpListen,
        RuntimeOperation::TcpLocalPort,
        RuntimeOperation::TcpConnect,
        RuntimeOperation::TcpAccept,
        RuntimeOperation::TcpSend,
        RuntimeOperation::TcpReceive,
        RuntimeOperation::TcpClose,
        RuntimeOperation::CodecWriteEvent,
        RuntimeOperation::CodecReadEvent,
    ];
    let symbols: BTreeSet<_> = operations
        .into_iter()
        .map(|operation| symbol(operation).expect("supported native operation"))
        .collect();
    assert_eq!(symbols.len(), operations.len());
    assert!(symbols.iter().all(|name| name.starts_with("pop_rt_")));
}

#[test]
fn tcp_symbols_are_exact_and_closed() {
    let symbols = [
        TCP_LISTEN_SYMBOL,
        TCP_LOCAL_PORT_SYMBOL,
        TCP_CONNECT_SYMBOL,
        TCP_ACCEPT_SYMBOL,
        TCP_SEND_SYMBOL,
        TCP_RECEIVE_SYMBOL,
        TCP_CLOSE_SYMBOL,
    ];
    assert_eq!(symbols.len(), symbols.iter().collect::<BTreeSet<_>>().len());
    assert!(
        symbols
            .iter()
            .all(|symbol| symbol.starts_with("pop_rt_tcp_"))
    );
}

#[test]
fn codec_event_abi_has_closed_widths_and_statuses() {
    assert!(std::mem::size_of::<CodecWriteEventAbi>() > 0);
    assert!(std::mem::size_of::<CodecReadEventAbi>() > 0);
    assert_eq!(CodecEventStatus::from_raw(0), Some(CodecEventStatus::Ok));
    assert_eq!(
        CodecEventStatus::from_raw(3),
        Some(CodecEventStatus::CapabilityFailure)
    );
    assert_eq!(CodecEventStatus::from_raw(4), None);
    assert_eq!(CodecEventTag::from_raw(0), Some(CodecEventTag::RecordStart));
    assert_eq!(CodecEventTag::from_raw(26), Some(CodecEventTag::Bytes));
    assert_eq!(CodecEventTag::from_raw(27), None);
    assert_eq!(NATIVE_ABI_1_VERSION.minor(), 27);
}

#[test]
fn channel_abi_has_closed_send_and_receive_statuses() {
    assert_eq!(
        ChannelSendStatus::from_raw(0),
        Some(ChannelSendStatus::Failure)
    );
    assert_eq!(
        ChannelSendStatus::from_raw(1),
        Some(ChannelSendStatus::Sent)
    );
    assert_eq!(
        ChannelSendStatus::from_raw(2),
        Some(ChannelSendStatus::Full)
    );
    assert_eq!(
        ChannelSendStatus::from_raw(3),
        Some(ChannelSendStatus::Closed)
    );
    assert_eq!(ChannelSendStatus::from_raw(4), None);
    assert_eq!(
        ChannelReceiveStatus::from_raw(0),
        Some(ChannelReceiveStatus::Failure)
    );
    assert_eq!(
        ChannelReceiveStatus::from_raw(1),
        Some(ChannelReceiveStatus::Item)
    );
    assert_eq!(
        ChannelReceiveStatus::from_raw(2),
        Some(ChannelReceiveStatus::Empty)
    );
    assert_eq!(
        ChannelReceiveStatus::from_raw(3),
        Some(ChannelReceiveStatus::Closed)
    );
    assert_eq!(ChannelReceiveStatus::from_raw(4), None);
}

#[test]
fn abi_one_twenty_four_appends_the_exact_string_iteration_kind() {
    assert_eq!(IterationCollectionKind::Array as u8, 0);
    assert_eq!(IterationCollectionKind::Table as u8, 1);
    assert_eq!(IterationCollectionKind::List as u8, 2);
    assert_eq!(IterationCollectionKind::Range as u8, 3);
    assert_eq!(IterationCollectionKind::String as u8, 4);
}

#[test]
fn allocation_site_descriptor_abi_is_fixed_width_and_has_one_symbol() {
    assert!(std::mem::size_of::<AllocationSiteDescriptorAbi>() > 0);
    assert_eq!(
        symbol(RuntimeOperation::AllocateObjectInitializedAtSite),
        Some("pop_rt_allocate_initialized_object_at_site")
    );
}

#[test]
fn adjacent_heap_fusion_symbols_are_exact_and_distinct() {
    assert_eq!(
        ALLOCATE_INITIALIZED_OBJECT_AT_SITE_AND_STORE_ARRAY_SYMBOL,
        "pop_rt_allocate_initialized_object_at_site_and_store_array"
    );
    assert_eq!(
        ARRAY_GET_OBJECT_FIELD_CHECKED_SYMBOL,
        "pop_rt_array_get_object_field_checked"
    );
    assert_ne!(
        ALLOCATE_INITIALIZED_OBJECT_AT_SITE_AND_STORE_ARRAY_SYMBOL,
        ARRAY_GET_OBJECT_FIELD_CHECKED_SYMBOL
    );
}

#[test]
fn abi_one_twenty_two_layout_symbols_are_exact_and_distinct() {
    assert_eq!(
        ALLOCATE_INITIALIZED_SELF_REFERENTIAL_OBJECT_AT_SITE_SYMBOL,
        "pop_rt_allocate_initialized_self_referential_object_at_site"
    );
    assert_eq!(ITERATION_MAKE_SYMBOL, "pop_rt_iteration_make");
    assert_ne!(
        ALLOCATE_INITIALIZED_SELF_REFERENTIAL_OBJECT_AT_SITE_SYMBOL,
        ITERATION_MAKE_SYMBOL
    );
}

#[test]
fn abi_one_twenty_three_unicode_scalar_symbol_is_exact() {
    assert_eq!(TEXT_VIEW_GET_RUNE_SYMBOL, "pop_rt_text_view_get_rune");
    assert!(std::mem::size_of::<TextViewGetRuneAbi>() > 0);
}

#[test]
fn unsupported_operations_have_no_fallback_symbol() {
    assert_eq!(symbol(RuntimeOperation::DispatchCall), None);
    assert_eq!(symbol(RuntimeOperation::InitializeBubble), None);
}
