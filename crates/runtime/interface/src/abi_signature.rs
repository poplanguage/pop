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

pub const CLOSED_RUNTIME_ABI_OPERATIONS: [RuntimeOperation; 30] = [
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
    RuntimeOperation::UdpBind,
    RuntimeOperation::UdpLocalPort,
    RuntimeOperation::UdpSendTo,
    RuntimeOperation::UdpReceive,
    RuntimeOperation::UdpClose,
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
pub const fn runtime_abi_signature(operation: RuntimeOperation) -> Option<RuntimeAbiSignature> {
    use RuntimeAbiType::{
        I64, ReadOnlyU8Pointer, U8, U16, U32, U64, WritableU8Pointer, WritableU16Pointer,
        WritableU32Pointer, WritableU64Pointer,
    };
    use RuntimeOperation::{
        ActorActivate, ActorBeginExit, ActorCompleteExit, ActorCreate, ActorRelease,
        ActorTryReceive, ActorTrySend, AtomicBoolCompareExchange, AtomicBoolCreate, AtomicBoolLoad,
        AtomicBoolStore, AtomicBoolSwap, AtomicIntCompareExchange, AtomicIntCreate, AtomicIntLoad,
        AtomicIntStore, AtomicIntSwap, AtomicRelease, TcpAccept, TcpClose, TcpConnect, TcpListen,
        TcpLocalPort, TcpReceive, TcpSend, UdpBind, UdpClose, UdpLocalPort, UdpReceive, UdpSendTo,
    };

    Some(match operation {
        AtomicIntCreate => signature(&[I64], U64),
        AtomicIntLoad => signature(&[U64, U8, WritableU64Pointer], U8),
        AtomicIntStore => signature(&[U64, I64, U8], U8),
        AtomicIntSwap => signature(&[U64, I64, U8, WritableU64Pointer], U8),
        AtomicIntCompareExchange => signature(&[U64, I64, I64, U8, U8, WritableU64Pointer], U8),
        AtomicBoolCreate => signature(&[U8], U64),
        AtomicBoolLoad => signature(&[U64, U8, WritableU8Pointer], U8),
        AtomicBoolStore => signature(&[U64, U8, U8], U8),
        AtomicBoolSwap => signature(&[U64, U8, U8, WritableU8Pointer], U8),
        AtomicBoolCompareExchange => signature(&[U64, U8, U8, U8, U8, WritableU8Pointer], U8),
        ActorCreate => signature(&[U64, U64, U64], U64),
        AtomicRelease | ActorActivate | ActorCompleteExit | ActorRelease | TcpClose | UdpClose => {
            signature(&[U64], U8)
        }
        ActorTrySend => signature(&[U64, U64, U64, U64, U8], U8),
        ActorTryReceive => signature(&[U64, WritableU64Pointer, WritableU8Pointer], U8),
        ActorBeginExit => signature(&[U64, U8], U8),
        TcpListen | TcpConnect | UdpBind => signature(&[U16], U64),
        TcpLocalPort | UdpLocalPort => signature(&[U64, WritableU16Pointer], U8),
        TcpAccept => signature(&[U64], U64),
        TcpSend => signature(&[U64, ReadOnlyU8Pointer, U64, WritableU64Pointer], U8),
        TcpReceive => signature(&[U64, WritableU8Pointer, U64, WritableU64Pointer], U8),
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
        _ => return None,
    })
}
