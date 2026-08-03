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
pub const ACTOR_TRY_SEND_HANDLE_SYMBOL: &str = "pop_rt_actor_try_send_handle";
pub const ACTOR_TRY_RECEIVE_SYMBOL: &str = "pop_rt_actor_try_receive";
pub const ACTOR_BEGIN_EXIT_SYMBOL: &str = "pop_rt_actor_begin_exit";
pub const ACTOR_COMPLETE_EXIT_SYMBOL: &str = "pop_rt_actor_complete_exit";
pub const ACTOR_RELEASE_SYMBOL: &str = "pop_rt_actor_release";

pub const TCP_LISTEN_SYMBOL: &str = "pop_rt_tcp_listen";
pub const TCP_LISTEN_IPV4_SYMBOL: &str = "pop_rt_tcp_listen_ipv4";
pub const TCP_LISTEN_IPV6_SYMBOL: &str = "pop_rt_tcp_listen_ipv6";
pub const TCP_LOCAL_PORT_SYMBOL: &str = "pop_rt_tcp_local_port";
pub const TCP_CONNECT_SYMBOL: &str = "pop_rt_tcp_connect";
pub const TCP_CONNECT_IPV4_SYMBOL: &str = "pop_rt_tcp_connect_ipv4";
pub const TCP_CONNECT_IPV6_SYMBOL: &str = "pop_rt_tcp_connect_ipv6";
pub const TCP_ACCEPT_SYMBOL: &str = "pop_rt_tcp_accept";
pub const TCP_SEND_SYMBOL: &str = "pop_rt_tcp_send";
pub const TCP_RECEIVE_SYMBOL: &str = "pop_rt_tcp_receive";
pub const TCP_SEND_BYTES_SYMBOL: &str = "pop_rt_tcp_send_bytes";
pub const TCP_RECEIVE_BYTES_SYMBOL: &str = "pop_rt_tcp_receive_bytes";
pub const TCP_RECEIVE_BUFFER_SYMBOL: &str = "pop_rt_tcp_receive_buffer";
pub const TCP_SHUTDOWN_SYMBOL: &str = "pop_rt_tcp_shutdown";
pub const TCP_SET_NO_DELAY_SYMBOL: &str = "pop_rt_tcp_set_no_delay";
pub const TCP_NO_DELAY_SYMBOL: &str = "pop_rt_tcp_no_delay";
pub const TCP_SET_TTL_SYMBOL: &str = "pop_rt_tcp_set_ttl";
pub const TCP_TTL_SYMBOL: &str = "pop_rt_tcp_ttl";
pub const TCP_ENDPOINT_PART_SYMBOL: &str = "pop_rt_tcp_endpoint_part";
pub const TCP_CLOSE_SYMBOL: &str = "pop_rt_tcp_close";

