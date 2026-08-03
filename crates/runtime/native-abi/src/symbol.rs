use pop_runtime_interface::RuntimeOperation;

/// ABI 1.11 lookup adapter that separates presence from a scalar payload.
pub const TABLE_GET_CHECKED_SYMBOL: &str = "pop_rt_table_get_checked";

/// ABI 1.21 verified initialized-object/managed-array-store adapter.
pub const ALLOCATE_INITIALIZED_OBJECT_AT_SITE_AND_STORE_ARRAY_SYMBOL: &str =
    "pop_rt_allocate_initialized_object_at_site_and_store_array";

/// ABI 1.21 verified checked-array-read/static-field-read adapter.
pub const ARRAY_GET_OBJECT_FIELD_CHECKED_SYMBOL: &str = "pop_rt_array_get_object_field_checked";

/// ABI 1.22 atomic initialized allocation for compiler-known self-reference
/// slots.
pub const ALLOCATE_INITIALIZED_SELF_REFERENTIAL_OBJECT_AT_SITE_SYMBOL: &str =
    "pop_rt_allocate_initialized_self_referential_object_at_site";

/// ABI 1.22 closed atomic constructor for one native iterator step.
pub const ITERATION_MAKE_SYMBOL: &str = "pop_rt_iteration_make";

/// ABI 1.23/2.1 validated Unicode-scalar read from a compiler-proven Text view.
pub const TEXT_VIEW_GET_RUNE_SYMBOL: &str = "pop_rt_text_view_get_rune";

pub const ACTOR_CREATE_SYMBOL: &str = "pop_rt_actor_create";
pub const ACTOR_ACTIVATE_SYMBOL: &str = "pop_rt_actor_activate";
pub const ACTOR_TRY_SEND_SYMBOL: &str = "pop_rt_actor_try_send";
pub const ACTOR_TRY_RECEIVE_SYMBOL: &str = "pop_rt_actor_try_receive";
pub const ACTOR_BEGIN_EXIT_SYMBOL: &str = "pop_rt_actor_begin_exit";
pub const ACTOR_COMPLETE_EXIT_SYMBOL: &str = "pop_rt_actor_complete_exit";
pub const ACTOR_RELEASE_SYMBOL: &str = "pop_rt_actor_release";

pub const ATOMIC_INT_CREATE_SYMBOL: &str = "pop_rt_atomic_int_create";
pub const ATOMIC_INT_LOAD_SYMBOL: &str = "pop_rt_atomic_int_load";
pub const ATOMIC_INT_STORE_SYMBOL: &str = "pop_rt_atomic_int_store";
pub const ATOMIC_INT_SWAP_SYMBOL: &str = "pop_rt_atomic_int_swap";
pub const ATOMIC_INT_COMPARE_EXCHANGE_SYMBOL: &str = "pop_rt_atomic_int_compare_exchange";
pub const ATOMIC_BOOL_CREATE_SYMBOL: &str = "pop_rt_atomic_bool_create";
pub const ATOMIC_BOOL_LOAD_SYMBOL: &str = "pop_rt_atomic_bool_load";
pub const ATOMIC_BOOL_STORE_SYMBOL: &str = "pop_rt_atomic_bool_store";
pub const ATOMIC_BOOL_SWAP_SYMBOL: &str = "pop_rt_atomic_bool_swap";
pub const ATOMIC_BOOL_COMPARE_EXCHANGE_SYMBOL: &str = "pop_rt_atomic_bool_compare_exchange";
pub const ATOMIC_RELEASE_SYMBOL: &str = "pop_rt_atomic_release";

