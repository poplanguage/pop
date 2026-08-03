# Native Runtime ABI

`pop-runtime-native-abi` owns the closed, versioned C vocabulary used by the
native backend and trusted native bootstrap adapters. It maps accepted PLRI
operations to constant `pop_rt_*` symbols and records physical sentinel rules.
ABI 1.11 includes `pop_rt_allocate_initialized_object`, whose exact map and
initializer arrays represent one failure-atomic object publication.
ABI 1.13 adds `pop_rt_enter_foreign` and `pop_rt_leave_foreign` as distinct,
balanced transition entries with writable exact root arrays. ABI 1.12 remains
the immutable task-frame descriptor and both earlier descriptors stay
supported.
ABI 1.14 adds explicit `pop_rt_attach_managed_thread` and
`pop_rt_detach_managed_thread` entries without changing the 1.13 transition
shape.
ABI 1.18 adds failure-atomic callback registration, managed entry/leave, and
deterministic close entries. Callback contexts are opaque lookup tokens paired
with a compile-time site identity; they are never dereferenced managed-object
addresses.
ABI 1.19 adds the exact `pop_rt_codec_write_event` and
`pop_rt_codec_read_event` entries for ADR 0092's closed typed codec tape. Their
fixed-width tags and statuses carry no descriptor pointer, registry key,
runtime Item name, or variadic payload.
ABI 1.20 adds `pop_rt_allocate_initialized_object_at_site` and one fixed-width
immutable allocation-site descriptor. New LLVM output passes the descriptor
plus initializer words without rebuilding pointer maps on the stack; ABI 1.19
entries remain supported.
ABI 1.21 adds only ADR 0102's two verified adjacent-operation adapters.
ABI 1.22 adds
`pop_rt_allocate_initialized_self_referential_object_at_site` and the closed
`pop_rt_iteration_make` constructor from ADR 0104.
ABI 1.23 adds the typed `pop_rt_text_view_get_rune` adapter from
[ADR 0114](../../../architecture/decisions/0114-unicode-scalar-value-and-text-access.md).
ABI 1.24 appends the closed `String = 4` native iteration kind from
[ADR 0116](../../../architecture/decisions/0116-linear-string-rune-iteration.md).
ABI 1.25 appends the distinct reusable byte-buffer construction, mutation,
endian-write, and immutable-snapshot operations from
[ADR 0117](../../../architecture/decisions/0117-reusable-byte-buffer-and-endian-writes.md).
ABI 1.26 appends checked UTF-8 Text-view encoding, Bytes-view decoding, and
direct reusable-buffer decoding from
[ADR 0118](../../../architecture/decisions/0118-checked-utf8-transcoding.md).
ABI 1.27 appends bounded-channel construction, directional endpoint lifetime,
close, and closed non-suspending send/receive statuses from
[ADR 0146](../../../architecture/decisions/0146-native-bounded-channel-abi.md).
ABI 1.28 appends the closed nonblocking socket I/O status and separate
byte-count outputs for TCP and UDP I/O from
[ADR 0162](../../../architecture/decisions/0162-bounded-tcp-native-handles.md)
and [ADR 0163](../../../architecture/decisions/0163-bounded-udp-native-handles.md).
ABI 1.29 appends opaque local-Actor reference admission. The adapter recovers
the exact stored actor identity and incarnation before bounded admission.
ABI 1.30 appends typed Atomic integer add, subtract, and bitwise fetch
operations with exact prior-value outputs.

[ADR 0078](../../../architecture/decisions/0078-native-abi-2-writable-root-coexistence.md)
adds distinct immutable ABI 1.11 and ABI 2.0 descriptors. ADR 0114 adds the
compatible ABI 2.1 descriptor for the same typed scalar-read adapter. ADR 0116
adds the compatible ABI 2.2 closed String-iteration descriptor. ABI 2.3 adds
the same reusable byte-buffer operations; ABI 2.4 adds checked UTF-8
transcoding; ABI 2.5 adds the same bounded-channel operations. ABI 2 owns the
separate `pop_rt_gc_safe_point_v2` writable-root spelling and the fixed
`pop_rt_supports_abi` negotiation spelling; their presence never makes an
incomplete facade advertise ABI 2 support. ADR 0103's separately built
production facade is the complete ABI 2.5 composition; the default facade
continues to reject it.

It owns no heap, collector, exported function implementation, process-global
state, or backend lowering. Unsupported operations return no symbol instead of
receiving a fallback. See
[ADR 0038](../../../architecture/decisions/0038-modular-portable-runtime-implementation.md).
Static allocation-site descriptors are specified by
[ADR 0100](../../../architecture/decisions/0100-static-allocation-site-descriptors.md).