pub const UDP_BIND_SYMBOL: &str = "pop_rt_udp_bind";
pub const UDP_BIND_IPV4_SYMBOL: &str = "pop_rt_udp_bind_ipv4";
pub const UDP_BIND_IPV6_SYMBOL: &str = "pop_rt_udp_bind_ipv6";
pub const UDP_LOCAL_PORT_SYMBOL: &str = "pop_rt_udp_local_port";
pub const UDP_SEND_TO_SYMBOL: &str = "pop_rt_udp_send_to";
pub const UDP_RECEIVE_SYMBOL: &str = "pop_rt_udp_receive";
pub const UDP_SEND_BYTES_TO_SYMBOL: &str = "pop_rt_udp_send_bytes_to";
pub const UDP_RECEIVE_BYTES_SYMBOL: &str = "pop_rt_udp_receive_bytes";
pub const UDP_RECEIVE_BUFFER_SYMBOL: &str = "pop_rt_udp_receive_buffer";
pub const UDP_ENDPOINT_PART_SYMBOL: &str = "pop_rt_udp_endpoint_part";
pub const UDP_SET_BROADCAST_SYMBOL: &str = "pop_rt_udp_set_broadcast";
pub const UDP_BROADCAST_SYMBOL: &str = "pop_rt_udp_broadcast";
pub const UDP_SET_TTL_SYMBOL: &str = "pop_rt_udp_set_ttl";
pub const UDP_TTL_SYMBOL: &str = "pop_rt_udp_ttl";
pub const UDP_JOIN_MULTICAST_IPV4_SYMBOL: &str = "pop_rt_udp_join_multicast_ipv4";
pub const UDP_LEAVE_MULTICAST_IPV4_SYMBOL: &str = "pop_rt_udp_leave_multicast_ipv4";
pub const UDP_CLOSE_SYMBOL: &str = "pop_rt_udp_close";
pub const UNIX_LISTEN_SYMBOL: &str = "pop_rt_unix_listen";
pub const UNIX_CONNECT_SYMBOL: &str = "pop_rt_unix_connect";
pub const UNIX_ACCEPT_SYMBOL: &str = "pop_rt_unix_accept";
pub const UNIX_SEND_BYTES_SYMBOL: &str = "pop_rt_unix_send_bytes";
pub const UNIX_RECEIVE_BUFFER_SYMBOL: &str = "pop_rt_unix_receive_buffer";
pub const UNIX_SHUTDOWN_SYMBOL: &str = "pop_rt_unix_shutdown";
pub const UNIX_CLOSE_SYMBOL: &str = "pop_rt_unix_close";
pub const MONOTONIC_CLOCK_CREATE_SYMBOL: &str = "pop_rt_monotonic_clock_create";
pub const MONOTONIC_CLOCK_NOW_SYMBOL: &str = "pop_rt_monotonic_clock_now";
pub const MONOTONIC_CLOCK_CLOSE_SYMBOL: &str = "pop_rt_monotonic_clock_close";
pub const DEADLINE_AFTER_SYMBOL: &str = "pop_rt_deadline_after";
pub const DEADLINE_EXPIRED_SYMBOL: &str = "pop_rt_deadline_expired";
pub const DEADLINE_CLOSE_SYMBOL: &str = "pop_rt_deadline_close";
pub const TCP_SEND_BYTES_UNTIL_SYMBOL: &str = "pop_rt_tcp_send_bytes_until";
pub const TCP_RECEIVE_BUFFER_UNTIL_SYMBOL: &str = "pop_rt_tcp_receive_buffer_until";
pub const UDP_SEND_BYTES_TO_UNTIL_SYMBOL: &str = "pop_rt_udp_send_bytes_to_until";
pub const UDP_RECEIVE_BUFFER_UNTIL_SYMBOL: &str = "pop_rt_udp_receive_buffer_until";
pub const UNIX_SEND_BYTES_UNTIL_SYMBOL: &str = "pop_rt_unix_send_bytes_until";
pub const UNIX_RECEIVE_BUFFER_UNTIL_SYMBOL: &str = "pop_rt_unix_receive_buffer_until";
pub const NET_INTERFACES_SNAPSHOT_SYMBOL: &str = "pop_rt_net_interfaces_snapshot";
pub const NET_INTERFACES_CLOSE_SYMBOL: &str = "pop_rt_net_interfaces_close";
pub const NET_INTERFACE_COUNT_SYMBOL: &str = "pop_rt_net_interface_count";
pub const NET_INTERFACE_NAME_SYMBOL: &str = "pop_rt_net_interface_name";
pub const NET_INTERFACE_INDEX_SYMBOL: &str = "pop_rt_net_interface_index";
pub const NET_INTERFACE_FLAGS_SYMBOL: &str = "pop_rt_net_interface_flags";
pub const NET_INTERFACE_ADDRESS_COUNT_SYMBOL: &str = "pop_rt_net_interface_address_count";
pub const NET_INTERFACE_ADDRESS_PART_SYMBOL: &str = "pop_rt_net_interface_address_part";
pub const NET_ROUTES_SNAPSHOT_SYMBOL: &str = "pop_rt_net_routes_snapshot";
pub const NET_ROUTES_CLOSE_SYMBOL: &str = "pop_rt_net_routes_close";
pub const NET_ROUTE_COUNT_SYMBOL: &str = "pop_rt_net_route_count";
pub const NET_ROUTE_PART_SYMBOL: &str = "pop_rt_net_route_part";
pub const UDP_JOIN_MULTICAST_IPV6_SYMBOL: &str = "pop_rt_udp_join_multicast_ipv6";
pub const UDP_LEAVE_MULTICAST_IPV6_SYMBOL: &str = "pop_rt_udp_leave_multicast_ipv6";
pub const DNS_RESOLVER_CREATE_SYMBOL: &str = "pop_rt_dns_resolver_create";
pub const DNS_RESOLVER_CLOSE_SYMBOL: &str = "pop_rt_dns_resolver_close";
pub const DNS_RESOLVE_SYMBOL: &str = "pop_rt_dns_resolve";
pub const DNS_ANSWER_COUNT_SYMBOL: &str = "pop_rt_dns_answer_count";
pub const DNS_ANSWER_FAMILY_SYMBOL: &str = "pop_rt_dns_answer_family";
pub const DNS_ANSWER_IPV4_SYMBOL: &str = "pop_rt_dns_answer_ipv4";
pub const DNS_ANSWER_IPV6_WORD_SYMBOL: &str = "pop_rt_dns_answer_ipv6_word";
pub const DNS_ANSWERS_CLOSE_SYMBOL: &str = "pop_rt_dns_answers_close";