/// Returns the native C symbol for an operation implemented through ABI 1.27.
///
/// Operations outside the native bootstrap capability set fail closed. MIR and
/// alternate runtime implementations continue to use the semantic operation.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive operation-to-symbol contract is clearest as one closed match"
)]
pub const fn symbol(operation: RuntimeOperation) -> Option<&'static str> {
    match operation {
        RuntimeOperation::AllocateObject => Some("pop_rt_allocate_object"),
        RuntimeOperation::AllocateObjectInitialized => Some("pop_rt_allocate_initialized_object"),
        RuntimeOperation::AllocateObjectInitializedAtSite => {
            Some("pop_rt_allocate_initialized_object_at_site")
        }
        RuntimeOperation::AllocateArray => Some("pop_rt_allocate_array"),
        RuntimeOperation::AllocateArrayFilled => Some("pop_rt_allocate_array_filled"),
        RuntimeOperation::AllocateTable => Some("pop_rt_allocate_table"),
        RuntimeOperation::TupleMake => Some("pop_rt_tuple_make"),
        RuntimeOperation::TableGet => Some("pop_rt_table_get"),
        RuntimeOperation::TableSet => Some("pop_rt_table_set"),
        RuntimeOperation::ArrayGet => Some("pop_rt_array_get"),
        RuntimeOperation::ArrayLength => Some("pop_rt_array_length"),
        RuntimeOperation::ArrayGetChecked => Some("pop_rt_array_get_checked"),
        RuntimeOperation::ArraySet => Some("pop_rt_array_set"),
        RuntimeOperation::ArrayFill => Some("pop_rt_array_fill"),
        RuntimeOperation::ListCreate => Some("pop_rt_list_create"),
        RuntimeOperation::ListLength => Some("pop_rt_list_length"),
        RuntimeOperation::ListGet => Some("pop_rt_list_get"),
        RuntimeOperation::ListGetChecked => Some("pop_rt_list_get_checked"),
        RuntimeOperation::ListSet => Some("pop_rt_list_set"),
        RuntimeOperation::ListAdd => Some("pop_rt_list_add"),
        RuntimeOperation::ByteBufferCreate => Some("pop_rt_byte_buffer_create"),
        RuntimeOperation::ByteBufferLength => Some("pop_rt_byte_buffer_length"),
        RuntimeOperation::ByteBufferReserve => Some("pop_rt_byte_buffer_reserve"),
        RuntimeOperation::ByteBufferClear => Some("pop_rt_byte_buffer_clear"),
        RuntimeOperation::ByteBufferWriteByte => Some("pop_rt_byte_buffer_write_byte"),
        RuntimeOperation::ByteBufferWriteBytes => Some("pop_rt_byte_buffer_write_bytes"),
        RuntimeOperation::ByteBufferWriteView => Some("pop_rt_byte_buffer_write_view"),
        RuntimeOperation::ByteBufferWriteInteger => Some("pop_rt_byte_buffer_write_integer"),
        RuntimeOperation::ByteBufferMaterialize => Some("pop_rt_byte_buffer_materialize"),
        RuntimeOperation::Utf8Encode => Some("pop_rt_text_view_encode_utf8"),
        RuntimeOperation::Utf8DecodeView => Some("pop_rt_bytes_view_decode_utf8"),
        RuntimeOperation::Utf8DecodeBuffer => Some("pop_rt_byte_buffer_decode_utf8"),
        RuntimeOperation::RangeCreate => Some("pop_rt_range_create"),
        RuntimeOperation::IterationAcquire => Some("pop_rt_iteration_acquire"),
        RuntimeOperation::IterationNext => Some("pop_rt_iteration_next"),
        RuntimeOperation::FieldGet => Some("pop_rt_field_get"),
        RuntimeOperation::FieldSet => Some("pop_rt_field_set"),
        RuntimeOperation::StringConcat => Some("pop_rt_string_concat"),
        RuntimeOperation::StringFormat => Some("pop_rt_string_format"),
        RuntimeOperation::FfiBufferOpen => Some("pop_rt_ffi_buffer_open"),
        RuntimeOperation::FfiBufferLength => Some("pop_rt_ffi_buffer_length"),
        RuntimeOperation::FfiBufferRead => Some("pop_rt_ffi_buffer_read"),
        RuntimeOperation::FfiBufferWrite => Some("pop_rt_ffi_buffer_write"),
        RuntimeOperation::FfiBufferBorrow => Some("pop_rt_ffi_buffer_borrow"),
        RuntimeOperation::FfiBufferEndBorrow => Some("pop_rt_ffi_buffer_end_borrow"),
        RuntimeOperation::FfiBufferClose => Some("pop_rt_ffi_buffer_close"),
        RuntimeOperation::FfiBytesBorrow => Some("pop_rt_ffi_bytes_borrow"),
        RuntimeOperation::FfiBytesEndBorrow => Some("pop_rt_ffi_bytes_end_borrow"),
        RuntimeOperation::FfiCallbackOpen => Some("pop_rt_ffi_callback_open"),
        RuntimeOperation::FfiCallbackEnter => Some("pop_rt_ffi_callback_enter"),
        RuntimeOperation::FfiCallbackLeave => Some("pop_rt_ffi_callback_leave"),
        RuntimeOperation::FfiCallbackClose => Some("pop_rt_ffi_callback_close"),
        RuntimeOperation::CodecWriteEvent => Some("pop_rt_codec_write_event"),
        RuntimeOperation::CodecReadEvent => Some("pop_rt_codec_read_event"),
        RuntimeOperation::RetainRoot => Some("pop_rt_retain_root"),
        RuntimeOperation::ResolveRoot => Some("pop_rt_resolve_root"),
        RuntimeOperation::ReleaseRoot => Some("pop_rt_release_root"),
        RuntimeOperation::Pin => Some("pop_rt_pin"),
        RuntimeOperation::Unpin => Some("pop_rt_unpin"),
        RuntimeOperation::AttachManagedThread => Some("pop_rt_attach_managed_thread"),
        RuntimeOperation::DetachManagedThread => Some("pop_rt_detach_managed_thread"),
        RuntimeOperation::EnterForeign => Some("pop_rt_enter_foreign"),
        RuntimeOperation::LeaveForeign => Some("pop_rt_leave_foreign"),
        RuntimeOperation::GcSafePoint => Some("pop_rt_gc_safe_point"),
        RuntimeOperation::SatbWriteBarrier => Some("pop_rt_satb_write_barrier"),
        RuntimeOperation::Trap => Some("pop_rt_trap"),
        RuntimeOperation::ContinueUnwind => Some("pop_rt_continue_unwind"),
        RuntimeOperation::CancelSourceCreate => Some("pop_rt_cancel_source_create"),
        RuntimeOperation::CancelSourceToken => Some("pop_rt_cancel_source_token"),
        RuntimeOperation::CancelSourceRelease => Some("pop_rt_cancel_source_release"),
        RuntimeOperation::CancelTokenRelease => Some("pop_rt_cancel_token_release"),
        RuntimeOperation::TaskFrameCreate => Some("pop_rt_task_frame_create"),
        RuntimeOperation::TaskFrameRelease => Some("pop_rt_task_frame_release"),
        RuntimeOperation::TaskFrameLoad => Some("pop_rt_task_frame_load"),
        RuntimeOperation::TaskFrameStore => Some("pop_rt_task_frame_store"),
        RuntimeOperation::TaskFrameSetLiveMap => Some("pop_rt_task_frame_set_live_map"),
        RuntimeOperation::TaskCreate => Some("pop_rt_task_create"),
        RuntimeOperation::TaskStartDirect => Some("pop_rt_task_start_direct"),
        RuntimeOperation::TaskStartGroup => Some("pop_rt_task_start_group"),
        RuntimeOperation::TaskAwait => Some("pop_rt_task_await"),
        RuntimeOperation::TaskCompletionStore => Some("pop_rt_task_completion_store"),
        RuntimeOperation::TaskRelease => Some("pop_rt_task_release"),
        RuntimeOperation::TaskGroupCreate => Some("pop_rt_task_group_create"),
        RuntimeOperation::TaskGroupWrap => Some("pop_rt_task_group_wrap"),
        RuntimeOperation::TaskGroupClose => Some("pop_rt_task_group_close"),
        RuntimeOperation::TaskGroupJoin => Some("pop_rt_task_group_join"),
        RuntimeOperation::Suspend => Some("pop_rt_suspend"),
        RuntimeOperation::Resume => Some("pop_rt_resume"),
        RuntimeOperation::TaskCancel => Some("pop_rt_task_cancel"),
        RuntimeOperation::TaskCancellationRequested => Some("pop_rt_task_cancellation_requested"),
        RuntimeOperation::ChannelCreate => Some("pop_rt_channel_create"),
        RuntimeOperation::ChannelRetainSender => Some("pop_rt_channel_retain_sender"),
        RuntimeOperation::ChannelReleaseSender => Some("pop_rt_channel_release_sender"),
        RuntimeOperation::ChannelRetainReceiver => Some("pop_rt_channel_retain_receiver"),
        RuntimeOperation::ChannelReleaseReceiver => Some("pop_rt_channel_release_receiver"),
        RuntimeOperation::ChannelClose => Some("pop_rt_channel_close"),
        RuntimeOperation::ChannelTrySend => Some("pop_rt_channel_try_send"),
        RuntimeOperation::ChannelTryReceive => Some("pop_rt_channel_try_receive"),
        RuntimeOperation::RecordUpdate
        | RuntimeOperation::UnionMake
        | RuntimeOperation::CaptureLoad
        | RuntimeOperation::CaptureStore
        | RuntimeOperation::DispatchCall
        | RuntimeOperation::PublishRoots
        | RuntimeOperation::GenerationalWriteBarrier
        | RuntimeOperation::Panic
        | RuntimeOperation::InitializeModule
        | RuntimeOperation::InitializeBubble => None,
    }
}
