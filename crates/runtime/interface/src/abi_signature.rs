use crate::RuntimeOperation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAbiType {
    U8,
    U16,
    U32,
    U64,
    I64,
    ReadOnlyU8Pointer,
    WritableU8Pointer,
    WritableU16Pointer,
    WritableU32Pointer,
    WritableU64Pointer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAbiSignature {
    parameters: &'static [RuntimeAbiType],
    result: RuntimeAbiType,
}

pub const CLOSED_RUNTIME_ABI_OPERATIONS: [RuntimeOperation; 116] = [
    RuntimeOperation::AtomicIntCreate,
    RuntimeOperation::AtomicIntLoad,
    RuntimeOperation::AtomicIntStore,
    RuntimeOperation::AtomicIntSwap,
    RuntimeOperation::AtomicIntCompareExchange,
    RuntimeOperation::AtomicIntFetchAdd,
    RuntimeOperation::AtomicIntFetchSubtract,
    RuntimeOperation::AtomicIntFetchAnd,
    RuntimeOperation::AtomicIntFetchOr,
    RuntimeOperation::AtomicIntFetchXor,
    RuntimeOperation::AtomicBoolCreate,
    RuntimeOperation::AtomicBoolLoad,
    RuntimeOperation::AtomicBoolStore,
    RuntimeOperation::AtomicBoolSwap,
    RuntimeOperation::AtomicBoolCompareExchange,
    RuntimeOperation::AtomicRelease,
    RuntimeOperation::ActorCreate,
    RuntimeOperation::ActorActivate,
    RuntimeOperation::ActorTrySend,
    RuntimeOperation::ActorTrySendHandle,
    RuntimeOperation::ActorTryReceive,
    RuntimeOperation::ActorBeginExit,
    RuntimeOperation::ActorCompleteExit,
    RuntimeOperation::ActorRelease,
    RuntimeOperation::TcpListen,
    RuntimeOperation::TcpListenIpv4,
    RuntimeOperation::TcpListenIpv6,
    RuntimeOperation::TcpLocalPort,
    RuntimeOperation::TcpConnect,
    RuntimeOperation::TcpConnectIpv4,
    RuntimeOperation::TcpConnectIpv6,
    RuntimeOperation::TcpAccept,
    RuntimeOperation::TcpSend,
    RuntimeOperation::TcpReceive,
    RuntimeOperation::TcpSendBytes,
    RuntimeOperation::TcpReceiveBytes,
    RuntimeOperation::TcpReceiveBuffer,
    RuntimeOperation::TcpShutdown,
    RuntimeOperation::TcpSetNoDelay,
    RuntimeOperation::TcpNoDelay,
    RuntimeOperation::TcpSetTtl,
    RuntimeOperation::TcpTtl,
    RuntimeOperation::TcpSetKeepalive,
    RuntimeOperation::TcpKeepalive,
    RuntimeOperation::TcpSetKeepaliveIdle,
    RuntimeOperation::TcpSetLinger,
    RuntimeOperation::TcpLinger,
    RuntimeOperation::TcpEndpointPart,
    RuntimeOperation::TcpClose,
    RuntimeOperation::UdpBind,
    RuntimeOperation::UdpBindIpv4,
    RuntimeOperation::UdpBindIpv6,
    RuntimeOperation::UdpLocalPort,
    RuntimeOperation::UdpSendTo,
    RuntimeOperation::UdpReceive,
    RuntimeOperation::UdpSendBytesTo,
    RuntimeOperation::UdpReceiveBytes,
    RuntimeOperation::UdpReceiveBuffer,
    RuntimeOperation::UdpEndpointPart,
    RuntimeOperation::UdpSetBroadcast,
    RuntimeOperation::UdpBroadcast,
    RuntimeOperation::UdpSetTtl,
    RuntimeOperation::UdpTtl,
    RuntimeOperation::UdpJoinMulticastIpv4,
    RuntimeOperation::UdpLeaveMulticastIpv4,
    RuntimeOperation::UdpClose,
    RuntimeOperation::UnixListen,
    RuntimeOperation::UnixConnect,
    RuntimeOperation::UnixAccept,
    RuntimeOperation::UnixSendBytes,
    RuntimeOperation::UnixReceiveBuffer,
    RuntimeOperation::UnixShutdown,
    RuntimeOperation::UnixClose,
    RuntimeOperation::MonotonicClockCreate,
    RuntimeOperation::MonotonicClockNow,
    RuntimeOperation::MonotonicClockClose,
    RuntimeOperation::DeadlineAfter,
    RuntimeOperation::DeadlineExpired,
    RuntimeOperation::DeadlineClose,
    RuntimeOperation::TcpSendBytesUntil,
    RuntimeOperation::TcpReceiveBufferUntil,
    RuntimeOperation::UdpSendBytesToUntil,
    RuntimeOperation::UdpReceiveBufferUntil,
    RuntimeOperation::UnixSendBytesUntil,
    RuntimeOperation::UnixReceiveBufferUntil,
    RuntimeOperation::NetInterfacesSnapshot,
    RuntimeOperation::NetInterfacesClose,
    RuntimeOperation::NetInterfaceCount,
    RuntimeOperation::NetInterfaceName,
    RuntimeOperation::NetInterfaceIndex,
    RuntimeOperation::NetInterfaceFlags,
    RuntimeOperation::NetInterfaceAddressCount,
    RuntimeOperation::NetInterfaceAddressPart,
    RuntimeOperation::NetRoutesSnapshot,
    RuntimeOperation::NetRoutesClose,
    RuntimeOperation::NetRouteCount,
    RuntimeOperation::NetRoutePart,
    RuntimeOperation::UdpJoinMulticastIpv6,
    RuntimeOperation::UdpLeaveMulticastIpv6,
    RuntimeOperation::DnsResolverCreate,
    RuntimeOperation::DnsResolverClose,
    RuntimeOperation::DnsResolve,
    RuntimeOperation::DnsAnswerCount,
    RuntimeOperation::DnsAnswerFamily,
    RuntimeOperation::DnsAnswerIpv4,
    RuntimeOperation::DnsAnswerIpv6Word,
    RuntimeOperation::DnsAnswersClose,
    RuntimeOperation::TlsClientSystemConfig,
    RuntimeOperation::TlsClientRootConfig,
    RuntimeOperation::TlsServerConfig,
    RuntimeOperation::TlsConfigClose,
    RuntimeOperation::TlsClientHandshake,
    RuntimeOperation::TlsServerHandshake,
    RuntimeOperation::TlsSendBytes,
    RuntimeOperation::TlsReceiveBuffer,
    RuntimeOperation::TlsClose,
];

impl RuntimeAbiSignature {
    #[must_use]
    pub const fn parameters(self) -> &'static [RuntimeAbiType] {
        self.parameters
    }

    #[must_use]
    pub const fn result(self) -> RuntimeAbiType {
        self.result
    }
}