pub const ATOMIC_INT_CREATE_SYMBOL: &str = "pop_rt_atomic_int_create";
pub const ATOMIC_INT_LOAD_SYMBOL: &str = "pop_rt_atomic_int_load";
pub const ATOMIC_INT_STORE_SYMBOL: &str = "pop_rt_atomic_int_store";
pub const ATOMIC_INT_SWAP_SYMBOL: &str = "pop_rt_atomic_int_swap";
pub const ATOMIC_INT_COMPARE_EXCHANGE_SYMBOL: &str = "pop_rt_atomic_int_compare_exchange";
pub const ATOMIC_INT_FETCH_ADD_SYMBOL: &str = "pop_rt_atomic_int_fetch_add";
pub const ATOMIC_INT_FETCH_SUBTRACT_SYMBOL: &str = "pop_rt_atomic_int_fetch_subtract";
pub const ATOMIC_INT_FETCH_AND_SYMBOL: &str = "pop_rt_atomic_int_fetch_and";
pub const ATOMIC_INT_FETCH_OR_SYMBOL: &str = "pop_rt_atomic_int_fetch_or";
pub const ATOMIC_INT_FETCH_XOR_SYMBOL: &str = "pop_rt_atomic_int_fetch_xor";
pub const ATOMIC_BOOL_CREATE_SYMBOL: &str = "pop_rt_atomic_bool_create";
pub const ATOMIC_BOOL_LOAD_SYMBOL: &str = "pop_rt_atomic_bool_load";
pub const ATOMIC_BOOL_STORE_SYMBOL: &str = "pop_rt_atomic_bool_store";
pub const ATOMIC_BOOL_SWAP_SYMBOL: &str = "pop_rt_atomic_bool_swap";
pub const ATOMIC_BOOL_COMPARE_EXCHANGE_SYMBOL: &str = "pop_rt_atomic_bool_compare_exchange";
pub const ATOMIC_RELEASE_SYMBOL: &str = "pop_rt_atomic_release";

/// Returns the native C symbol for an operation implemented through ABI 1.45.
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
        RuntimeOperation::AtomicIntCreate => Some(ATOMIC_INT_CREATE_SYMBOL),
        RuntimeOperation::AtomicIntLoad => Some(ATOMIC_INT_LOAD_SYMBOL),
        RuntimeOperation::AtomicIntStore => Some(ATOMIC_INT_STORE_SYMBOL),
        RuntimeOperation::AtomicIntSwap => Some(ATOMIC_INT_SWAP_SYMBOL),
        RuntimeOperation::AtomicIntCompareExchange => Some(ATOMIC_INT_COMPARE_EXCHANGE_SYMBOL),
        RuntimeOperation::AtomicIntFetchAdd => Some(ATOMIC_INT_FETCH_ADD_SYMBOL),
        RuntimeOperation::AtomicIntFetchSubtract => Some(ATOMIC_INT_FETCH_SUBTRACT_SYMBOL),
        RuntimeOperation::AtomicIntFetchAnd => Some(ATOMIC_INT_FETCH_AND_SYMBOL),
        RuntimeOperation::AtomicIntFetchOr => Some(ATOMIC_INT_FETCH_OR_SYMBOL),
        RuntimeOperation::AtomicIntFetchXor => Some(ATOMIC_INT_FETCH_XOR_SYMBOL),
        RuntimeOperation::AtomicBoolCreate => Some(ATOMIC_BOOL_CREATE_SYMBOL),
        RuntimeOperation::AtomicBoolLoad => Some(ATOMIC_BOOL_LOAD_SYMBOL),
        RuntimeOperation::AtomicBoolStore => Some(ATOMIC_BOOL_STORE_SYMBOL),
        RuntimeOperation::AtomicBoolSwap => Some(ATOMIC_BOOL_SWAP_SYMBOL),
        RuntimeOperation::AtomicBoolCompareExchange => Some(ATOMIC_BOOL_COMPARE_EXCHANGE_SYMBOL),
        RuntimeOperation::AtomicRelease => Some(ATOMIC_RELEASE_SYMBOL),
        RuntimeOperation::ActorCreate => Some(ACTOR_CREATE_SYMBOL),
        RuntimeOperation::ActorActivate => Some(ACTOR_ACTIVATE_SYMBOL),
        RuntimeOperation::ActorTrySend => Some(ACTOR_TRY_SEND_SYMBOL),
        RuntimeOperation::ActorTrySendHandle => Some(ACTOR_TRY_SEND_HANDLE_SYMBOL),
        RuntimeOperation::ActorTryReceive => Some(ACTOR_TRY_RECEIVE_SYMBOL),
        RuntimeOperation::ActorBeginExit => Some(ACTOR_BEGIN_EXIT_SYMBOL),
        RuntimeOperation::ActorCompleteExit => Some(ACTOR_COMPLETE_EXIT_SYMBOL),
        RuntimeOperation::ActorRelease => Some(ACTOR_RELEASE_SYMBOL),
        RuntimeOperation::TcpListen => Some(TCP_LISTEN_SYMBOL),
        RuntimeOperation::TcpListenIpv4 => Some(TCP_LISTEN_IPV4_SYMBOL),
        RuntimeOperation::TcpListenIpv6 => Some(TCP_LISTEN_IPV6_SYMBOL),
        RuntimeOperation::TcpLocalPort => Some(TCP_LOCAL_PORT_SYMBOL),
        RuntimeOperation::TcpConnect => Some(TCP_CONNECT_SYMBOL),
        RuntimeOperation::TcpConnectIpv4 => Some(TCP_CONNECT_IPV4_SYMBOL),
        RuntimeOperation::TcpConnectIpv6 => Some(TCP_CONNECT_IPV6_SYMBOL),
        RuntimeOperation::TcpAccept => Some(TCP_ACCEPT_SYMBOL),
        RuntimeOperation::TcpSend => Some(TCP_SEND_SYMBOL),
        RuntimeOperation::TcpReceive => Some(TCP_RECEIVE_SYMBOL),
        RuntimeOperation::TcpSendBytes => Some(TCP_SEND_BYTES_SYMBOL),
        RuntimeOperation::TcpReceiveBytes => Some(TCP_RECEIVE_BYTES_SYMBOL),
        RuntimeOperation::TcpReceiveBuffer => Some(TCP_RECEIVE_BUFFER_SYMBOL),
        RuntimeOperation::TcpShutdown => Some(TCP_SHUTDOWN_SYMBOL),
        RuntimeOperation::TcpSetNoDelay => Some(TCP_SET_NO_DELAY_SYMBOL),
        RuntimeOperation::TcpNoDelay => Some(TCP_NO_DELAY_SYMBOL),
        RuntimeOperation::TcpSetTtl => Some(TCP_SET_TTL_SYMBOL),
        RuntimeOperation::TcpTtl => Some(TCP_TTL_SYMBOL),
        RuntimeOperation::TcpEndpointPart => Some(TCP_ENDPOINT_PART_SYMBOL),
        RuntimeOperation::TcpClose => Some(TCP_CLOSE_SYMBOL),
        RuntimeOperation::UdpBind => Some(UDP_BIND_SYMBOL),
        RuntimeOperation::UdpBindIpv4 => Some(UDP_BIND_IPV4_SYMBOL),
        RuntimeOperation::UdpBindIpv6 => Some(UDP_BIND_IPV6_SYMBOL),
        RuntimeOperation::UdpLocalPort => Some(UDP_LOCAL_PORT_SYMBOL),
        RuntimeOperation::UdpSendTo => Some(UDP_SEND_TO_SYMBOL),
        RuntimeOperation::UdpReceive => Some(UDP_RECEIVE_SYMBOL),
        RuntimeOperation::UdpSendBytesTo => Some(UDP_SEND_BYTES_TO_SYMBOL),
        RuntimeOperation::UdpReceiveBytes => Some(UDP_RECEIVE_BYTES_SYMBOL),
        RuntimeOperation::UdpReceiveBuffer => Some(UDP_RECEIVE_BUFFER_SYMBOL),
        RuntimeOperation::UdpEndpointPart => Some(UDP_ENDPOINT_PART_SYMBOL),
        RuntimeOperation::UdpSetBroadcast => Some(UDP_SET_BROADCAST_SYMBOL),
        RuntimeOperation::UdpBroadcast => Some(UDP_BROADCAST_SYMBOL),
        RuntimeOperation::UdpSetTtl => Some(UDP_SET_TTL_SYMBOL),
        RuntimeOperation::UdpTtl => Some(UDP_TTL_SYMBOL),
        RuntimeOperation::UdpJoinMulticastIpv4 => Some(UDP_JOIN_MULTICAST_IPV4_SYMBOL),
        RuntimeOperation::UdpLeaveMulticastIpv4 => Some(UDP_LEAVE_MULTICAST_IPV4_SYMBOL),
        RuntimeOperation::UdpClose => Some(UDP_CLOSE_SYMBOL),
        RuntimeOperation::UnixListen => Some(UNIX_LISTEN_SYMBOL),
        RuntimeOperation::UnixConnect => Some(UNIX_CONNECT_SYMBOL),
        RuntimeOperation::UnixAccept => Some(UNIX_ACCEPT_SYMBOL),
        RuntimeOperation::UnixSendBytes => Some(UNIX_SEND_BYTES_SYMBOL),
        RuntimeOperation::UnixReceiveBuffer => Some(UNIX_RECEIVE_BUFFER_SYMBOL),
        RuntimeOperation::UnixShutdown => Some(UNIX_SHUTDOWN_SYMBOL),
        RuntimeOperation::UnixClose => Some(UNIX_CLOSE_SYMBOL),
        RuntimeOperation::MonotonicClockCreate => Some(MONOTONIC_CLOCK_CREATE_SYMBOL),
        RuntimeOperation::MonotonicClockNow => Some(MONOTONIC_CLOCK_NOW_SYMBOL),
        RuntimeOperation::MonotonicClockClose => Some(MONOTONIC_CLOCK_CLOSE_SYMBOL),
        RuntimeOperation::DeadlineAfter => Some(DEADLINE_AFTER_SYMBOL),
        RuntimeOperation::DeadlineExpired => Some(DEADLINE_EXPIRED_SYMBOL),
        RuntimeOperation::DeadlineClose => Some(DEADLINE_CLOSE_SYMBOL),
        RuntimeOperation::TcpSendBytesUntil => Some(TCP_SEND_BYTES_UNTIL_SYMBOL),
        RuntimeOperation::TcpReceiveBufferUntil => Some(TCP_RECEIVE_BUFFER_UNTIL_SYMBOL),
        RuntimeOperation::UdpSendBytesToUntil => Some(UDP_SEND_BYTES_TO_UNTIL_SYMBOL),
        RuntimeOperation::UdpReceiveBufferUntil => Some(UDP_RECEIVE_BUFFER_UNTIL_SYMBOL),
        RuntimeOperation::UnixSendBytesUntil => Some(UNIX_SEND_BYTES_UNTIL_SYMBOL),
        RuntimeOperation::UnixReceiveBufferUntil => Some(UNIX_RECEIVE_BUFFER_UNTIL_SYMBOL),
        RuntimeOperation::NetInterfacesSnapshot => Some(NET_INTERFACES_SNAPSHOT_SYMBOL),
        RuntimeOperation::NetInterfacesClose => Some(NET_INTERFACES_CLOSE_SYMBOL),
        RuntimeOperation::NetInterfaceCount => Some(NET_INTERFACE_COUNT_SYMBOL),
        RuntimeOperation::NetInterfaceName => Some(NET_INTERFACE_NAME_SYMBOL),
        RuntimeOperation::NetInterfaceIndex => Some(NET_INTERFACE_INDEX_SYMBOL),
        RuntimeOperation::NetInterfaceFlags => Some(NET_INTERFACE_FLAGS_SYMBOL),
        RuntimeOperation::NetInterfaceAddressCount => Some(NET_INTERFACE_ADDRESS_COUNT_SYMBOL),
        RuntimeOperation::NetInterfaceAddressPart => Some(NET_INTERFACE_ADDRESS_PART_SYMBOL),
        RuntimeOperation::NetRoutesSnapshot => Some(NET_ROUTES_SNAPSHOT_SYMBOL),
        RuntimeOperation::NetRoutesClose => Some(NET_ROUTES_CLOSE_SYMBOL),
        RuntimeOperation::NetRouteCount => Some(NET_ROUTE_COUNT_SYMBOL),
        RuntimeOperation::NetRoutePart => Some(NET_ROUTE_PART_SYMBOL),
        RuntimeOperation::UdpJoinMulticastIpv6 => Some(UDP_JOIN_MULTICAST_IPV6_SYMBOL),
        RuntimeOperation::UdpLeaveMulticastIpv6 => Some(UDP_LEAVE_MULTICAST_IPV6_SYMBOL),
        RuntimeOperation::DnsResolverCreate => Some(DNS_RESOLVER_CREATE_SYMBOL),
        RuntimeOperation::DnsResolverClose => Some(DNS_RESOLVER_CLOSE_SYMBOL),
        RuntimeOperation::DnsResolve => Some(DNS_RESOLVE_SYMBOL),
        RuntimeOperation::DnsAnswerCount => Some(DNS_ANSWER_COUNT_SYMBOL),
        RuntimeOperation::DnsAnswerFamily => Some(DNS_ANSWER_FAMILY_SYMBOL),
        RuntimeOperation::DnsAnswerIpv4 => Some(DNS_ANSWER_IPV4_SYMBOL),
        RuntimeOperation::DnsAnswerIpv6Word => Some(DNS_ANSWER_IPV6_WORD_SYMBOL),
        RuntimeOperation::DnsAnswersClose => Some(DNS_ANSWERS_CLOSE_SYMBOL),
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