const fn signature(
    parameters: &'static [RuntimeAbiType],
    result: RuntimeAbiType,
) -> RuntimeAbiSignature {
    RuntimeAbiSignature { parameters, result }
}

#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the closed ABI signature table is clearest as one exhaustive match"
)]
pub const fn runtime_abi_signature(operation: RuntimeOperation) -> Option<RuntimeAbiSignature> {
    use RuntimeAbiType::{
        I64, ReadOnlyU8Pointer, U8, U16, U32, U64, WritableU8Pointer, WritableU16Pointer,
        WritableU32Pointer, WritableU64Pointer,
    };
    use RuntimeOperation::{
        ActorActivate, ActorBeginExit, ActorCompleteExit, ActorCreate, ActorRelease,
        ActorTryReceive, ActorTrySend, ActorTrySendHandle, AtomicBoolCompareExchange,
        AtomicBoolCreate, AtomicBoolLoad, AtomicBoolStore, AtomicBoolSwap,
        AtomicIntCompareExchange, AtomicIntCreate, AtomicIntFetchAdd, AtomicIntFetchAnd,
        AtomicIntFetchOr, AtomicIntFetchSubtract, AtomicIntFetchXor, AtomicIntLoad, AtomicIntStore,
        AtomicIntSwap, AtomicRelease, DeadlineAfter, DeadlineClose, DeadlineExpired,
        DnsAnswerCount, DnsAnswerFamily, DnsAnswerIpv4, DnsAnswerIpv6Word, DnsAnswersClose,
        DnsResolve, DnsResolverClose, DnsResolverCreate, MonotonicClockClose, MonotonicClockCreate,
        MonotonicClockNow, NetInterfaceAddressCount, NetInterfaceAddressPart, NetInterfaceCount,
        NetInterfaceFlags, NetInterfaceIndex, NetInterfaceName, NetInterfacesClose,
        NetInterfacesSnapshot, NetRouteCount, NetRoutePart, NetRoutesClose, NetRoutesSnapshot,
        TcpAccept, TcpClose, TcpConnect, TcpConnectIpv4, TcpConnectIpv6, TcpEndpointPart,
        TcpKeepalive, TcpLinger, TcpListen, TcpListenIpv4, TcpListenIpv6, TcpLocalPort, TcpNoDelay,
        TcpReceive, TcpReceiveBuffer, TcpReceiveBufferUntil, TcpReceiveBytes, TcpSend,
        TcpSendBytes, TcpSendBytesUntil, TcpSetKeepalive, TcpSetKeepaliveIdle, TcpSetLinger,
        TcpSetNoDelay, TcpSetTtl, TcpShutdown, TcpTtl, TlsClientHandshake, TlsClientRootConfig,
        TlsClientSystemConfig, TlsClose, TlsConfigClose, TlsReceiveBuffer, TlsSendBytes,
        TlsServerConfig, TlsServerHandshake, UdpBind, UdpBindIpv4, UdpBindIpv6, UdpBroadcast,
        UdpClose, UdpEndpointPart, UdpJoinMulticastIpv4, UdpJoinMulticastIpv6,
        UdpLeaveMulticastIpv4, UdpLeaveMulticastIpv6, UdpLocalPort, UdpReceive, UdpReceiveBuffer,
        UdpReceiveBufferUntil, UdpReceiveBytes, UdpSendBytesTo, UdpSendBytesToUntil, UdpSendTo,
        UdpSetBroadcast, UdpSetTtl, UdpTtl, UnixAccept, UnixClose, UnixConnect, UnixListen,
        UnixReceiveBuffer, UnixReceiveBufferUntil, UnixSendBytes, UnixSendBytesUntil, UnixShutdown,
    };

    Some(match operation {
        AtomicIntCreate => signature(&[I64], U64),
        AtomicIntLoad => signature(&[U64, U8, WritableU64Pointer], U8),
        AtomicIntStore => signature(&[U64, I64, U8], U8),
        AtomicIntSwap
        | AtomicIntFetchAdd
        | AtomicIntFetchSubtract
        | AtomicIntFetchAnd
        | AtomicIntFetchOr
        | AtomicIntFetchXor => signature(&[U64, I64, U8, WritableU64Pointer], U8),
        AtomicIntCompareExchange => signature(&[U64, I64, I64, U8, U8, WritableU64Pointer], U8),
        AtomicBoolCreate => signature(&[U8], U64),
        AtomicBoolLoad => signature(&[U64, U8, WritableU8Pointer], U8),
        AtomicBoolStore => signature(&[U64, U8, U8], U8),
        AtomicBoolSwap => signature(&[U64, U8, U8, WritableU8Pointer], U8),
        AtomicBoolCompareExchange => signature(&[U64, U8, U8, U8, U8, WritableU8Pointer], U8),
        ActorCreate => signature(&[U64, U64, U64], U64),
        AtomicRelease | ActorActivate | ActorCompleteExit | ActorRelease | TcpClose | UdpClose
        | DnsResolverClose | DnsAnswersClose | UnixClose | MonotonicClockClose | DeadlineClose
        | NetInterfacesClose | NetRoutesClose | TlsConfigClose | TlsClose => signature(&[U64], U8),
        ActorTrySend => signature(&[U64, U64, U64, U64, U8], U8),
        ActorTrySendHandle => signature(&[U64, U64, U8], U8),
        ActorTryReceive => signature(&[U64, WritableU64Pointer, WritableU8Pointer], U8),
        ActorBeginExit | TcpShutdown | TcpSetNoDelay | TcpSetKeepalive | UdpSetBroadcast
        | UnixShutdown => signature(&[U64, U8], U8),
        TcpListen | TcpConnect | UdpBind => signature(&[U16], U64),
        TcpListenIpv4 | TcpConnectIpv4 | UdpBindIpv4 => signature(&[U32, U16], U64),
        TcpListenIpv6 | TcpConnectIpv6 | UdpBindIpv6 => {
            signature(&[U32, U32, U32, U32, U16, U32], U64)
        }
        DnsResolverCreate
        | MonotonicClockCreate
        | NetInterfacesSnapshot
        | NetRoutesSnapshot
        | TlsClientSystemConfig => signature(&[], U64),
        DnsResolve => signature(&[U64, U64, U16], U64),
        DnsAnswerCount | NetInterfaceCount | NetRouteCount => {
            signature(&[U64, WritableU64Pointer], U8)
        }
        DnsAnswerFamily | DeadlineExpired => signature(&[U64, U64, WritableU8Pointer], U8),
        DnsAnswerIpv4 | NetInterfaceIndex | NetInterfaceFlags => {
            signature(&[U64, U64, WritableU32Pointer], U8)
        }
        DnsAnswerIpv6Word => signature(&[U64, U64, U8, WritableU32Pointer], U8),
        TcpLocalPort | UdpLocalPort => signature(&[U64, WritableU16Pointer], U8),
        TcpAccept | UnixListen | UnixConnect | UnixAccept | TlsClientRootConfig => {
            signature(&[U64], U64)
        }
        TcpSend => signature(&[U64, ReadOnlyU8Pointer, U64, WritableU64Pointer], U8),
        TcpReceive => signature(&[U64, WritableU8Pointer, U64, WritableU64Pointer], U8),
        TcpSendBytes
        | UnixSendBytes
        | TlsSendBytes
        | NetInterfaceName
        | NetInterfaceAddressCount => signature(&[U64, U64, WritableU64Pointer], U8),
        TcpReceiveBytes => signature(&[U64, U64, WritableU64Pointer, WritableU64Pointer], U8),
        TcpReceiveBuffer | UnixReceiveBuffer | TlsReceiveBuffer => {
            signature(&[U64, U64, U64, WritableU64Pointer], U8)
        }
        TcpNoDelay | TcpKeepalive | UdpBroadcast => signature(&[U64, WritableU8Pointer], U8),
        TcpSetTtl | UdpSetTtl => signature(&[U64, U32], U8),
        TcpTtl | UdpTtl => signature(&[U64, WritableU32Pointer], U8),
        TcpSetKeepaliveIdle | TcpSetLinger => signature(&[U64, U64], U8),
        TcpLinger => signature(&[U64, WritableU64Pointer], U8),
        TcpEndpointPart => signature(&[U64, U8, U8, U8, WritableU32Pointer], U8),
        UdpSendTo => signature(
            &[U64, U32, U16, ReadOnlyU8Pointer, U64, WritableU64Pointer],
            U8,
        ),
        UdpReceive => signature(
            &[
                U64,
                WritableU8Pointer,
                U64,
                WritableU32Pointer,
                WritableU16Pointer,
                WritableU64Pointer,
            ],
            U8,
        ),
        UdpSendBytesTo => signature(&[U64, U32, U16, U64, WritableU64Pointer], U8),
        UdpReceiveBytes => signature(
            &[
                U64,
                U64,
                WritableU64Pointer,
                WritableU32Pointer,
                WritableU16Pointer,
                WritableU64Pointer,
            ],
            U8,
        ),
        UdpReceiveBuffer => signature(
            &[
                U64,
                U64,
                U64,
                WritableU32Pointer,
                WritableU16Pointer,
                WritableU64Pointer,
            ],
            U8,
        ),
        UdpEndpointPart => signature(&[U64, U8, U8, WritableU32Pointer], U8),
        UdpJoinMulticastIpv4 | UdpLeaveMulticastIpv4 => signature(&[U64, U32, U32], U8),
        UdpJoinMulticastIpv6 | UdpLeaveMulticastIpv6 => {
            signature(&[U64, U32, U32, U32, U32, U32], U8)
        }
        MonotonicClockNow => signature(&[U64, WritableU64Pointer, WritableU32Pointer], U8),
        DeadlineAfter => signature(&[U64, U64, U32], U64),
        TcpSendBytesUntil | UnixSendBytesUntil => {
            signature(&[U64, U64, U64, U64, WritableU64Pointer], U8)
        }
        TcpReceiveBufferUntil | UnixReceiveBufferUntil => {
            signature(&[U64, U64, U64, U64, U64, WritableU64Pointer], U8)
        }
        UdpSendBytesToUntil => signature(&[U64, U32, U16, U64, U64, U64, WritableU64Pointer], U8),
        UdpReceiveBufferUntil => signature(
            &[
                U64,
                U64,
                U64,
                U64,
                U64,
                WritableU32Pointer,
                WritableU16Pointer,
                WritableU64Pointer,
            ],
            U8,
        ),
        NetInterfaceAddressPart => signature(&[U64, U64, U64, U8, U8, WritableU32Pointer], U8),
        NetRoutePart => signature(&[U64, U64, U8, U8, WritableU32Pointer], U8),
        TlsServerConfig => signature(&[U64, U64], U64),
        TlsClientHandshake => signature(&[U64, U64, U64, U64, U64], U64),
        TlsServerHandshake => signature(&[U64, U64, U64, U64], U64),
        _ => return None,
    })
}
