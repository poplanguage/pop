//! Verified-MIR execution engine and its public resource-limited API.
//!
//! Construction verifies the complete `MirBubble` before retaining it. Execution
//! consumes resolved stable IDs only and delegates every runtime operation through
//! the backend-neutral PLRI adapter.
#![allow(unsafe_code)]
use crate::evaluation::*;
use crate::ffi_buffer::{
    integer_from_u64, integer_i64, integer_kind_for_type, integer_u64, marshal, unmarshal,
};
use crate::runtime::ReferenceRuntimeAdapter;
use crate::values::{
    MirClassValue, MirCodecError, MirCodecEvent, MirCodecReader, MirValue, MirViewLenderValue,
    MirViewValue, RuntimeValue,
};
use pop_foundation::{
    BorrowRegionId, ClassId, FfiCallbackSiteId as MirFfiCallbackSiteId, NestedFunctionId, SymbolId,
    SymbolIdentity, TypeId, ValueId,
};
use pop_mir::{
    MirBubble, MirCancellationMode, MirDeclarationKind, MirFfiLayout, MirFfiValueClass,
    MirGeneratedCodecAdapter, MirGeneratedCodecMemberId, MirInstruction, MirInstructionKind,
    MirSuspendOperation, MirTaskDispatch, MirTerminator, MirUnwindAction, MirVerificationError,
    is_managed_reference_type_id, verify_mir_bubble,
};
use pop_runtime_interface::{
    ActorExit, ActorId, ActorIncarnation, ActorLifecycle, ActorReceive, ActorSendError,
    AllocationClass, ArrayAllocationRequest, AtomicBoolean, AtomicCompareExchangeOrder, AtomicInt,
    AtomicLoadOrder, AtomicReadModifyWriteOrder, AtomicStoreOrder, BarrierKind,
    CancellationObservation, CancellationTokenId, ChannelId, ChannelLifecycle, ChannelReceive,
    ChannelSendError, FfiBufferBorrowId, FfiBufferOpenFailure, FfiBufferOpenRequest,
    FfiBytesBorrowId, FfiCallbackCloseFailure, FfiCallbackLifetime, FfiCallbackOpenFailure,
    FfiCallbackOpenRequest, FfiCallbackRegistration, FfiCallbackRegistrationId, FfiCallbackSiteId,
    FfiCallbackThread, ForeignAddress, ForeignCallMode, ManagedReference, ObjectAllocationRequest,
    ObjectMap, ObjectSlot, PinHandle, RootHandle, RootPublication, RootSlot, RuntimeAdapter,
    RuntimeFailure, RuntimeTypeId, SchedulerId, StackMap, TableAllocationRequest, TaskGroupExit,
    TaskGroupId, TaskGroupLifecycle, TaskId, TaskLifecycle, TaskOwner, TaskPollCompletion,
    TaskState as RuntimeTaskState, Trap, TrapKind, UnwindReason, WriteBarrier,
};
use pop_types::{
    FFI_CALLBACK_CONTEXT_TYPE_ID, FFI_HANDLE_TYPE_ID, FFI_OPTIONAL_POINTER_TYPE_ID,
    FFI_OPTIONAL_READ_ONLY_POINTER_TYPE_ID, FloatKind, IntegerKind, IntegerValue, PrimitiveType,
    SemanticType, TypeArena, is_ffi_function_type_constructor, is_ffi_integer_abi_builtin_type,
    is_ffi_pointer_type_constructor,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection};
use rustls_platform_verifier::ConfigVerifierExt;
use std::cell::{Ref, RefCell};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(unix)]
use std::ffi::CStr;
#[cfg(target_os = "linux")]
use std::ffi::CString;
use std::io::{Read as _, Write as _};
use std::net::{
    IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream, ToSocketAddrs,
    UdpSocket,
};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Once};
use std::thread;
use std::time::{Duration, Instant};

const MAX_CODEC_NESTING_DEPTH: u8 = 32;
const MAX_CODEC_EVENTS: usize = 65_536;
const MAX_CODEC_SEQUENCE_ELEMENTS: usize = 65_535;

fn scoped_file_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::ParentDir
        )
    }) {
        return None;
    }
    let canonical = std::fs::canonicalize(root.join(path)).ok()?;
    canonical.starts_with(root).then_some(canonical)
}

#[derive(Clone)]
struct InterpreterInterfaceAddress {
    family: u8,
    words: [u32; 4],
    prefix: u8,
    scope: u32,
}

#[derive(Clone)]
struct InterpreterInterface {
    name: String,
    index: u32,
    flags: u32,
    addresses: Vec<InterpreterInterfaceAddress>,
}

#[derive(Clone)]
struct InterpreterRoute {
    family: u8,
    destination: [u32; 4],
    prefix: u8,
    gateway: [u32; 4],
    interface: u32,
    metric: u32,
    flags: u32,
}

#[cfg(unix)]
fn interface_ipv4_prefix(mask: *const libc::sockaddr) -> u8 {
    if mask.is_null() {
        return 0;
    }
    let mask = unsafe { std::ptr::read_unaligned(mask.cast::<libc::sockaddr_in>()) };
    u8::try_from(u32::from_be(mask.sin_addr.s_addr).leading_ones()).unwrap_or(0)
}

#[cfg(unix)]
fn interface_ipv6_prefix(mask: *const libc::sockaddr) -> u8 {
    if mask.is_null() {
        return 0;
    }
    let mask = unsafe { std::ptr::read_unaligned(mask.cast::<libc::sockaddr_in6>()) };
    let mut prefix = 0_u8;
    for byte in mask.sin6_addr.s6_addr {
        let ones = byte.leading_ones();
        prefix = prefix.saturating_add(u8::try_from(ones).unwrap_or(0));
        if ones != 8 {
            break;
        }
    }
    prefix
}

#[cfg(unix)]
fn capture_interface_address(entry: &libc::ifaddrs) -> Option<InterpreterInterfaceAddress> {
    if entry.ifa_addr.is_null() {
        return None;
    }
    let family = unsafe { i32::from((*entry.ifa_addr).sa_family) };
    match family {
        libc::AF_INET => {
            let socket =
                unsafe { std::ptr::read_unaligned(entry.ifa_addr.cast::<libc::sockaddr_in>()) };
            Some(InterpreterInterfaceAddress {
                family: 4,
                words: [u32::from_be(socket.sin_addr.s_addr), 0, 0, 0],
                prefix: interface_ipv4_prefix(entry.ifa_netmask),
                scope: 0,
            })
        }
        libc::AF_INET6 => {
            let socket =
                unsafe { std::ptr::read_unaligned(entry.ifa_addr.cast::<libc::sockaddr_in6>()) };
            let mut words = [0_u32; 4];
            for (index, octets) in socket.sin6_addr.s6_addr.chunks_exact(4).enumerate() {
                words[index] = u32::from_be_bytes([octets[0], octets[1], octets[2], octets[3]]);
            }
            Some(InterpreterInterfaceAddress {
                family: 6,
                words,
                prefix: interface_ipv6_prefix(entry.ifa_netmask),
                scope: socket.sin6_scope_id,
            })
        }
        _ => None,
    }
}

#[cfg(unix)]
fn capture_interfaces() -> Option<Vec<InterpreterInterface>> {
    let mut head = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&raw mut head) } != 0 {
        return None;
    }
    let mut by_index = BTreeMap::<u32, InterpreterInterface>::new();
    let mut current = head;
    while !current.is_null() {
        let entry = unsafe { &*current };
        if !entry.ifa_name.is_null() {
            let name = unsafe { CStr::from_ptr(entry.ifa_name) }
                .to_string_lossy()
                .into_owned();
            let index = unsafe { libc::if_nametoindex(entry.ifa_name) };
            if index != 0 {
                let interface = by_index
                    .entry(index)
                    .or_insert_with(|| InterpreterInterface {
                        name,
                        index,
                        flags: 0,
                        addresses: Vec::new(),
                    });
                interface.flags |= entry.ifa_flags;
                if let Some(address) = capture_interface_address(entry) {
                    interface.addresses.push(address);
                }
            }
        }
        current = entry.ifa_next;
    }
    unsafe { libc::freeifaddrs(head) };
    Some(by_index.into_values().collect())
}

#[cfg(not(unix))]
fn capture_interfaces() -> Option<Vec<InterpreterInterface>> {
    Some(Vec::new())
}

#[cfg(target_os = "linux")]
fn interpreter_interface_index(name: &str) -> u32 {
    CString::new(name)
        .ok()
        .map_or(0, |name| unsafe { libc::if_nametoindex(name.as_ptr()) })
}

fn parse_route_hex(value: &str) -> Option<u32> {
    u32::from_str_radix(value, 16).ok()
}

fn parse_ipv6_route_words(value: &str) -> Option<[u32; 4]> {
    if value.len() != 32 {
        return None;
    }
    let mut words = [0_u32; 4];
    for (index, word) in words.iter_mut().enumerate() {
        *word = parse_route_hex(&value[index * 8..index * 8 + 8])?;
    }
    Some(words)
}

#[cfg(target_os = "linux")]
fn capture_routes() -> Vec<InterpreterRoute> {
    let mut routes = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/proc/net/route") {
        for line in text.lines().skip(1) {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let Some((destination, gateway, flags, metric, mask)) = (fields.len() >= 8)
                .then(|| {
                    Some((
                        parse_route_hex(fields[1])?,
                        parse_route_hex(fields[2])?,
                        parse_route_hex(fields[3])?,
                        fields[6].parse::<u32>().ok()?,
                        parse_route_hex(fields[7])?,
                    ))
                })
                .flatten()
            else {
                continue;
            };
            routes.push(InterpreterRoute {
                family: 4,
                destination: [destination.swap_bytes(), 0, 0, 0],
                prefix: u8::try_from(mask.swap_bytes().leading_ones()).unwrap_or(0),
                gateway: [gateway.swap_bytes(), 0, 0, 0],
                interface: interpreter_interface_index(fields[0]),
                metric,
                flags,
            });
        }
    }
    if let Ok(text) = std::fs::read_to_string("/proc/net/ipv6_route") {
        for line in text.lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 10 {
                continue;
            }
            let Some(destination) = parse_ipv6_route_words(fields[0]) else {
                continue;
            };
            let Some(prefix) = u8::from_str_radix(fields[1], 16).ok() else {
                continue;
            };
            let Some(gateway) = parse_ipv6_route_words(fields[4]) else {
                continue;
            };
            let Some(metric) = parse_route_hex(fields[5]) else {
                continue;
            };
            let Some(flags) = parse_route_hex(fields[8]) else {
                continue;
            };
            routes.push(InterpreterRoute {
                family: 6,
                destination,
                prefix,
                gateway,
                interface: interpreter_interface_index(fields[9]),
                metric,
                flags,
            });
        }
    }
    routes
}

#[cfg(not(target_os = "linux"))]
fn capture_routes() -> Vec<InterpreterRoute> {
    Vec::new()
}

#[cfg(unix)]
fn interpreter_set_socket_i32(stream: &TcpStream, level: i32, option: i32, value: i32) -> bool {
    let length = libc::socklen_t::try_from(std::mem::size_of_val(&value)).unwrap_or(0);
    unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            level,
            option,
            (&raw const value).cast(),
            length,
        ) == 0
    }
}

#[cfg(unix)]
fn interpreter_socket_i32(stream: &TcpStream, level: i32, option: i32) -> Option<i32> {
    let mut value = 0_i32;
    let mut length = libc::socklen_t::try_from(std::mem::size_of_val(&value)).ok()?;
    let accepted = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            level,
            option,
            (&raw mut value).cast(),
            &raw mut length,
        ) == 0
    };
    accepted.then_some(value)
}

#[cfg(unix)]
fn interpreter_set_linger(stream: &TcpStream, milliseconds: u64) -> bool {
    let seconds = milliseconds.saturating_add(999) / 1_000;
    let Ok(seconds) = i32::try_from(seconds) else {
        return false;
    };
    let linger = libc::linger {
        l_onoff: i32::from(milliseconds != 0),
        l_linger: seconds,
    };
    let length = libc::socklen_t::try_from(std::mem::size_of_val(&linger)).unwrap_or(0);
    unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_LINGER,
            (&raw const linger).cast(),
            length,
        ) == 0
    }
}

#[cfg(unix)]
fn interpreter_linger(stream: &TcpStream) -> Option<u64> {
    let mut linger = libc::linger {
        l_onoff: 0,
        l_linger: 0,
    };
    let mut length = libc::socklen_t::try_from(std::mem::size_of_val(&linger)).ok()?;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_LINGER,
            (&raw mut linger).cast(),
            &raw mut length,
        )
    } != 0
    {
        return None;
    }
    if linger.l_onoff == 0 {
        Some(0)
    } else {
        u64::try_from(linger.l_linger).ok()?.checked_mul(1_000)
    }
}

fn install_interpreter_tls_provider() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn complete_interpreter_tls_handshake(
    deadline: Instant,
    cancellation: &Rc<RefCell<CancellationState>>,
    mut complete: impl FnMut() -> std::io::Result<bool>,
) -> bool {
    loop {
        if cancellation.borrow().requested || Instant::now() >= deadline {
            return false;
        }
        match complete() {
            Ok(false) => return true,
            Ok(true) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return false,
        }
        thread::yield_now();
    }
}

fn push_codec_event(
    events: &mut Vec<MirCodecEvent>,
    event: MirCodecEvent,
) -> Result<(), MirCodecError> {
    if events.len() >= MAX_CODEC_EVENTS {
        return Err(MirCodecError::LimitExceeded);
    }
    events.push(event);
    Ok(())
}

fn managed_type(arena: &TypeArena, type_id: TypeId) -> bool {
    is_managed_reference_type_id(type_id, Some(arena))
}

fn ffi_pointer(value: &MirValue) -> Result<ForeignAddress, ExecutionError> {
    let MirValue::FfiPointer(address) = value else {
        return Err(ExecutionError::TypeMismatch);
    };
    Ok(*address)
}

fn encode_codec_value<R: RuntimeAdapter>(
    adapter: &MirGeneratedCodecAdapter,
    value: &MirValue,
    events: &mut Vec<MirCodecEvent>,
    arena: &TypeArena,
    catalog: &[MirGeneratedCodecAdapter],
    runtime: &mut R,
    depth: u8,
) -> Result<(), MirCodecError> {
    if depth > MAX_CODEC_NESTING_DEPTH {
        return Err(MirCodecError::LimitExceeded);
    }
    match value {
        MirValue::Record { record, fields } if *record == adapter.target().symbol() => {
            push_codec_event(
                events,
                MirCodecEvent::RecordStart(
                    u16::try_from(adapter.members().len())
                        .map_err(|_| MirCodecError::LimitExceeded)?,
                ),
            )?;
            for member in adapter.members() {
                let MirGeneratedCodecMemberId::Field(field) = member.member() else {
                    return Err(MirCodecError::CapabilityFailure);
                };
                let field_value = fields
                    .iter()
                    .find_map(|(found, value)| (*found == field).then_some(value))
                    .ok_or(MirCodecError::CapabilityFailure)?;
                push_codec_event(
                    events,
                    MirCodecEvent::Member {
                        ordinal: member.ordinal(),
                        label: member.name().to_owned(),
                    },
                )?;
                encode_codec_scalar(
                    member
                        .types()
                        .first()
                        .copied()
                        .ok_or(MirCodecError::CapabilityFailure)?,
                    field_value,
                    events,
                    arena,
                    catalog,
                    runtime,
                    depth + 1,
                )?;
            }
            push_codec_event(events, MirCodecEvent::RecordEnd)?;
        }
        MirValue::Enum {
            definition,
            case,
            discriminant,
        } if *definition == adapter.target().symbol() => {
            let member = adapter
                .members()
                .iter()
                .find(|member| member.member() == MirGeneratedCodecMemberId::EnumCase(*case))
                .ok_or(MirCodecError::CapabilityFailure)?;
            if member.discriminant() != Some(*discriminant) {
                return Err(MirCodecError::CapabilityFailure);
            }
            push_codec_event(
                events,
                MirCodecEvent::EnumCase {
                    ordinal: member.ordinal(),
                    label: member.name().to_owned(),
                    discriminant: *discriminant,
                },
            )?;
        }
        MirValue::Union {
            union,
            case,
            arguments,
        } if *union == adapter.target().symbol() => {
            let member = adapter
                .members()
                .iter()
                .find(|member| member.member() == MirGeneratedCodecMemberId::UnionCase(*case))
                .ok_or(MirCodecError::CapabilityFailure)?;
            if member.types().len() != arguments.len() {
                return Err(MirCodecError::CapabilityFailure);
            }
            push_codec_event(
                events,
                MirCodecEvent::UnionStart {
                    ordinal: member.ordinal(),
                    label: member.name().to_owned(),
                    payload_count: u16::try_from(arguments.len())
                        .map_err(|_| MirCodecError::LimitExceeded)?,
                },
            )?;
            for (ordinal, (type_id, argument)) in member.types().iter().zip(arguments).enumerate() {
                push_codec_event(
                    events,
                    MirCodecEvent::Payload(
                        u16::try_from(ordinal).map_err(|_| MirCodecError::LimitExceeded)?,
                    ),
                )?;
                encode_codec_scalar(
                    *type_id,
                    argument,
                    events,
                    arena,
                    catalog,
                    runtime,
                    depth + 1,
                )?;
            }
            push_codec_event(events, MirCodecEvent::UnionEnd)?;
        }
        _ => return Err(MirCodecError::CapabilityFailure),
    }
    Ok(())
}

fn encode_codec_scalar<R: RuntimeAdapter>(
    type_id: TypeId,
    value: &MirValue,
    events: &mut Vec<MirCodecEvent>,
    arena: &TypeArena,
    catalog: &[MirGeneratedCodecAdapter],
    runtime: &mut R,
    depth: u8,
) -> Result<(), MirCodecError> {
    if depth > MAX_CODEC_NESTING_DEPTH {
        return Err(MirCodecError::LimitExceeded);
    }
    if let Some(adapter) = catalog
        .iter()
        .find(|adapter| adapter.target_type() == type_id)
    {
        return encode_codec_value(adapter, value, events, arena, catalog, runtime, depth);
    }
    match (arena.get(type_id), value) {
        (Some(SemanticType::Tuple(types)), MirValue::Tuple(values))
            if types.len() == values.len() =>
        {
            push_codec_event(
                events,
                MirCodecEvent::TupleStart(
                    u16::try_from(values.len()).map_err(|_| MirCodecError::LimitExceeded)?,
                ),
            )?;
            for (index, (type_id, value)) in types.iter().zip(values).enumerate() {
                push_codec_event(
                    events,
                    MirCodecEvent::Element(
                        u16::try_from(index).map_err(|_| MirCodecError::LimitExceeded)?,
                    ),
                )?;
                encode_codec_scalar(*type_id, value, events, arena, catalog, runtime, depth + 1)?;
            }
            push_codec_event(events, MirCodecEvent::TupleEnd)?;
            return Ok(());
        }
        (Some(SemanticType::Array(element)), MirValue::Array(values)) => {
            encode_codec_sequence(*element, values, events, arena, catalog, runtime, depth + 1)?;
            return Ok(());
        }
        (
            Some(SemanticType::Builtin {
                definition,
                arguments,
            }),
            MirValue::List(values),
        ) if definition.raw() == 101 && arguments.len() == 1 => {
            encode_codec_sequence(
                arguments[0],
                values,
                events,
                arena,
                catalog,
                runtime,
                depth + 1,
            )?;
            return Ok(());
        }
        (Some(SemanticType::Union(types)), value)
            if types.len() == 2
                && types.iter().any(|type_id| {
                    arena.get(*type_id) == Some(&SemanticType::Primitive(PrimitiveType::Nil))
                }) =>
        {
            if matches!(value, MirValue::Nil) {
                push_codec_event(events, MirCodecEvent::OptionalAbsent)?;
            } else {
                push_codec_event(events, MirCodecEvent::OptionalPresent)?;
                let payload = types
                    .iter()
                    .copied()
                    .find(|type_id| {
                        arena.get(*type_id) != Some(&SemanticType::Primitive(PrimitiveType::Nil))
                    })
                    .ok_or(MirCodecError::CapabilityFailure)?;
                encode_codec_scalar(payload, value, events, arena, catalog, runtime, depth + 1)?;
            }
            return Ok(());
        }
        _ => {}
    }
    let event = match (arena.get(type_id), value) {
        (Some(SemanticType::Primitive(PrimitiveType::Boolean)), MirValue::Boolean(value)) => {
            MirCodecEvent::Boolean(*value)
        }
        (Some(SemanticType::Primitive(PrimitiveType::String)), MirValue::String(value)) => {
            MirCodecEvent::String(value.clone())
        }
        (
            Some(SemanticType::Builtin {
                definition,
                arguments,
            }),
            MirValue::Bytes(reference),
        ) if definition.raw() == 0 && arguments.is_empty() => {
            let length = runtime
                .immutable_bytes_length(*reference)
                .map_err(|_| MirCodecError::CapabilityFailure)?;
            if length > MAX_CODEC_SEQUENCE_ELEMENTS as u64 {
                return Err(MirCodecError::LimitExceeded);
            }
            let mut bytes =
                vec![0; usize::try_from(length).map_err(|_| MirCodecError::LimitExceeded)?];
            runtime
                .immutable_bytes_read(*reference, 0, &mut bytes)
                .map_err(|_| MirCodecError::CapabilityFailure)?;
            MirCodecEvent::Bytes(bytes)
        }
        (Some(SemanticType::Primitive(PrimitiveType::Integer(kind))), MirValue::Integer(value))
            if value.kind() == *kind =>
        {
            MirCodecEvent::Integer(*value)
        }
        (Some(SemanticType::Primitive(PrimitiveType::Float32)), MirValue::Float(value))
            if value.kind() == FloatKind::Float32 =>
        {
            MirCodecEvent::Float(*value)
        }
        (Some(SemanticType::Primitive(PrimitiveType::Float64)), MirValue::Float(value))
            if value.kind() == FloatKind::Float64 =>
        {
            MirCodecEvent::Float(*value)
        }
        _ => return Err(MirCodecError::CapabilityFailure),
    };
    push_codec_event(events, event)
}

fn encode_codec_sequence<R: RuntimeAdapter>(
    element: TypeId,
    values: &[MirValue],
    events: &mut Vec<MirCodecEvent>,
    arena: &TypeArena,
    catalog: &[MirGeneratedCodecAdapter],
    runtime: &mut R,
    depth: u8,
) -> Result<(), MirCodecError> {
    if values.len() > MAX_CODEC_SEQUENCE_ELEMENTS || depth > MAX_CODEC_NESTING_DEPTH {
        return Err(MirCodecError::LimitExceeded);
    }
    push_codec_event(
        events,
        MirCodecEvent::SequenceStart(
            u32::try_from(values.len()).map_err(|_| MirCodecError::LimitExceeded)?,
        ),
    )?;
    for (index, value) in values.iter().enumerate() {
        push_codec_event(
            events,
            MirCodecEvent::Element(u16::try_from(index).map_err(|_| MirCodecError::LimitExceeded)?),
        )?;
        encode_codec_scalar(element, value, events, arena, catalog, runtime, depth)?;
    }
    push_codec_event(events, MirCodecEvent::SequenceEnd)?;
    Ok(())
}

fn decode_codec_value<R: RuntimeAdapter>(
    adapter: &MirGeneratedCodecAdapter,
    reader: &MirCodecReader,
    arena: &TypeArena,
    catalog: &[MirGeneratedCodecAdapter],
    runtime: &mut R,
    depth: u8,
) -> Result<MirValue, MirCodecError> {
    if depth > MAX_CODEC_NESTING_DEPTH {
        return Err(MirCodecError::LimitExceeded);
    }
    match next_codec_event(reader)? {
        MirCodecEvent::RecordStart(count) if usize::from(count) == adapter.members().len() => {
            let mut fields = Vec::with_capacity(adapter.members().len());
            for member in adapter.members() {
                if next_codec_event(reader)?
                    != (MirCodecEvent::Member {
                        ordinal: member.ordinal(),
                        label: member.name().to_owned(),
                    })
                {
                    return Err(MirCodecError::MalformedInput);
                }
                let MirGeneratedCodecMemberId::Field(field) = member.member() else {
                    return Err(MirCodecError::MalformedInput);
                };
                let type_id = member
                    .types()
                    .first()
                    .copied()
                    .ok_or(MirCodecError::MalformedInput)?;
                fields.push((
                    field,
                    decode_codec_scalar(type_id, reader, arena, catalog, runtime, depth + 1)?,
                ));
            }
            if next_codec_event(reader)? != MirCodecEvent::RecordEnd {
                return Err(MirCodecError::MalformedInput);
            }
            Ok(MirValue::Record {
                record: adapter.target().symbol(),
                fields,
            })
        }
        MirCodecEvent::EnumCase {
            ordinal,
            label,
            discriminant,
        } => {
            let member = adapter
                .members()
                .iter()
                .find(|member| {
                    member.ordinal() == ordinal
                        && member.name() == label
                        && member.discriminant() == Some(discriminant)
                })
                .ok_or(MirCodecError::MalformedInput)?;
            let MirGeneratedCodecMemberId::EnumCase(case) = member.member() else {
                return Err(MirCodecError::MalformedInput);
            };
            Ok(MirValue::Enum {
                definition: adapter.target().symbol(),
                case,
                discriminant,
            })
        }
        MirCodecEvent::UnionStart {
            ordinal,
            label,
            payload_count,
        } => {
            let member = adapter
                .members()
                .iter()
                .find(|member| member.ordinal() == ordinal && member.name() == label)
                .ok_or(MirCodecError::MalformedInput)?;
            let MirGeneratedCodecMemberId::UnionCase(case) = member.member() else {
                return Err(MirCodecError::MalformedInput);
            };
            if usize::from(payload_count) != member.types().len() {
                return Err(MirCodecError::MalformedInput);
            }
            let mut arguments = Vec::with_capacity(member.types().len());
            for (ordinal, type_id) in member.types().iter().enumerate() {
                if next_codec_event(reader)?
                    != MirCodecEvent::Payload(
                        u16::try_from(ordinal).map_err(|_| MirCodecError::LimitExceeded)?,
                    )
                {
                    return Err(MirCodecError::MalformedInput);
                }
                arguments.push(decode_codec_scalar(
                    *type_id,
                    reader,
                    arena,
                    catalog,
                    runtime,
                    depth + 1,
                )?);
            }
            if next_codec_event(reader)? != MirCodecEvent::UnionEnd {
                return Err(MirCodecError::MalformedInput);
            }
            Ok(MirValue::Union {
                union: adapter.target().symbol(),
                case,
                arguments,
            })
        }
        _ => Err(MirCodecError::MalformedInput),
    }
}

fn decode_codec_scalar<R: RuntimeAdapter>(
    type_id: TypeId,
    reader: &MirCodecReader,
    arena: &TypeArena,
    catalog: &[MirGeneratedCodecAdapter],
    runtime: &mut R,
    depth: u8,
) -> Result<MirValue, MirCodecError> {
    if depth > MAX_CODEC_NESTING_DEPTH {
        return Err(MirCodecError::LimitExceeded);
    }
    if let Some(adapter) = catalog
        .iter()
        .find(|adapter| adapter.target_type() == type_id)
    {
        return decode_codec_value(adapter, reader, arena, catalog, runtime, depth);
    }
    let event = next_codec_event(reader)?;
    match (arena.get(type_id), event) {
        (Some(SemanticType::Tuple(types)), MirCodecEvent::TupleStart(count))
            if usize::from(count) == types.len() =>
        {
            let mut values = Vec::with_capacity(types.len());
            for (index, type_id) in types.iter().enumerate() {
                if next_codec_event(reader)?
                    != MirCodecEvent::Element(
                        u16::try_from(index).map_err(|_| MirCodecError::LimitExceeded)?,
                    )
                {
                    return Err(MirCodecError::MalformedInput);
                }
                values.push(decode_codec_scalar(
                    *type_id,
                    reader,
                    arena,
                    catalog,
                    runtime,
                    depth + 1,
                )?);
            }
            if next_codec_event(reader)? != MirCodecEvent::TupleEnd {
                return Err(MirCodecError::MalformedInput);
            }
            Ok(MirValue::Tuple(values))
        }
        (Some(SemanticType::Array(element)), MirCodecEvent::SequenceStart(count)) => {
            decode_codec_sequence(*element, count, reader, arena, catalog, runtime, depth + 1)
                .map(MirValue::Array)
        }
        (
            Some(SemanticType::Builtin {
                definition,
                arguments,
            }),
            MirCodecEvent::SequenceStart(count),
        ) if definition.raw() == 101 && arguments.len() == 1 => decode_codec_sequence(
            arguments[0],
            count,
            reader,
            arena,
            catalog,
            runtime,
            depth + 1,
        )
        .map(MirValue::List),
        (Some(SemanticType::Union(types)), MirCodecEvent::OptionalAbsent)
            if optional_payload_type(types, arena).is_some() =>
        {
            Ok(MirValue::Nil)
        }
        (Some(SemanticType::Union(types)), MirCodecEvent::OptionalPresent) => {
            let payload =
                optional_payload_type(types, arena).ok_or(MirCodecError::MalformedInput)?;
            decode_codec_scalar(payload, reader, arena, catalog, runtime, depth + 1)
        }
        (Some(SemanticType::Primitive(PrimitiveType::Boolean)), MirCodecEvent::Boolean(value)) => {
            Ok(MirValue::Boolean(value))
        }
        (Some(SemanticType::Primitive(PrimitiveType::String)), MirCodecEvent::String(value)) => {
            Ok(MirValue::String(value))
        }
        (
            Some(SemanticType::Builtin {
                definition,
                arguments,
            }),
            MirCodecEvent::Bytes(bytes),
        ) if definition.raw() == 0 && arguments.is_empty() => runtime
            .allocate_immutable_bytes(&bytes)
            .map(MirValue::Bytes)
            .map_err(|_| MirCodecError::CapabilityFailure),
        (
            Some(SemanticType::Primitive(PrimitiveType::Integer(kind))),
            MirCodecEvent::Integer(value),
        ) if value.kind() == *kind => Ok(MirValue::Integer(value)),
        (Some(SemanticType::Primitive(PrimitiveType::Float32)), MirCodecEvent::Float(value))
            if value.kind() == FloatKind::Float32 =>
        {
            Ok(MirValue::Float(value))
        }
        (Some(SemanticType::Primitive(PrimitiveType::Float64)), MirCodecEvent::Float(value))
            if value.kind() == FloatKind::Float64 =>
        {
            Ok(MirValue::Float(value))
        }
        _ => Err(MirCodecError::MalformedInput),
    }
}

fn decode_codec_sequence<R: RuntimeAdapter>(
    element: TypeId,
    count: u32,
    reader: &MirCodecReader,
    arena: &TypeArena,
    catalog: &[MirGeneratedCodecAdapter],
    runtime: &mut R,
    depth: u8,
) -> Result<Vec<MirValue>, MirCodecError> {
    let count = usize::try_from(count).map_err(|_| MirCodecError::LimitExceeded)?;
    if count > MAX_CODEC_SEQUENCE_ELEMENTS || depth > MAX_CODEC_NESTING_DEPTH {
        return Err(MirCodecError::LimitExceeded);
    }
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        if next_codec_event(reader)?
            != MirCodecEvent::Element(
                u16::try_from(index).map_err(|_| MirCodecError::LimitExceeded)?,
            )
        {
            return Err(MirCodecError::MalformedInput);
        }
        values.push(decode_codec_scalar(
            element, reader, arena, catalog, runtime, depth,
        )?);
    }
    if next_codec_event(reader)? != MirCodecEvent::SequenceEnd {
        return Err(MirCodecError::MalformedInput);
    }
    Ok(values)
}

fn optional_payload_type(types: &[TypeId], arena: &TypeArena) -> Option<TypeId> {
    if types.len() != 2
        || !types.iter().any(|type_id| {
            arena.get(*type_id) == Some(&SemanticType::Primitive(PrimitiveType::Nil))
        })
    {
        return None;
    }
    types
        .iter()
        .copied()
        .find(|type_id| arena.get(*type_id) != Some(&SemanticType::Primitive(PrimitiveType::Nil)))
}

fn next_codec_event(reader: &MirCodecReader) -> Result<MirCodecEvent, MirCodecError> {
    let position = reader.position.get();
    let event = reader
        .events
        .get(position)
        .cloned()
        .ok_or(MirCodecError::MalformedInput)?;
    reader.position.set(position + 1);
    Ok(event)
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    use pop_foundation::{BubbleId, BuiltinTypeId, EnumCaseId, FieldId, ModuleId, UnionCaseId};
    use pop_mir::{MirGeneratedCodecAdapter, MirGeneratedCodecMember, MirGeneratedCodecMemberId};
    use pop_resolve::Visibility;

    fn adapter(
        target: SymbolId,
        target_type: TypeId,
        members: Vec<MirGeneratedCodecMember>,
    ) -> MirGeneratedCodecAdapter {
        MirGeneratedCodecAdapter::new(
            SymbolId::from_raw(target.raw() + 10),
            SymbolIdentity::new(BubbleId::from_raw(0), target),
            ModuleId::from_raw(0),
            Visibility::Public,
            "ValueSchema".to_owned(),
            "Value".to_owned(),
            target_type,
            TypeId::from_raw(999),
            1,
            "0".repeat(64),
            members,
        )
    }

    #[test]
    fn generated_codec_record_enum_union_events_round_trip_and_reject_tamper() {
        let mut arena = TypeArena::new();
        let mut runtime = ReferenceRuntimeAdapter::default();
        let text = arena.source_type("String").expect("String");
        let integer = arena.source_type("Int").expect("Int");
        let record_type = arena
            .intern(SemanticType::Record(vec![
                ("name".to_owned(), text),
                ("age".to_owned(), integer),
            ]))
            .expect("record type");
        let record = adapter(
            SymbolId::from_raw(1),
            record_type,
            vec![
                MirGeneratedCodecMember::new(
                    0,
                    "name".to_owned(),
                    MirGeneratedCodecMemberId::Field(FieldId::from_raw(0)),
                    vec![text],
                    None,
                ),
                MirGeneratedCodecMember::new(
                    1,
                    "age".to_owned(),
                    MirGeneratedCodecMemberId::Field(FieldId::from_raw(1)),
                    vec![integer],
                    None,
                ),
            ],
        );
        let value = MirValue::Record {
            record: SymbolId::from_raw(1),
            fields: vec![
                (FieldId::from_raw(0), MirValue::String("Ada".to_owned())),
                (
                    FieldId::from_raw(1),
                    MirValue::Integer(
                        IntegerValue::parse_decimal("42", IntegerKind::Int64).expect("Int"),
                    ),
                ),
            ],
        };
        let mut events = Vec::new();
        encode_codec_value(
            &record,
            &value,
            &mut events,
            &arena,
            std::slice::from_ref(&record),
            &mut runtime,
            0,
        )
        .expect("encode record");
        assert_eq!(
            decode_codec_value(
                &record,
                &MirCodecReader::new(events.clone()),
                &arena,
                std::slice::from_ref(&record),
                &mut runtime,
                0,
            ),
            Ok(value)
        );
        let MirCodecEvent::Member { label, .. } = &mut events[1] else {
            panic!("member")
        };
        *label = "wrong".to_owned();
        assert_eq!(
            decode_codec_value(
                &record,
                &MirCodecReader::new(events),
                &arena,
                std::slice::from_ref(&record),
                &mut runtime,
                0,
            ),
            Err(MirCodecError::MalformedInput)
        );

        let enumeration = adapter(
            SymbolId::from_raw(2),
            TypeId::from_raw(700),
            vec![MirGeneratedCodecMember::new(
                0,
                "Ready".to_owned(),
                MirGeneratedCodecMemberId::EnumCase(EnumCaseId::from_raw(0)),
                Vec::new(),
                Some(7),
            )],
        );
        let enum_value = MirValue::Enum {
            definition: SymbolId::from_raw(2),
            case: EnumCaseId::from_raw(0),
            discriminant: 7,
        };
        let mut enum_events = Vec::new();
        encode_codec_value(
            &enumeration,
            &enum_value,
            &mut enum_events,
            &arena,
            std::slice::from_ref(&enumeration),
            &mut runtime,
            0,
        )
        .expect("encode enum");
        assert_eq!(
            decode_codec_value(
                &enumeration,
                &MirCodecReader::new(enum_events),
                &arena,
                std::slice::from_ref(&enumeration),
                &mut runtime,
                0,
            ),
            Ok(enum_value)
        );

        let union = adapter(
            SymbolId::from_raw(3),
            TypeId::from_raw(701),
            vec![MirGeneratedCodecMember::new(
                0,
                "Named".to_owned(),
                MirGeneratedCodecMemberId::UnionCase(UnionCaseId::from_raw(0)),
                vec![text],
                None,
            )],
        );
        let union_value = MirValue::Union {
            union: SymbolId::from_raw(3),
            case: UnionCaseId::from_raw(0),
            arguments: vec![MirValue::String("Pop".to_owned())],
        };
        let mut union_events = Vec::new();
        encode_codec_value(
            &union,
            &union_value,
            &mut union_events,
            &arena,
            std::slice::from_ref(&union),
            &mut runtime,
            0,
        )
        .expect("encode union");
        assert_eq!(
            decode_codec_value(
                &union,
                &MirCodecReader::new(union_events),
                &arena,
                std::slice::from_ref(&union),
                &mut runtime,
                0,
            ),
            Ok(union_value)
        );
    }

    #[test]
    fn generated_codec_recurses_through_exact_nested_catalog_and_closed_containers() {
        let mut arena = TypeArena::new();
        let integer = arena.source_type("Int").expect("Int");
        let text = arena.source_type("String").expect("String");
        let nil = arena.source_type("nil").expect("nil");
        let array = arena.intern(SemanticType::Array(integer)).expect("array");
        let list = arena
            .intern(SemanticType::Builtin {
                definition: BuiltinTypeId::from_raw(101),
                arguments: vec![integer],
            })
            .expect("list");
        let optional = arena
            .intern(SemanticType::Union(vec![nil, text]))
            .expect("optional");
        let tuple = arena
            .intern(SemanticType::Tuple(vec![array, list, optional]))
            .expect("tuple");
        let inner_type = arena
            .intern(SemanticType::Record(vec![("items".to_owned(), tuple)]))
            .expect("inner record");
        let outer_type = arena
            .intern(SemanticType::Record(vec![("inner".to_owned(), inner_type)]))
            .expect("outer record");
        let inner = adapter(
            SymbolId::from_raw(20),
            inner_type,
            vec![MirGeneratedCodecMember::new(
                0,
                "items".to_owned(),
                MirGeneratedCodecMemberId::Field(FieldId::from_raw(20)),
                vec![tuple],
                None,
            )],
        );
        let outer = adapter(
            SymbolId::from_raw(21),
            outer_type,
            vec![MirGeneratedCodecMember::new(
                0,
                "inner".to_owned(),
                MirGeneratedCodecMemberId::Field(FieldId::from_raw(21)),
                vec![inner_type],
                None,
            )],
        );
        let number = || {
            MirValue::Integer(IntegerValue::parse_decimal("7", IntegerKind::Int64).expect("Int"))
        };
        let value = MirValue::Record {
            record: SymbolId::from_raw(21),
            fields: vec![(
                FieldId::from_raw(21),
                MirValue::Record {
                    record: SymbolId::from_raw(20),
                    fields: vec![(
                        FieldId::from_raw(20),
                        MirValue::Tuple(vec![
                            MirValue::Array(vec![number()]),
                            MirValue::List(vec![number()]),
                            MirValue::String("Pop".to_owned()),
                        ]),
                    )],
                },
            )],
        };
        let catalog = vec![outer.clone(), inner];
        let mut runtime = ReferenceRuntimeAdapter::default();
        let mut events = Vec::new();
        encode_codec_value(
            &outer,
            &value,
            &mut events,
            &arena,
            &catalog,
            &mut runtime,
            0,
        )
        .expect("encode nested value");
        assert!(events.contains(&MirCodecEvent::TupleStart(3)));
        assert!(events.contains(&MirCodecEvent::OptionalPresent));
        assert_eq!(
            decode_codec_value(
                &outer,
                &MirCodecReader::new(events),
                &arena,
                &catalog,
                &mut runtime,
                0,
            ),
            Ok(value)
        );
    }

    #[test]
    fn generated_codec_bytes_and_sequence_limits_are_typed() {
        let mut arena = TypeArena::new();
        let bytes_type = arena
            .intern(SemanticType::Builtin {
                definition: BuiltinTypeId::from_raw(0),
                arguments: Vec::new(),
            })
            .expect("Bytes");
        let integer = arena.source_type("Int").expect("Int");
        let array = arena.intern(SemanticType::Array(integer)).expect("array");
        let mut runtime = ReferenceRuntimeAdapter::default();
        let reference = runtime
            .allocate_immutable_bytes(&[0, 1, 255])
            .expect("allocate Bytes");
        let mut events = Vec::new();
        encode_codec_scalar(
            bytes_type,
            &MirValue::Bytes(reference),
            &mut events,
            &arena,
            &[],
            &mut runtime,
            0,
        )
        .expect("encode Bytes");
        assert_eq!(events, vec![MirCodecEvent::Bytes(vec![0, 1, 255])]);
        let decoded = decode_codec_scalar(
            bytes_type,
            &MirCodecReader::new(events),
            &arena,
            &[],
            &mut runtime,
            0,
        )
        .expect("decode Bytes");
        let MirValue::Bytes(decoded) = decoded else {
            panic!("decoded Bytes")
        };
        let mut payload = [0; 3];
        runtime
            .immutable_bytes_read(decoded, 0, &mut payload)
            .expect("read Bytes");
        assert_eq!(payload, [0, 1, 255]);

        assert_eq!(
            decode_codec_scalar(
                array,
                &MirCodecReader::new(vec![MirCodecEvent::SequenceStart(65_536)]),
                &arena,
                &[],
                &mut runtime,
                0,
            ),
            Err(MirCodecError::LimitExceeded)
        );

        let value =
            MirValue::Integer(IntegerValue::parse_decimal("7", IntegerKind::Int64).expect("Int"));
        let mut bounded_events = Vec::new();
        assert_eq!(
            encode_codec_scalar(
                array,
                &MirValue::Array(vec![value; 32_768]),
                &mut bounded_events,
                &arena,
                &[],
                &mut runtime,
                0,
            ),
            Err(MirCodecError::LimitExceeded)
        );
        assert_eq!(bounded_events.len(), MAX_CODEC_EVENTS);
        let writer = crate::values::MirCodecWriter::new();
        assert!(writer.append_within_limit(vec![MirCodecEvent::Boolean(false)], MAX_CODEC_EVENTS));
        assert_eq!(
            writer.events(),
            vec![MirCodecEvent::Boolean(false)],
            "over-limit temporary events must never replace committed tape"
        );
        assert!(writer.append_within_limit(vec![MirCodecEvent::Boolean(true)], MAX_CODEC_EVENTS));
        assert_eq!(
            writer.events(),
            vec![MirCodecEvent::Boolean(false), MirCodecEvent::Boolean(true)]
        );

        assert_eq!(
            encode_codec_scalar(
                array,
                &MirValue::Array(vec![MirValue::Nil; 65_536]),
                &mut Vec::new(),
                &arena,
                &[],
                &mut runtime,
                0,
            ),
            Err(MirCodecError::LimitExceeded)
        );
    }
}

fn runtime_callback_site(
    owner: SymbolId,
    site: MirFfiCallbackSiteId,
) -> Result<FfiCallbackSiteId, ExecutionError> {
    FfiCallbackSiteId::new((u64::from(owner.raw()) << 32) | u64::from(site.raw()))
        .ok_or(ExecutionError::InvalidControlFlow)
}

fn require_foreign_abi_values(
    mir: &MirBubble,
    arena: &TypeArena,
    expected: &[TypeId],
    layouts: &[Option<pop_runtime_interface::FfiAbiLayoutId>],
    values: &[MirValue],
) -> Result<(), ExecutionError> {
    if expected.len() != values.len() || layouts.len() != values.len() {
        return Err(ExecutionError::WrongArity);
    }
    for ((expected, layout), value) in expected.iter().zip(layouts).zip(values) {
        let matches = if let Some(layout) = layout {
            let layout = mir
                .ffi_layouts()
                .get(*layout)
                .filter(|layout| layout.element() == *expected)
                .ok_or(ExecutionError::InvalidControlFlow)?;
            foreign_layout_value_matches(mir, arena, layout, value)?
        } else {
            foreign_scalar_value_matches(mir, arena, *expected, value)?
        };
        if !matches {
            return Err(ExecutionError::TypeMismatch);
        }
    }
    Ok(())
}

fn foreign_scalar_value_matches(
    mir: &MirBubble,
    arena: &TypeArena,
    expected: TypeId,
    value: &MirValue,
) -> Result<bool, ExecutionError> {
    Ok(match arena.get(expected) {
        Some(SemanticType::Primitive(PrimitiveType::Integer(kind))) => {
            matches!(value, MirValue::Integer(integer) if integer.kind() == *kind)
        }
        Some(SemanticType::Primitive(PrimitiveType::Float32)) => {
            matches!(value, MirValue::Float(float) if float.kind() == FloatKind::Float32)
        }
        Some(SemanticType::Primitive(PrimitiveType::Float64)) => {
            matches!(value, MirValue::Float(float) if float.kind() == FloatKind::Float64)
        }
        Some(SemanticType::Builtin { definition, .. })
            if is_ffi_integer_abi_builtin_type(*definition) =>
        {
            let kind = integer_kind_for_type(expected, mir.ffi_layouts(), arena)?;
            matches!(value, MirValue::Integer(integer) if integer.kind() == kind)
        }
        Some(SemanticType::Builtin { definition, .. })
            if is_ffi_pointer_type_constructor(*definition) =>
        {
            matches!(value, MirValue::FfiPointer(_))
                || (*definition == FFI_OPTIONAL_POINTER_TYPE_ID
                    || *definition == FFI_OPTIONAL_READ_ONLY_POINTER_TYPE_ID)
                    && matches!(value, MirValue::Nil)
        }
        Some(SemanticType::Builtin { definition, .. })
            if is_ffi_function_type_constructor(*definition) =>
        {
            matches!(value, MirValue::FfiFunction(_))
                || definition.raw() == 203 && matches!(value, MirValue::Nil)
        }
        Some(SemanticType::Builtin { definition, .. }) if *definition == FFI_HANDLE_TYPE_ID => {
            matches!(value, MirValue::FfiHandle(handle) if *handle != 0)
        }
        Some(SemanticType::Builtin {
            definition,
            arguments,
        }) if *definition == FFI_CALLBACK_CONTEXT_TYPE_ID && arguments.is_empty() => {
            matches!(value, MirValue::FfiPointer(_))
        }
        _ => return Err(ExecutionError::InvalidControlFlow),
    })
}

fn callback_abi_value_matches(
    mir: &MirBubble,
    arena: &TypeArena,
    expected: TypeId,
    value: &MirValue,
) -> Result<bool, ExecutionError> {
    let mut layouts = mir
        .ffi_layouts()
        .entries()
        .iter()
        .filter(|layout| layout.element() == expected);
    let first = layouts.next();
    if layouts.next().is_some() {
        return Err(ExecutionError::InvalidControlFlow);
    }
    match first {
        Some(layout) => foreign_layout_value_matches(mir, arena, layout, value),
        None => foreign_scalar_value_matches(mir, arena, expected, value),
    }
}

fn foreign_layout_value_matches(
    mir: &MirBubble,
    arena: &TypeArena,
    layout: &MirFfiLayout,
    value: &MirValue,
) -> Result<bool, ExecutionError> {
    Ok(match layout.value_class() {
        MirFfiValueClass::Integer => {
            let kind = integer_kind_for_type(layout.element(), mir.ffi_layouts(), arena)?;
            matches!(value, MirValue::Integer(integer) if integer.kind() == kind)
        }
        MirFfiValueClass::Float => match layout.size() {
            4 => matches!(value, MirValue::Float(float) if float.kind() == FloatKind::Float32),
            8 => matches!(value, MirValue::Float(float) if float.kind() == FloatKind::Float64),
            _ => return Err(ExecutionError::InvalidControlFlow),
        },
        MirFfiValueClass::Pointer
        | MirFfiValueClass::FunctionPointer
        | MirFfiValueClass::Handle => {
            foreign_scalar_value_matches(mir, arena, layout.element(), value)?
        }
        MirFfiValueClass::Record(plan) => {
            let Some(expected_record) =
                mir.declarations()
                    .iter()
                    .find_map(|declaration| match declaration.kind() {
                        pop_mir::MirDeclarationKind::Record(record)
                            if record.type_id() == layout.element() =>
                        {
                            Some(declaration.symbol())
                        }
                        _ => None,
                    })
            else {
                return Err(ExecutionError::InvalidControlFlow);
            };
            let MirValue::Record { record, fields } = value else {
                return Ok(false);
            };
            if *record != expected_record || fields.len() != plan.len() {
                return Ok(false);
            }
            for field in plan {
                let Some(value) = fields
                    .iter()
                    .find_map(|(identity, value)| (*identity == field.field()).then_some(value))
                else {
                    return Ok(false);
                };
                let child = mir
                    .ffi_layouts()
                    .get(field.layout())
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                if !foreign_layout_value_matches(mir, arena, child, value)? {
                    return Ok(false);
                }
            }
            true
        }
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    maximum_steps: u64,
    maximum_call_depth: u32,
}

impl ExecutionLimits {
    #[must_use]
    pub const fn new(maximum_steps: u64, maximum_call_depth: u32) -> Self {
        Self {
            maximum_steps,
            maximum_call_depth,
        }
    }
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self::new(1_000_000, 256)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    UnknownFunction(SymbolId),
    UnsupportedForeignFunction(SymbolId),
    UnsupportedFfiCallback {
        function: u64,
        context: ForeignAddress,
    },
    UnknownReferencedFunction(SymbolIdentity),
    WrongArity,
    TypeMismatch,
    MissingValue(ValueId),
    IntegerOverflow,
    DivisionByZero,
    NumericConversion,
    Runtime(RuntimeFailure),
    StepLimit,
    CallDepthLimit,
    ReachedUnreachable,
    InvalidControlFlow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForeignAdapterRegistrationError {
    UnknownForeignFunction(SymbolId),
    SignatureMismatch(SymbolId),
    Duplicate(SymbolId),
}

pub trait FfiCallbackInvoker {
    /// Invokes one exact function/context pair published by the interpreter.
    ///
    /// # Errors
    ///
    /// Rejects unavailable, stale, closed, mismatched, or ill-typed pairs
    /// before managed callback execution.
    fn invoke(
        &mut self,
        function: &MirValue,
        context: &MirValue,
        arguments: &[MirValue],
    ) -> Result<Vec<MirValue>, ExecutionError>;
}

type ForeignAdapterFunction = dyn FnMut(&[MirValue], &mut dyn FfiCallbackInvoker) -> Result<Vec<MirValue>, ExecutionError>
    + 'static;

pub struct TypedForeignAdapter {
    symbol: SymbolId,
    parameters: Vec<TypeId>,
    results: Vec<TypeId>,
    function: Box<ForeignAdapterFunction>,
}

impl TypedForeignAdapter {
    #[must_use]
    pub fn new<F>(
        symbol: SymbolId,
        parameters: Vec<TypeId>,
        results: Vec<TypeId>,
        function: F,
    ) -> Self
    where
        F: FnMut(&[MirValue]) -> Result<Vec<MirValue>, RuntimeFailure> + 'static,
    {
        let mut function = function;
        Self {
            symbol,
            parameters,
            results,
            function: Box::new(move |arguments, _| {
                function(arguments).map_err(ExecutionError::Runtime)
            }),
        }
    }

    /// Creates an exact foreign adapter that may synchronously invoke a
    /// compiler-proven callback function/context pair.
    #[must_use]
    pub fn new_with_callbacks<F>(
        symbol: SymbolId,
        parameters: Vec<TypeId>,
        results: Vec<TypeId>,
        function: F,
    ) -> Self
    where
        F: FnMut(&[MirValue], &mut dyn FfiCallbackInvoker) -> Result<Vec<MirValue>, ExecutionError>
            + 'static,
    {
        Self {
            symbol,
            parameters,
            results,
            function: Box::new(function),
        }
    }
}

pub struct MirInterpreter<'mir, R = ReferenceRuntimeAdapter> {
    mir: &'mir MirBubble,
    arena: &'mir TypeArena,
    limits: ExecutionLimits,
    runtime: RefCell<R>,
    foreign_adapters: RefCell<BTreeMap<SymbolId, TypedForeignAdapter>>,
    ffi_callbacks: RefCell<BTreeMap<FfiCallbackRegistrationId, InterpreterCallback>>,
}

impl<'mir> MirInterpreter<'mir, ReferenceRuntimeAdapter> {
    /// Accepts only MIR that passes the canonical verifier.
    ///
    /// # Errors
    ///
    /// Returns every verifier failure before execution can begin.
    pub fn new(
        mir: &'mir MirBubble,
        arena: &'mir TypeArena,
    ) -> Result<Self, Vec<MirVerificationError>> {
        verify_mir_bubble(mir, arena)?;
        Ok(Self {
            mir,
            arena,
            limits: ExecutionLimits::default(),
            runtime: RefCell::new(ReferenceRuntimeAdapter::default()),
            foreign_adapters: RefCell::new(BTreeMap::new()),
            ffi_callbacks: RefCell::new(BTreeMap::new()),
        })
    }
}

impl<'mir, R: RuntimeAdapter> MirInterpreter<'mir, R> {
    /// Accepts verified MIR with an explicitly selected PLRI adapter.
    ///
    /// # Errors
    ///
    /// Returns all canonical MIR verification failures before retaining the
    /// runtime adapter.
    pub fn with_runtime(
        mir: &'mir MirBubble,
        arena: &'mir TypeArena,
        runtime: R,
    ) -> Result<Self, Vec<MirVerificationError>> {
        verify_mir_bubble(mir, arena)?;
        Ok(Self {
            mir,
            arena,
            limits: ExecutionLimits::default(),
            runtime: RefCell::new(runtime),
            foreign_adapters: RefCell::new(BTreeMap::new()),
            ffi_callbacks: RefCell::new(BTreeMap::new()),
        })
    }

    #[must_use]
    pub const fn with_limits(mut self, limits: ExecutionLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Installs one test-only foreign adapter after matching its exact resolved
    /// symbol and static parameter/result packs.
    ///
    /// # Errors
    ///
    /// Rejects unknown identities, signature drift, and duplicate authority.
    pub fn with_foreign_adapter(
        mut self,
        adapter: TypedForeignAdapter,
    ) -> Result<Self, ForeignAdapterRegistrationError> {
        let Some(declaration) = self
            .mir
            .foreign_functions()
            .iter()
            .find(|declaration| declaration.symbol() == adapter.symbol)
        else {
            return Err(ForeignAdapterRegistrationError::UnknownForeignFunction(
                adapter.symbol,
            ));
        };
        if declaration.parameters() != adapter.parameters
            || declaration.results() != adapter.results
        {
            return Err(ForeignAdapterRegistrationError::SignatureMismatch(
                adapter.symbol,
            ));
        }
        if self
            .foreign_adapters
            .get_mut()
            .contains_key(&adapter.symbol)
        {
            return Err(ForeignAdapterRegistrationError::Duplicate(
                declaration.symbol(),
            ));
        }
        self.foreign_adapters
            .get_mut()
            .insert(adapter.symbol, adapter);
        Ok(self)
    }

    #[must_use]
    pub fn runtime(&self) -> Ref<'_, R> {
        self.runtime.borrow()
    }

    /// Calls one MIR function by its already-resolved stable symbol.
    ///
    /// # Errors
    ///
    /// Returns deterministic type, arithmetic, control-flow, or resource
    /// failures. It never performs runtime lookup from a source string.
    pub fn call(
        &self,
        function: SymbolId,
        arguments: &[MirValue],
    ) -> Result<Vec<MirValue>, ExecutionError> {
        let arguments: Vec<_> = arguments
            .iter()
            .cloned()
            .map(RuntimeValue::visible)
            .collect();
        let mut runtime = self.runtime.borrow_mut();
        let mut foreign_adapters = self.foreign_adapters.borrow_mut();
        let mut ffi_callbacks = self.ffi_callbacks.borrow_mut();
        Engine {
            mir: self.mir,
            arena: self.arena,
            limits: self.limits,
            steps: 0,
            depth: 0,
            runtime: &mut *runtime,
            foreign_adapters: &mut foreign_adapters,
            root_handles: BTreeMap::new(),
            ffi_handles: BTreeMap::new(),
            ffi_buffer_borrows: BTreeMap::new(),
            ffi_bytes_borrows: BTreeMap::new(),
            ffi_callbacks: &mut ffi_callbacks,
            pin_handles: BTreeMap::new(),
            private_values: BTreeMap::new(),
            next_private_value: u32::MAX,
            active_captures: None,
            active_task: None,
        }
        .call(function, &arguments)
        .map(|values| {
            values
                .into_iter()
                .map(|value| value.observed_visible())
                .collect()
        })
    }
}

struct Engine<'mir, 'runtime, R> {
    mir: &'mir MirBubble,
    arena: &'mir TypeArena,
    limits: ExecutionLimits,
    steps: u64,
    depth: u32,
    runtime: &'runtime mut R,
    foreign_adapters: &'runtime mut BTreeMap<SymbolId, TypedForeignAdapter>,
    root_handles: BTreeMap<ValueId, RootHandle>,
    ffi_handles: BTreeMap<RootHandle, RuntimeValue>,
    ffi_buffer_borrows: BTreeMap<BorrowRegionId, FfiBufferBorrowId>,
    ffi_bytes_borrows: BTreeMap<BorrowRegionId, FfiBytesBorrowState>,
    ffi_callbacks: &'runtime mut BTreeMap<FfiCallbackRegistrationId, InterpreterCallback>,
    pin_handles: BTreeMap<ValueId, PinHandle>,
    private_values: BTreeMap<SymbolId, PrivateValue>,
    next_private_value: u32,
    active_captures: Option<Rc<RefCell<Vec<RuntimeValue>>>>,
    active_task: Option<TaskId>,
}

#[derive(Clone, Copy)]
struct FfiBytesBorrowState {
    owner: ManagedReference,
    borrow: FfiBytesBorrowId,
    length: u64,
}

#[derive(Clone)]
struct InterpreterCallback {
    registration: FfiCallbackRegistration,
    site: FfiCallbackSiteId,
    target: InterpreterCallbackTarget,
    environment: ManagedReference,
    closed: bool,
}

#[derive(Clone)]
enum InterpreterCallbackTarget {
    Closure {
        owner: SymbolId,
        function: NestedFunctionId,
        captures: Rc<RefCell<Vec<RuntimeValue>>>,
    },
}

enum PrivateValue {
    Cell(Rc<RefCell<RuntimeValue>>),
    Closure {
        owner: SymbolId,
        function: NestedFunctionId,
        captures: Rc<RefCell<Vec<RuntimeValue>>>,
    },
    Iterator {
        source: RuntimeValue,
        expected_length: usize,
        position: usize,
        range_current: Option<pop_types::IntegerValue>,
        range_started: bool,
    },
    Task(Rc<RefCell<TaskState>>),
    CancellationSource(Rc<RefCell<CancellationState>>),
    CancellationToken(Rc<RefCell<CancellationState>>),
    TaskGroup(Rc<RefCell<InterpreterTaskGroup>>),
    Channel(Rc<RefCell<ChannelLifecycle<InterpreterChannelValue>>>),
    Actor(Rc<RefCell<ActorLifecycle<InterpreterChannelValue>>>),
    AtomicInt(AtomicInt),
    AtomicBoolean(AtomicBoolean),
    TcpListener(TcpListener),
    TcpStream(TcpStream),
    FileAccess(PathBuf),
    DirectoryAccess(PathBuf),
    FileHandle(std::fs::File),
    FileWriteHandle(std::fs::File),
    DirectorySnapshot(Vec<String>),
    TlsClientConfig(Arc<ClientConfig>),
    TlsServerConfig(Arc<ServerConfig>),
    TlsClientStream(rustls::StreamOwned<ClientConnection, TcpStream>),
    TlsServerStream(rustls::StreamOwned<ServerConnection, TcpStream>),
    UdpSocket(UdpSocket),
    DnsResolver,
    DnsAnswers(Vec<IpAddr>),
    NetInterfaces(Vec<InterpreterInterface>),
    NetRoutes(Vec<InterpreterRoute>),
    #[cfg(unix)]
    UnixListener(UnixListener),
    #[cfg(unix)]
    UnixStream(UnixStream),
    MonotonicClock(Instant),
    LiveDeadline {
        clock: SymbolId,
        target: Instant,
    },
}

#[derive(Clone, Debug)]
struct InterpreterChannelValue {
    value: RuntimeValue,
    root: Option<RootHandle>,
}

#[derive(Clone)]
struct CancellationState {
    token: CancellationTokenId,
    requested: bool,
}

struct InterpreterTaskGroup {
    lifecycle: TaskGroupLifecycle,
    cancellation: Rc<RefCell<CancellationState>>,
    children: BTreeMap<TaskId, SymbolId>,
    reference: ManagedReference,
}

#[derive(Clone)]
enum TaskTarget {
    Direct(SymbolId),
    Referenced(SymbolIdentity),
    Indirect(RuntimeValue),
    Group { body: RuntimeValue, group: SymbolId },
}

#[derive(Clone)]
struct TaskState {
    lifecycle: TaskLifecycle,
    completion_type: TypeId,
    execution: TaskExecution,
}

#[derive(Clone)]
enum TaskExecution {
    Created {
        target: TaskTarget,
        arguments: Vec<RuntimeValue>,
        owner: pop_runtime_interface::ManagedReference,
        completion_slot: ObjectSlot,
    },
    Running,
    Completed(Result<RuntimeValue, ExecutionError>),
}

impl<R: RuntimeAdapter> Engine<'_, '_, R> {
    fn call(
        &mut self,
        symbol: SymbolId,
        arguments: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let function = self
            .mir
            .functions()
            .iter()
            .find(|function| function.symbol() == symbol)
            .ok_or(ExecutionError::UnknownFunction(symbol))?;
        if function.parameters().len() != arguments.len() {
            return Err(ExecutionError::WrongArity);
        }
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::CallDepthLimit)?;
        if self.depth > self.limits.maximum_call_depth {
            return Err(ExecutionError::CallDepthLimit);
        }
        let result = self.execute(
            function.parameters(),
            function.results(),
            function.blocks(),
            arguments,
            None,
        );
        self.depth -= 1;
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_foreign_call(
        &mut self,
        symbol: SymbolId,
        arguments: &[ValueId],
        roots: &[ValueId],
        safe_point: pop_runtime_interface::SafePointId,
        effects: pop_mir::MirEffectSummary,
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let declaration = self
            .mir
            .foreign_functions()
            .iter()
            .find(|declaration| declaration.symbol() == symbol)
            .ok_or(ExecutionError::UnsupportedForeignFunction(symbol))?;
        let visible_arguments = arguments
            .iter()
            .map(|argument| value(values, *argument).map(|value| value.visible.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        require_foreign_abi_values(
            self.mir,
            self.arena,
            declaration.parameters(),
            declaration.parameter_layouts(),
            &visible_arguments,
        )?;
        if !self.foreign_adapters.contains_key(&symbol) {
            return Err(ExecutionError::UnsupportedForeignFunction(symbol));
        }
        let published_values = roots
            .iter()
            .map(|root| value(values, *root).map(|value| value.reference))
            .collect::<Result<Vec<_>, _>>()?;
        let stack_map = StackMap::new(
            safe_point,
            (0..roots.len())
                .map(|slot| {
                    u32::try_from(slot)
                        .map(RootSlot::new)
                        .map_err(|_| ExecutionError::InvalidControlFlow)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| ExecutionError::InvalidControlFlow)?;
        let mut publication = RootPublication::new(stack_map, published_values)
            .map_err(|_| ExecutionError::InvalidControlFlow)?;
        let mode = if effects.contains(pop_mir::MirEffect::Blocks) {
            ForeignCallMode::Blocking
        } else {
            ForeignCallMode::BoundedNonblocking
        };
        let transition = self
            .runtime
            .enter_foreign(&mut publication, mode)
            .map_err(ExecutionError::Runtime)?;
        let mut adapter = self
            .foreign_adapters
            .remove(&symbol)
            .ok_or(ExecutionError::UnsupportedForeignFunction(symbol))?;
        let invocation = (adapter.function)(&visible_arguments, self);
        if self.foreign_adapters.insert(symbol, adapter).is_some() {
            return Err(ExecutionError::InvalidControlFlow);
        }
        self.runtime
            .leave_foreign(transition, &mut publication)
            .map_err(ExecutionError::Runtime)?;
        install_published_relocations(roots, &publication, values)?;
        let returned = invocation?;
        require_foreign_abi_values(
            self.mir,
            self.arena,
            declaration.results(),
            declaration.result_layouts(),
            &returned,
        )?;
        Ok(returned.into_iter().map(RuntimeValue::visible).collect())
    }

    fn execute(
        &mut self,
        parameters: &[TypeId],
        results: &[TypeId],
        blocks: &[pop_mir::MirBlock],
        arguments: &[RuntimeValue],
        captures: Option<Rc<RefCell<Vec<RuntimeValue>>>>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        require_runtime_numeric_types(self.arena, parameters, arguments)?;
        let previous_captures = std::mem::replace(&mut self.active_captures, captures);
        let result = self.execute_blocks(results, blocks, arguments);
        self.active_captures = previous_captures;
        result
    }

    fn execute_blocks(
        &mut self,
        results: &[TypeId],
        blocks: &[pop_mir::MirBlock],
        arguments: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let mut values = BTreeMap::new();
        let entry = blocks.first().ok_or(ExecutionError::InvalidControlFlow)?;
        for (argument, value) in entry.arguments().iter().zip(arguments) {
            values.insert(argument.value(), value.clone());
        }
        let mut block_index = 0_usize;
        let mut pending_unwind = None;
        loop {
            self.step()?;
            let block = blocks
                .get(block_index)
                .ok_or(ExecutionError::InvalidControlFlow)?;
            let mut unwound_to_cleanup = None;
            for instruction in block.instructions() {
                self.step()?;
                let evaluated = if instruction.has_result() {
                    self.evaluate_instruction(instruction, &mut values)
                        .map(Some)
                } else {
                    self.evaluate_effect_instruction(instruction, &mut values)
                        .map(|()| None)
                };
                match evaluated {
                    Ok(Some(value)) => {
                        values.insert(instruction.result(), value);
                    }
                    Ok(None) => {}
                    Err(ExecutionError::Runtime(RuntimeFailure::Unwind(reason))) => {
                        if pending_unwind.is_some() {
                            return Err(ExecutionError::Runtime(self.runtime.begin_panic(
                                pop_runtime_interface::PanicPayload::new(
                                    pop_runtime_interface::PanicKind::DoublePanic,
                                ),
                            )));
                        }
                        if let Some(target) = call_cleanup_target(instruction) {
                            pending_unwind = Some(reason);
                            unwound_to_cleanup = Some(target.raw() as usize);
                            break;
                        }
                        return Err(ExecutionError::Runtime(RuntimeFailure::Unwind(reason)));
                    }
                    Err(error) => return Err(error),
                }
            }
            if let Some(cleanup) = unwound_to_cleanup {
                block_index = cleanup;
                continue;
            }
            self.step()?;
            match block.terminator() {
                MirTerminator::Branch { target, arguments } => {
                    Self::assign_block_arguments(blocks, *target, arguments, &mut values)?;
                    block_index = target.raw() as usize;
                }
                MirTerminator::ConditionalBranch {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let target = match &value(&values, *condition)?.visible {
                        MirValue::Boolean(true) => *when_true,
                        MirValue::Boolean(false) => *when_false,
                        _ => return Err(ExecutionError::TypeMismatch),
                    };
                    block_index = target.raw() as usize;
                }
                MirTerminator::UnionSwitch {
                    scrutinee,
                    union,
                    arms,
                } => {
                    let MirValue::Union {
                        union: value_union,
                        case,
                        arguments,
                    } = value(&values, *scrutinee)?.visible.clone()
                    else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    if value_union != *union {
                        return Err(ExecutionError::TypeMismatch);
                    }
                    let arm = arms
                        .iter()
                        .find(|arm| arm.case() == case)
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                    Self::assign_runtime_block_arguments(
                        blocks,
                        arm.target(),
                        &arguments,
                        &mut values,
                    )?;
                    block_index = arm.target().raw() as usize;
                }
                MirTerminator::ErrorSwitch {
                    scrutinee,
                    error,
                    arms,
                } => {
                    let MirValue::Error {
                        error: value_error,
                        case,
                        arguments,
                    } = value(&values, *scrutinee)?.visible.clone()
                    else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    if value_error != *error {
                        return Err(ExecutionError::TypeMismatch);
                    }
                    let arm = arms
                        .iter()
                        .find(|arm| arm.case() == case)
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                    Self::assign_runtime_block_arguments(
                        blocks,
                        arm.target(),
                        &arguments,
                        &mut values,
                    )?;
                    block_index = arm.target().raw() as usize;
                }
                MirTerminator::CodecErrorSwitch { scrutinee, arms } => {
                    let MirValue::CodecError(error) = &value(&values, *scrutinee)?.visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    let arm = arms
                        .iter()
                        .find(|arm| arm.case() == error.case())
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                    Self::assign_runtime_block_arguments(blocks, arm.target(), &[], &mut values)?;
                    block_index = arm.target().raw() as usize;
                }
                MirTerminator::Return { values: returned } => {
                    let returned: Vec<_> = returned
                        .iter()
                        .map(|value_id| value(&values, *value_id).cloned())
                        .collect::<Result<_, _>>()?;
                    require_runtime_numeric_types(self.arena, results, &returned)?;
                    return Ok(returned);
                }
                MirTerminator::Trap(trap) => {
                    return Err(ExecutionError::Runtime(self.runtime.raise_trap(*trap)));
                }
                MirTerminator::Panic(payload) => {
                    if pending_unwind.is_some() {
                        return Err(ExecutionError::Runtime(self.runtime.begin_panic(
                            pop_runtime_interface::PanicPayload::new(
                                pop_runtime_interface::PanicKind::DoublePanic,
                            ),
                        )));
                    }
                    return Err(ExecutionError::Runtime(
                        self.runtime.begin_panic(payload.clone()),
                    ));
                }
                MirTerminator::ContinueUnwind(reason) => {
                    if pending_unwind.is_some() {
                        return Err(ExecutionError::Runtime(self.runtime.begin_panic(
                            pop_runtime_interface::PanicPayload::new(
                                pop_runtime_interface::PanicKind::DoublePanic,
                            ),
                        )));
                    }
                    return Err(ExecutionError::Runtime(RuntimeFailure::Unwind(
                        reason.clone(),
                    )));
                }
                MirTerminator::ResumeUnwind => {
                    let reason = pending_unwind
                        .take()
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                    return Err(ExecutionError::Runtime(RuntimeFailure::Unwind(reason)));
                }
                MirTerminator::Suspend {
                    operation: MirSuspendOperation::Task { task, result_type },
                    resume,
                    cancellation,
                    cancellation_mode,
                    unwind,
                    live_frame,
                    ..
                } => {
                    if *cancellation_mode == MirCancellationMode::Observe
                        && self.active_cancellation_observation(false)
                            == CancellationObservation::Requested
                    {
                        pending_unwind = None;
                        block_index = cancellation.raw() as usize;
                        continue;
                    }
                    self.publish_suspend_frame(live_frame, &mut values)?;
                    let task = value(&values, *task)?.clone();
                    match self.await_task(&task, *result_type) {
                        Ok(completion) => {
                            let resume_block = blocks
                                .get(resume.raw() as usize)
                                .ok_or(ExecutionError::InvalidControlFlow)?;
                            let [argument] = resume_block.arguments() else {
                                return Err(ExecutionError::WrongArity);
                            };
                            values.insert(argument.value(), completion);
                            block_index = resume.raw() as usize;
                        }
                        Err(ExecutionError::Runtime(RuntimeFailure::Unwind(
                            pop_runtime_interface::UnwindReason::Cancellation,
                        ))) => {
                            pending_unwind = None;
                            block_index = cancellation.raw() as usize;
                        }
                        Err(ExecutionError::Runtime(RuntimeFailure::Unwind(reason))) => {
                            if let MirUnwindAction::Cleanup(target) = unwind {
                                pending_unwind = Some(reason);
                                block_index = target.raw() as usize;
                            } else {
                                return Err(ExecutionError::Runtime(RuntimeFailure::Unwind(
                                    reason,
                                )));
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }
                MirTerminator::Unreachable => return Err(ExecutionError::ReachedUnreachable),
                MirTerminator::Missing => return Err(ExecutionError::InvalidControlFlow),
            }
        }
    }

    fn active_cancellation_observation(&self, masked: bool) -> CancellationObservation {
        let Some(task) = self.active_task else {
            return CancellationObservation::Active;
        };
        self.private_values
            .values()
            .find_map(|value| match value {
                PrivateValue::Task(state) if state.borrow().lifecycle.id() == task => {
                    Some(state.borrow().lifecycle.cancellation_observation(masked))
                }
                _ => None,
            })
            .unwrap_or(CancellationObservation::Active)
    }

    fn publish_suspend_frame(
        &mut self,
        frame: &pop_mir::MirLiveFrame,
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<(), ExecutionError> {
        let roots = frame
            .stack_map()
            .root_slots()
            .iter()
            .map(|root| {
                frame
                    .slots()
                    .get(root.raw() as usize)
                    .ok_or(ExecutionError::InvalidControlFlow)
                    .and_then(|slot| value(values, slot.value()).map(|value| value.reference))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut publication = RootPublication::new(frame.stack_map().clone(), roots)
            .map_err(|_| ExecutionError::InvalidControlFlow)?;
        self.runtime
            .safe_point(&mut publication)
            .map_err(ExecutionError::Runtime)?;
        let root_values = frame
            .stack_map()
            .root_slots()
            .iter()
            .map(|root| {
                frame
                    .slots()
                    .get(root.raw() as usize)
                    .map(|slot| slot.value())
                    .ok_or(ExecutionError::InvalidControlFlow)
            })
            .collect::<Result<Vec<_>, _>>()?;
        install_published_relocations(&root_values, &publication, values)?;
        Ok(())
    }

    fn await_task(
        &mut self,
        task: &RuntimeValue,
        expected_completion_type: TypeId,
    ) -> Result<RuntimeValue, ExecutionError> {
        let MirValue::Task(task) = &task.visible else {
            return Err(ExecutionError::TypeMismatch);
        };
        let state = match self.private_values.get(task) {
            Some(PrivateValue::Task(state)) => state.clone(),
            _ => return Err(ExecutionError::InvalidControlFlow),
        };
        let (target, arguments, completion_type, owner, completion_slot) = {
            let mut state = state.borrow_mut();
            let completion_type = state.completion_type;
            match state.execution.clone() {
                TaskExecution::Completed(result) => return result,
                TaskExecution::Running => return Err(ExecutionError::InvalidControlFlow),
                TaskExecution::Created {
                    target,
                    arguments,
                    owner,
                    completion_slot,
                } => {
                    let created = (target, arguments, completion_type, owner, completion_slot);
                    if state.lifecycle.state() == RuntimeTaskState::Created {
                        state
                            .lifecycle
                            .start(TaskOwner::DirectAwait {
                                parent: self.active_task,
                            })
                            .map_err(|_| ExecutionError::InvalidControlFlow)?;
                    } else if !matches!(state.lifecycle.owner(), Some(TaskOwner::Group(_))) {
                        return Err(ExecutionError::InvalidControlFlow);
                    }
                    state
                        .lifecycle
                        .begin_poll()
                        .map_err(|_| ExecutionError::InvalidControlFlow)?;
                    state.execution = TaskExecution::Running;
                    created
                }
            }
        };
        if completion_type != expected_completion_type {
            let result = Err(ExecutionError::TypeMismatch);
            let mut state = state.borrow_mut();
            state
                .lifecycle
                .finish_poll(TaskPollCompletion::Panicked)
                .map_err(|_| ExecutionError::InvalidControlFlow)?;
            state.execution = TaskExecution::Completed(result.clone());
            return result;
        }
        let active_task = state.borrow().lifecycle.id();
        let previous_active_task = self.active_task.replace(active_task);
        let mut result = match target {
            TaskTarget::Direct(function) => self.call(function, &arguments),
            TaskTarget::Referenced(function) => {
                Err(ExecutionError::UnknownReferencedFunction(function))
            }
            TaskTarget::Indirect(callee) => self.execute_indirect_value(&callee, &arguments),
            TaskTarget::Group { body, group } => self
                .execute_task_group(&body, group, completion_type)
                .map(|completion| vec![completion]),
        }
        .and_then(|returned| self.task_completion(completion_type, returned));
        self.active_task = previous_active_task;
        if let Ok(completion) = &result
            && let Some(reference) = completion.reference
            && let Err(failure) = self.runtime.write_barrier(WriteBarrier::new(
                BarrierKind::CombinedSatbGenerational,
                owner,
                completion_slot,
                None,
                Some(reference),
            ))
        {
            result = Err(ExecutionError::Runtime(failure));
        }
        let completion = match &result {
            Ok(_) => TaskPollCompletion::Completed,
            Err(ExecutionError::Runtime(RuntimeFailure::Unwind(
                pop_runtime_interface::UnwindReason::Cancellation,
            ))) => TaskPollCompletion::Cancelled,
            Err(_) => TaskPollCompletion::Panicked,
        };
        let mut state = state.borrow_mut();
        state
            .lifecycle
            .finish_poll(completion)
            .map_err(|_| ExecutionError::InvalidControlFlow)?;
        debug_assert!(matches!(
            state.lifecycle.state(),
            RuntimeTaskState::Completed | RuntimeTaskState::Cancelled | RuntimeTaskState::Panicked
        ));
        state.execution = TaskExecution::Completed(result.clone());
        result
    }

    fn execute_task_group(
        &mut self,
        body: &RuntimeValue,
        group_symbol: SymbolId,
        completion_type: TypeId,
    ) -> Result<RuntimeValue, ExecutionError> {
        let group = match self.private_values.get(&group_symbol) {
            Some(PrivateValue::TaskGroup(group)) => group.clone(),
            _ => return Err(ExecutionError::InvalidControlFlow),
        };
        let group_value = {
            let group = group.borrow();
            RuntimeValue::managed(MirValue::TaskGroup(group_symbol), group.reference)
        };
        let body_result = self
            .execute_indirect_value(body, &[group_value])
            .and_then(|returned| self.task_completion(completion_type, returned));
        let exit = match &body_result {
            Ok(_) => TaskGroupExit::BodyCompleted,
            Err(ExecutionError::Runtime(RuntimeFailure::Unwind(UnwindReason::Cancellation))) => {
                TaskGroupExit::Cancelled
            }
            Err(ExecutionError::Runtime(RuntimeFailure::Unwind(UnwindReason::Panic(_)))) => {
                TaskGroupExit::BodyPanicked
            }
            Err(_) => TaskGroupExit::BodyFailed,
        };
        let children = group
            .borrow_mut()
            .lifecycle
            .begin_close(exit)
            .map_err(|_| ExecutionError::InvalidControlFlow)?;
        let mut child_failure = None;
        for child_id in children {
            let child_symbol = group
                .borrow()
                .children
                .get(&child_id)
                .copied()
                .ok_or(ExecutionError::InvalidControlFlow)?;
            let child_state = match self.private_values.get(&child_symbol) {
                Some(PrivateValue::Task(child)) => child.clone(),
                _ => return Err(ExecutionError::InvalidControlFlow),
            };
            let (completion_type, child_value) = {
                let mut child = child_state.borrow_mut();
                let token = group.borrow().lifecycle.cancellation_token();
                if !child.lifecycle.state().terminal() {
                    let _ = child.lifecycle.request_cancellation(token);
                }
                let reference = match &child.execution {
                    TaskExecution::Created { owner, .. } => *owner,
                    TaskExecution::Running | TaskExecution::Completed(_) => {
                        group.borrow().reference
                    }
                };
                (
                    child.completion_type,
                    RuntimeValue::managed(MirValue::Task(child_symbol), reference),
                )
            };
            let outcome = self.await_task(&child_value, completion_type);
            if child_failure.is_none() {
                child_failure = outcome.err();
            }
            group
                .borrow_mut()
                .lifecycle
                .join_child(&child_state.borrow().lifecycle)
                .map_err(|_| ExecutionError::InvalidControlFlow)?;
        }
        group
            .borrow_mut()
            .lifecycle
            .complete_close()
            .map_err(|_| ExecutionError::InvalidControlFlow)?;
        match body_result {
            Err(error) => Err(error),
            Ok(_) if child_failure.is_some() => Err(child_failure.expect("checked child failure")),
            Ok(completion) => Ok(completion),
        }
    }

    fn task_completion(
        &mut self,
        result_type: TypeId,
        mut returned: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, ExecutionError> {
        if returned.len() == 1 {
            return Ok(returned.remove(0));
        }
        let reference_slots = returned
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .reference
                    .map(|_| ObjectSlot::new(u32::try_from(index).unwrap_or(u32::MAX)))
            })
            .collect();
        let object_map = ObjectMap::new(
            u32::try_from(returned.len()).unwrap_or(u32::MAX),
            reference_slots,
        )
        .map_err(|_| ExecutionError::InvalidControlFlow)?;
        let reference = self
            .runtime
            .allocate_object(&ObjectAllocationRequest::new(
                RuntimeTypeId::new(result_type.raw()),
                AllocationClass::NurseryEligible,
                object_map,
            ))
            .map_err(ExecutionError::Runtime)?;
        Ok(RuntimeValue::managed(
            MirValue::Tuple(returned.into_iter().map(|value| value.visible).collect()),
            reference,
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn evaluate_instruction(
        &mut self,
        instruction: &MirInstruction,
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<RuntimeValue, ExecutionError> {
        if let Some(result) = self.evaluate_structured_instruction(instruction, values)? {
            return Ok(result);
        }
        match evaluate_numeric_instruction(instruction.kind(), values) {
            Ok(Some(result)) => return Ok(RuntimeValue::visible(result)),
            Ok(None) => {}
            Err(ExecutionError::IntegerOverflow) => {
                return Err(ExecutionError::Runtime(
                    self.runtime
                        .raise_trap(Trap::new(TrapKind::IntegerOverflow)),
                ));
            }
            Err(ExecutionError::DivisionByZero) => {
                return Err(ExecutionError::Runtime(
                    self.runtime.raise_trap(Trap::new(TrapKind::DivisionByZero)),
                ));
            }
            Err(ExecutionError::NumericConversion) => {
                return Err(ExecutionError::Runtime(
                    self.runtime
                        .raise_trap(Trap::new(TrapKind::NumericConversion)),
                ));
            }
            Err(error) => return Err(error),
        }
        let result = match instruction.kind() {
            MirInstructionKind::TaskCreate {
                dispatch,
                arguments,
                completion_type,
                object_map,
            } => {
                let arguments = evaluated_arguments(arguments, values)?;
                let target = match dispatch {
                    MirTaskDispatch::Direct(function) => TaskTarget::Direct(*function),
                    MirTaskDispatch::Referenced(function) => TaskTarget::Referenced(*function),
                    MirTaskDispatch::Indirect(callee) => {
                        let callee = value(values, *callee)?.clone();
                        if !matches!(callee.visible, MirValue::Function(_)) {
                            return Err(ExecutionError::TypeMismatch);
                        }
                        TaskTarget::Indirect(callee)
                    }
                };
                let mut stored = arguments.clone();
                if let TaskTarget::Indirect(callee) = &target {
                    stored.insert(0, callee.clone());
                }
                if stored.iter().enumerate().any(|(index, value)| {
                    value.reference.is_some()
                        && !object_map.is_reference_slot(ObjectSlot::new(
                            u32::try_from(index).unwrap_or(u32::MAX),
                        ))
                }) {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map.clone(),
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let completion_slot = object_map
                    .slot_count()
                    .checked_sub(1)
                    .map(ObjectSlot::new)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let task = self.fresh_private_symbol();
                self.private_values.insert(
                    task,
                    PrivateValue::Task(Rc::new(RefCell::new(TaskState {
                        lifecycle: TaskLifecycle::created(TaskId::new(u64::from(task.raw()))),
                        completion_type: *completion_type,
                        execution: TaskExecution::Created {
                            target,
                            arguments,
                            owner: reference,
                            completion_slot,
                        },
                    }))),
                );
                return Ok(RuntimeValue::managed(MirValue::Task(task), reference));
            }
            MirInstructionKind::CancelSourceCreate => {
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        ObjectMap::new(0, Vec::new())
                            .map_err(|_| ExecutionError::InvalidControlFlow)?,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let source = self.fresh_private_symbol();
                let cancellation = Rc::new(RefCell::new(CancellationState {
                    token: CancellationTokenId::new(u64::from(source.raw())),
                    requested: false,
                }));
                self.private_values
                    .insert(source, PrivateValue::CancellationSource(cancellation));
                return Ok(RuntimeValue::managed(
                    MirValue::CancellationSource(source),
                    reference,
                ));
            }
            MirInstructionKind::CancelSourceToken { source } => {
                let source = value(values, *source)?.clone();
                let MirValue::CancellationSource(source_symbol) = source.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let cancellation = match self.private_values.get(&source_symbol) {
                    Some(PrivateValue::CancellationSource(cancellation)) => cancellation.clone(),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                let token = self.fresh_private_symbol();
                self.private_values
                    .insert(token, PrivateValue::CancellationToken(cancellation));
                return Ok(RuntimeValue {
                    visible: MirValue::CancellationToken(token),
                    reference: source.reference,
                    shared_visible: None,
                });
            }
            MirInstructionKind::CancelRequest { source } => {
                let MirValue::CancellationSource(source) = value(values, *source)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let cancellation = match self.private_values.get(&source) {
                    Some(PrivateValue::CancellationSource(cancellation)) => cancellation.clone(),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                let token = {
                    let mut cancellation = cancellation.borrow_mut();
                    cancellation.requested = true;
                    cancellation.token
                };
                let tasks = self
                    .private_values
                    .values()
                    .filter_map(|value| match value {
                        PrivateValue::Task(task) => Some(task.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                for task in tasks {
                    let mut task = task.borrow_mut();
                    if task.lifecycle.cancellation_token() == Some(token)
                        && !task.lifecycle.state().terminal()
                    {
                        let _ = task.lifecycle.request_cancellation(token);
                    }
                }
                MirValue::Nil
            }
            MirInstructionKind::TaskGroupCreate {
                cancel,
                body,
                completion_type,
                object_map,
            } => {
                let cancel = value(values, *cancel)?.clone();
                let body = value(values, *body)?.clone();
                let MirValue::CancellationToken(token_symbol) = cancel.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if !matches!(body.visible, MirValue::Function(_)) {
                    return Err(ExecutionError::TypeMismatch);
                }
                let cancellation = match self.private_values.get(&token_symbol) {
                    Some(PrivateValue::CancellationToken(cancellation)) => cancellation.clone(),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                for (index, stored) in [&cancel, &body].into_iter().enumerate() {
                    if stored.reference.is_some()
                        && !object_map.is_reference_slot(ObjectSlot::new(
                            u32::try_from(index).unwrap_or(u32::MAX),
                        ))
                    {
                        return Err(ExecutionError::InvalidControlFlow);
                    }
                }
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map.clone(),
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let group_symbol = self.fresh_private_symbol();
                let group_id = TaskGroupId::new(u64::from(group_symbol.raw()));
                let token = cancellation.borrow().token;
                self.private_values.insert(
                    group_symbol,
                    PrivateValue::TaskGroup(Rc::new(RefCell::new(InterpreterTaskGroup {
                        lifecycle: TaskGroupLifecycle::open(group_id, token),
                        cancellation: cancellation.clone(),
                        children: BTreeMap::new(),
                        reference,
                    }))),
                );
                let task_symbol = self.fresh_private_symbol();
                let mut lifecycle =
                    TaskLifecycle::created(TaskId::new(u64::from(task_symbol.raw())));
                lifecycle
                    .bind_cancellation_token(token)
                    .map_err(|_| ExecutionError::InvalidControlFlow)?;
                if cancellation.borrow().requested {
                    lifecycle
                        .request_cancellation(token)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?;
                }
                let completion_slot = object_map
                    .slot_count()
                    .checked_sub(1)
                    .map(ObjectSlot::new)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.private_values.insert(
                    task_symbol,
                    PrivateValue::Task(Rc::new(RefCell::new(TaskState {
                        lifecycle,
                        completion_type: *completion_type,
                        execution: TaskExecution::Created {
                            target: TaskTarget::Group {
                                body,
                                group: group_symbol,
                            },
                            arguments: Vec::new(),
                            owner: reference,
                            completion_slot,
                        },
                    }))),
                );
                return Ok(RuntimeValue::managed(
                    MirValue::Task(task_symbol),
                    reference,
                ));
            }
            MirInstructionKind::TaskStart { group, task } => {
                let task_value = value(values, *task)?.clone();
                let MirValue::TaskGroup(group_symbol) = value(values, *group)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Task(task_symbol) = task_value.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let group = match self.private_values.get(&group_symbol) {
                    Some(PrivateValue::TaskGroup(group)) => group.clone(),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                let task = match self.private_values.get(&task_symbol) {
                    Some(PrivateValue::Task(task)) => task.clone(),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                {
                    let mut group = group.borrow_mut();
                    let mut task = task.borrow_mut();
                    group
                        .lifecycle
                        .start_child(&mut task.lifecycle)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?;
                    group.children.insert(task.lifecycle.id(), task_symbol);
                    if group.cancellation.borrow().requested {
                        let token = group.lifecycle.cancellation_token();
                        task.lifecycle
                            .request_cancellation(token)
                            .map_err(|_| ExecutionError::InvalidControlFlow)?;
                    }
                }
                return Ok(task_value);
            }
            MirInstructionKind::StringConstant(value) => MirValue::String(value.clone()),
            MirInstructionKind::StringConcat { left, right } => {
                let MirValue::String(left) = &value(values, *left)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::String(right) = &value(values, *right)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let mut result = String::with_capacity(left.len().saturating_add(right.len()));
                result.push_str(left);
                result.push_str(right);
                MirValue::String(result)
            }
            MirInstructionKind::ViewCreate { kind, lender, .. } => {
                let (lender, byte_length, scalar_length) =
                    match (kind, &value(values, *lender)?.visible) {
                        (pop_mir::MirViewKind::Bytes, MirValue::Bytes(reference)) => {
                            let length = self
                                .runtime
                                .immutable_bytes_length(*reference)
                                .map_err(ExecutionError::Runtime)?;
                            let length = usize::try_from(length)
                                .map_err(|_| ExecutionError::InvalidControlFlow)?;
                            (MirViewLenderValue::Bytes(*reference), length, length)
                        }
                        (pop_mir::MirViewKind::Text, MirValue::String(text)) => (
                            MirViewLenderValue::Text(Rc::from(text.as_str())),
                            text.len(),
                            text.chars().count(),
                        ),
                        _ => return Err(ExecutionError::TypeMismatch),
                    };
                MirValue::View(MirViewValue {
                    kind: *kind,
                    lender,
                    byte_offset: 0,
                    byte_length,
                    scalar_length,
                })
            }
            MirInstructionKind::ViewSlice {
                kind,
                view,
                start,
                length,
                ..
            } => {
                let MirValue::View(parent) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if parent.kind != *kind {
                    return Err(ExecutionError::TypeMismatch);
                }
                let start = integer_i64(&value(values, *start)?.visible)?;
                let length = integer_i64(&value(values, *length)?.visible)?;
                let owner_length = match kind {
                    pop_mir::MirViewKind::Bytes => parent.byte_length,
                    pop_mir::MirViewKind::Text => parent.scalar_length,
                };
                let (relative_start, selected_length) =
                    checked_view_range(owner_length, start, length)
                        .ok_or_else(|| self.bounds_violation())?;
                let (byte_start, byte_length) = match kind {
                    pop_mir::MirViewKind::Bytes => (relative_start, selected_length),
                    pop_mir::MirViewKind::Text => {
                        let text = view_text(parent)?;
                        let start = scalar_byte_offset(text, relative_start)
                            .ok_or(ExecutionError::InvalidControlFlow)?;
                        let end = scalar_byte_offset(
                            text,
                            relative_start.saturating_add(selected_length),
                        )
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                        (start, end - start)
                    }
                };
                MirValue::View(MirViewValue {
                    kind: *kind,
                    lender: parent.lender.clone(),
                    byte_offset: parent
                        .byte_offset
                        .checked_add(byte_start)
                        .ok_or_else(|| self.integer_overflow())?,
                    byte_length,
                    scalar_length: match kind {
                        pop_mir::MirViewKind::Bytes => byte_length,
                        pop_mir::MirViewKind::Text => selected_length,
                    },
                })
            }
            MirInstructionKind::ViewLength { kind, view } => {
                let MirValue::View(view) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if view.kind != *kind {
                    return Err(ExecutionError::TypeMismatch);
                }
                let length = match kind {
                    pop_mir::MirViewKind::Bytes => view.byte_length,
                    pop_mir::MirViewKind::Text => view.scalar_length,
                };
                MirValue::Integer(
                    IntegerValue::parse_decimal(&length.to_string(), IntegerKind::Int64)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::ViewGetByte { view, index } => {
                let MirValue::View(view) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if view.kind != pop_mir::MirViewKind::Bytes {
                    return Err(ExecutionError::TypeMismatch);
                }
                let index = integer_i64(&value(values, *index)?.visible)?;
                let Some(relative) = index
                    .checked_sub(1)
                    .and_then(|index| usize::try_from(index).ok())
                    .filter(|index| *index < view.byte_length)
                else {
                    return Ok(RuntimeValue::visible(MirValue::Nil));
                };
                let reference = view_bytes_reference(view)?;
                let mut byte = [0_u8; 1];
                let offset = view
                    .byte_offset
                    .checked_add(relative)
                    .and_then(|offset| u64::try_from(offset).ok())
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.runtime
                    .immutable_bytes_read(reference, offset, &mut byte)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Integer(
                    IntegerValue::parse_decimal(&byte[0].to_string(), IntegerKind::UInt8)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::ViewGetRune { view, index } => {
                let MirValue::View(view) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if view.kind != pop_mir::MirViewKind::Text {
                    return Err(ExecutionError::TypeMismatch);
                }
                let index = integer_i64(&value(values, *index)?.visible)?;
                let Some(relative) = index
                    .checked_sub(1)
                    .and_then(|index| usize::try_from(index).ok())
                else {
                    return Ok(RuntimeValue::visible(MirValue::Nil));
                };
                view_text(view)?
                    .chars()
                    .nth(relative)
                    .map_or(MirValue::Nil, |value| MirValue::Rune(u32::from(value)))
            }
            MirInstructionKind::ViewMaterialize { kind, view, .. } => {
                let MirValue::View(view) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if view.kind != *kind {
                    return Err(ExecutionError::TypeMismatch);
                }
                match kind {
                    pop_mir::MirViewKind::Text => MirValue::String(view_text(view)?.to_owned()),
                    pop_mir::MirViewKind::Bytes => {
                        let reference = view_bytes_reference(view)?;
                        let mut bytes = vec![0_u8; view.byte_length];
                        self.runtime
                            .immutable_bytes_read(
                                reference,
                                u64::try_from(view.byte_offset)
                                    .map_err(|_| ExecutionError::InvalidControlFlow)?,
                                &mut bytes,
                            )
                            .map_err(ExecutionError::Runtime)?;
                        let reference = self
                            .runtime
                            .allocate_immutable_bytes(&bytes)
                            .map_err(ExecutionError::Runtime)?;
                        MirValue::Bytes(reference)
                    }
                }
            }
            MirInstructionKind::Utf8Encode { view, .. } => {
                let MirValue::View(view) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if view.kind != pop_mir::MirViewKind::Text {
                    return Err(ExecutionError::TypeMismatch);
                }
                let reference = self
                    .runtime
                    .allocate_immutable_bytes(view_text(view)?.as_bytes())
                    .map_err(ExecutionError::Runtime)?;
                return Ok(RuntimeValue::managed(MirValue::Bytes(reference), reference));
            }
            MirInstructionKind::Utf8DecodeView { view, .. } => {
                let MirValue::View(view) = &value(values, *view)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if view.kind != pop_mir::MirViewKind::Bytes {
                    return Err(ExecutionError::TypeMismatch);
                }
                let reference = view_bytes_reference(view)?;
                let mut bytes = vec![0_u8; view.byte_length];
                self.runtime
                    .immutable_bytes_read(
                        reference,
                        u64::try_from(view.byte_offset)
                            .map_err(|_| ExecutionError::InvalidControlFlow)?,
                        &mut bytes,
                    )
                    .map_err(ExecutionError::Runtime)?;
                String::from_utf8(bytes).map_or(MirValue::Nil, MirValue::String)
            }
            MirInstructionKind::Utf8DecodeBuffer { buffer, .. } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .byte_buffer_length(buffer)
                    .and_then(|length| {
                        usize::try_from(length).map_err(|_| RuntimeFailure::runtime_invariant())
                    })
                    .map_err(ExecutionError::Runtime)?;
                let mut bytes = vec![0_u8; length];
                self.runtime
                    .byte_buffer_read(buffer, 0, &mut bytes)
                    .map_err(ExecutionError::Runtime)?;
                String::from_utf8(bytes).map_or(MirValue::Nil, MirValue::String)
            }
            MirInstructionKind::RuneFromCodePoint { value: code_point } => {
                let MirValue::Integer(value) = value(values, *code_point)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if value.kind() != IntegerKind::UInt32 {
                    return Err(ExecutionError::TypeMismatch);
                }
                value
                    .unsigned()
                    .and_then(|value| u32::try_from(value).ok())
                    .and_then(char::from_u32)
                    .map_or(MirValue::Nil, |value| MirValue::Rune(u32::from(value)))
            }
            MirInstructionKind::RuneCodePoint { value: rune } => {
                let MirValue::Rune(value) = value(values, *rune)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                MirValue::Integer(
                    IntegerValue::parse_decimal(&value.to_string(), IntegerKind::UInt32)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::StringFormat {
                kind,
                value: operand,
            } => {
                let operand = &value(values, *operand)?.visible;
                let formatted = match (kind, operand) {
                    (pop_types::StringFormatKind::Boolean, MirValue::Boolean(value)) => {
                        value.to_string()
                    }
                    (pop_types::StringFormatKind::Integer(expected), MirValue::Integer(value))
                        if expected == &value.kind() =>
                    {
                        value.to_string()
                    }
                    (pop_types::StringFormatKind::Float(expected), MirValue::Float(value))
                        if expected == &value.kind() =>
                    {
                        value.format_string()
                    }
                    _ => return Err(ExecutionError::TypeMismatch),
                };
                MirValue::String(formatted)
            }
            MirInstructionKind::BooleanConstant(value) => MirValue::Boolean(*value),
            MirInstructionKind::NilConstant => MirValue::Nil,
            MirInstructionKind::FfiPointerNone => MirValue::Nil,
            MirInstructionKind::FfiPointerToOptional { pointer }
            | MirInstructionKind::FfiPointerReadOnly { pointer } => {
                let pointer = value(values, *pointer)?.visible.clone();
                if !matches!(pointer, MirValue::FfiPointer(_)) {
                    return Err(ExecutionError::TypeMismatch);
                }
                pointer
            }
            MirInstructionKind::FfiPointerIsPresent { pointer } => {
                match &value(values, *pointer)?.visible {
                    MirValue::Nil => MirValue::Boolean(false),
                    MirValue::FfiPointer(_) => MirValue::Boolean(true),
                    _ => return Err(ExecutionError::TypeMismatch),
                }
            }
            MirInstructionKind::FfiPointerRequire {
                pointer,
                result,
                success,
                failure,
            } => {
                let (case, arguments) = match &value(values, *pointer)?.visible {
                    MirValue::FfiPointer(address) => {
                        (*success, vec![MirValue::FfiPointer(*address)])
                    }
                    MirValue::Nil => (*failure, vec![MirValue::FfiNullPointerError]),
                    _ => return Err(ExecutionError::TypeMismatch),
                };
                MirValue::Result {
                    definition: *result,
                    case,
                    arguments,
                }
            }
            MirInstructionKind::OptionalMake { value: present } => {
                value(values, *present)?.visible.clone()
            }
            MirInstructionKind::OptionalIsPresent { optional } => {
                MirValue::Boolean(!matches!(value(values, *optional)?.visible, MirValue::Nil))
            }
            MirInstructionKind::OptionalGet { optional } => {
                let present = value(values, *optional)?.visible.clone();
                if matches!(present, MirValue::Nil) {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                present
            }
            MirInstructionKind::ResultIsOk { result, definition } => {
                let MirValue::Result {
                    definition: found,
                    case,
                    ..
                } = &value(values, *result)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if found != definition {
                    return Err(ExecutionError::TypeMismatch);
                }
                MirValue::Boolean(case.raw() == 0)
            }
            MirInstructionKind::ResultGetOk { result, definition }
            | MirInstructionKind::ResultGetError { result, definition } => {
                let MirValue::Result {
                    definition: found,
                    case,
                    arguments,
                } = &value(values, *result)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let expected = u32::from(matches!(
                    instruction.kind(),
                    MirInstructionKind::ResultGetError { .. }
                ));
                if found != definition || case.raw() != expected || arguments.len() != 1 {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                arguments[0].clone()
            }
            MirInstructionKind::IterationIsItem {
                iteration,
                definition,
                item_case,
                end_case,
            } => {
                let MirValue::Iteration {
                    definition: found,
                    case,
                    ..
                } = &value(values, *iteration)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if found != definition || (case != item_case && case != end_case) {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                MirValue::Boolean(case == item_case)
            }
            MirInstructionKind::IterationGetItem {
                iteration,
                definition,
                item_case,
            } => {
                let MirValue::Iteration {
                    definition: found,
                    case,
                    arguments,
                } = &value(values, *iteration)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if found != definition || case != item_case || arguments.len() != 1 {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                arguments[0].clone()
            }
            MirInstructionKind::EnumConstant {
                definition,
                case,
                discriminant,
            } => MirValue::Enum {
                definition: *definition,
                case: *case,
                discriminant: *discriminant,
            },
            MirInstructionKind::CodecErrorConstant { case } => {
                let reason = match case.raw() {
                    0 => MirCodecError::MalformedInput,
                    1 => MirCodecError::LimitExceeded,
                    2 => MirCodecError::CapabilityFailure,
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                MirValue::CodecError(reason)
            }
            MirInstructionKind::FunctionReference(function) => MirValue::Function(*function),
            MirInstructionKind::GeneratedCodecSchema(adapter) => MirValue::CodecSchema(*adapter),
            MirInstructionKind::CodecEncode {
                adapter,
                value: input,
                writer,
                result,
                success,
                failure,
            } => {
                let catalog = self.mir.generated_codec_adapters();
                let adapter = catalog
                    .iter()
                    .find(|candidate| candidate.symbol() == *adapter)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let input = &value(values, *input)?.visible;
                let MirValue::CodecWriter(writer) = &value(values, *writer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let mut events = Vec::new();
                let encoded = encode_codec_value(
                    adapter,
                    input,
                    &mut events,
                    self.arena,
                    catalog,
                    self.runtime,
                    0,
                );
                let committed = encoded.is_ok()
                    && !events.is_empty()
                    && writer.append_within_limit(events, MAX_CODEC_EVENTS);
                match encoded {
                    Ok(()) if committed => MirValue::Result {
                        definition: *result,
                        case: *success,
                        arguments: vec![MirValue::Nil],
                    },
                    Ok(()) | Err(MirCodecError::LimitExceeded) => MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::CodecError(MirCodecError::LimitExceeded)],
                    },
                    Err(error) => MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::CodecError(error)],
                    },
                }
            }
            MirInstructionKind::CodecDecode {
                adapter,
                reader,
                result,
                success,
                failure,
            } => {
                let catalog = self.mir.generated_codec_adapters();
                let adapter = catalog
                    .iter()
                    .find(|candidate| candidate.symbol() == *adapter)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let MirValue::CodecReader(reader) = &value(values, *reader)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let decoded = if reader.events.len() > MAX_CODEC_EVENTS {
                    Err(MirCodecError::LimitExceeded)
                } else {
                    decode_codec_value(adapter, reader, self.arena, catalog, self.runtime, 0)
                };
                match decoded {
                    Ok(decoded) => MirValue::Result {
                        definition: *result,
                        case: *success,
                        arguments: vec![decoded],
                    },
                    Err(MirCodecError::MalformedInput) => MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::CodecError(MirCodecError::MalformedInput)],
                    },
                    Err(error) => MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::CodecError(error)],
                    },
                }
            }
            MirInstructionKind::TupleMake { elements, .. } => {
                let tuple = MirValue::Tuple(
                    elements
                        .iter()
                        .map(|element| value(values, *element).map(RuntimeValue::observed_visible))
                        .collect::<Result<_, _>>()?,
                );
                let Some(SemanticType::Tuple(element_types)) =
                    self.arena.get(instruction.result_type())
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let references = element_types
                    .iter()
                    .enumerate()
                    .filter_map(|(index, type_id)| {
                        managed_type(self.arena, *type_id)
                            .then(|| u32::try_from(index).ok().map(ObjectSlot::new))
                            .flatten()
                    })
                    .collect();
                let object_map = ObjectMap::new(
                    u32::try_from(element_types.len()).unwrap_or(u32::MAX),
                    references,
                )
                .map_err(|_| ExecutionError::InvalidControlFlow)?;
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                return Ok(RuntimeValue::managed(tuple, reference));
            }
            MirInstructionKind::TupleGet { tuple, index } => {
                let MirValue::Tuple(elements) = &value(values, *tuple)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                elements
                    .get(*index as usize)
                    .cloned()
                    .ok_or(ExecutionError::InvalidControlFlow)?
            }
            MirInstructionKind::ArrayMake {
                elements,
                element_map,
            } => {
                let reference = self
                    .runtime
                    .allocate_array(&ArrayAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        u32::try_from(elements.len()).unwrap_or(u32::MAX),
                        *element_map,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let elements = elements
                    .iter()
                    .map(|element| value(values, *element).map(|value| value.visible.clone()))
                    .collect::<Result<_, _>>()?;
                return Ok(RuntimeValue::managed_array(elements, reference));
            }
            MirInstructionKind::ArrayCreate {
                length,
                initial_value,
                element_map,
            } => {
                let MirValue::Integer(length) = value(values, *length)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(length) = length
                    .signed()
                    .filter(|length| *length >= 0)
                    .and_then(|length| u32::try_from(length).ok())
                else {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                    ));
                };
                let reference = self
                    .runtime
                    .allocate_array(&ArrayAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        length,
                        *element_map,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let initial_value = value(values, *initial_value)?.visible.clone();
                let mut elements = Vec::new();
                elements
                    .try_reserve_exact(length as usize)
                    .map_err(|_| ExecutionError::InvalidControlFlow)?;
                elements.resize(length as usize, initial_value);
                return Ok(RuntimeValue::managed_array(elements, reference));
            }
            MirInstructionKind::TableMake {
                entries,
                key_map,
                value_map,
            } => {
                let reference = self
                    .runtime
                    .allocate_table(
                        &TableAllocationRequest::new(
                            RuntimeTypeId::new(instruction.result_type().raw()),
                            AllocationClass::NurseryEligible,
                            u32::try_from(entries.len()).unwrap_or(u32::MAX),
                            *key_map,
                            *value_map,
                        )
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                    )
                    .map_err(ExecutionError::Runtime)?;
                let visible = MirValue::Table(
                    entries
                        .iter()
                        .map(|(key, entry_value)| {
                            Ok((
                                value(values, *key)?.visible.clone(),
                                value(values, *entry_value)?.visible.clone(),
                            ))
                        })
                        .collect::<Result<_, ExecutionError>>()?,
                );
                return Ok(RuntimeValue::managed(visible, reference));
            }
            MirInstructionKind::TableGet { table, key } => {
                let (MirValue::Table(entries), key) = (
                    &value(values, *table)?.visible,
                    &value(values, *key)?.visible,
                ) else {
                    return Err(ExecutionError::TypeMismatch);
                };
                return Ok(RuntimeValue::visible(
                    entries
                        .iter()
                        .find(|(candidate, _)| candidate == key)
                        .map_or(MirValue::Nil, |(_, value)| value.clone()),
                ));
            }
            MirInstructionKind::TableSet {
                table,
                key,
                value: stored,
                ..
            } => {
                let owner = value(values, *table)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let key = value(values, *key)?.visible.clone();
                let stored = value(values, *stored)?.visible.clone();
                let mut updated = false;
                for candidate in values.values_mut() {
                    if candidate.reference != Some(owner) {
                        continue;
                    }
                    let MirValue::Table(entries) = &mut candidate.visible else {
                        continue;
                    };
                    if let Some((_, current)) = entries
                        .iter_mut()
                        .find(|(candidate_key, _)| *candidate_key == key)
                    {
                        *current = stored.clone();
                    } else {
                        entries.push((key.clone(), stored.clone()));
                    }
                    updated = true;
                }
                if !updated {
                    return Err(ExecutionError::TypeMismatch);
                }
                MirValue::Nil
            }
            MirInstructionKind::ArrayGet { array, index } => {
                let array = value(values, *array)?;
                let MirValue::Integer(index) = &value(values, *index)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let shared = array.shared_visible.as_ref().map(|value| value.borrow());
                let visible = shared.as_deref().unwrap_or(&array.visible);
                let MirValue::Array(elements) = visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if index.kind() != IntegerKind::Int64 {
                    return Err(ExecutionError::TypeMismatch);
                }
                let Some(index) = index.signed() else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(zero_based) = index
                    .checked_sub(1)
                    .and_then(|value| usize::try_from(value).ok())
                else {
                    return Ok(RuntimeValue::visible(MirValue::Nil));
                };
                return Ok(RuntimeValue::visible(
                    elements.get(zero_based).cloned().unwrap_or(MirValue::Nil),
                ));
            }
            MirInstructionKind::ArrayLength { array } => {
                let array = value(values, *array)?;
                let shared = array.shared_visible.as_ref().map(|value| value.borrow());
                let visible = shared.as_deref().unwrap_or(&array.visible);
                let MirValue::Array(elements) = visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                MirValue::Integer(
                    IntegerValue::parse_decimal(&elements.len().to_string(), IntegerKind::Int64)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::ArrayGetChecked { array, index } => {
                let array = value(values, *array)?;
                let MirValue::Integer(index) = &value(values, *index)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let shared = array.shared_visible.as_ref().map(|value| value.borrow());
                let visible = shared.as_deref().unwrap_or(&array.visible);
                let MirValue::Array(elements) = visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(zero_based) = index
                    .signed()
                    .and_then(|value| value.checked_sub(1))
                    .and_then(|value| usize::try_from(value).ok())
                else {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                    ));
                };
                let Some(element) = elements.get(zero_based).cloned() else {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                    ));
                };
                element
            }
            MirInstructionKind::ArraySet {
                array,
                index,
                value: stored,
                ..
            } => {
                let owner = value(values, *array)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let MirValue::Integer(index) = value(values, *index)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(zero_based) = index
                    .signed()
                    .and_then(|value| value.checked_sub(1))
                    .and_then(|value| usize::try_from(value).ok())
                else {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                    ));
                };
                let stored = value(values, *stored)?.visible.clone();
                let mut updated = false;
                if let Some(shared) = value(values, *array)?.shared_visible.as_ref() {
                    let mut visible = shared.borrow_mut();
                    let MirValue::Array(elements) = &mut *visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    let Some(slot) = elements.get_mut(zero_based) else {
                        return Err(ExecutionError::Runtime(
                            self.runtime
                                .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                        ));
                    };
                    *slot = stored.clone();
                    updated = true;
                }
                for candidate in values.values_mut() {
                    if candidate.shared_visible.is_some() {
                        continue;
                    }
                    if candidate.reference != Some(owner) {
                        continue;
                    }
                    let MirValue::Array(elements) = &mut candidate.visible else {
                        continue;
                    };
                    let Some(slot) = elements.get_mut(zero_based) else {
                        return Err(ExecutionError::Runtime(
                            self.runtime
                                .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                        ));
                    };
                    *slot = stored.clone();
                    updated = true;
                }
                if !updated {
                    return Err(ExecutionError::TypeMismatch);
                }
                MirValue::Nil
            }
            MirInstructionKind::ArrayFill {
                array,
                value: stored,
                ..
            } => {
                let owner = value(values, *array)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let stored = value(values, *stored)?.visible.clone();
                let mut updated = false;
                if let Some(shared) = value(values, *array)?.shared_visible.as_ref() {
                    let mut visible = shared.borrow_mut();
                    let MirValue::Array(elements) = &mut *visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    elements.fill(stored.clone());
                    updated = true;
                }
                for candidate in values.values_mut() {
                    if candidate.shared_visible.is_some() {
                        continue;
                    }
                    if candidate.reference != Some(owner) {
                        continue;
                    }
                    let MirValue::Array(elements) = &mut candidate.visible else {
                        continue;
                    };
                    elements.fill(stored.clone());
                    updated = true;
                }
                if !updated {
                    return Err(ExecutionError::TypeMismatch);
                }
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferCreate { capacity, .. } => {
                let capacity = capacity
                    .map(|capacity| {
                        let MirValue::Integer(capacity) = value(values, capacity)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        capacity
                            .signed()
                            .filter(|capacity| *capacity >= 0)
                            .and_then(|capacity| u64::try_from(capacity).ok())
                            .ok_or_else(|| self.bounds_violation())
                    })
                    .transpose()?
                    .unwrap_or(0);
                let reference = self
                    .runtime
                    .allocate_byte_buffer(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        capacity,
                    )
                    .map_err(ExecutionError::Runtime)?;
                return Ok(RuntimeValue::managed(
                    MirValue::ByteBuffer(reference),
                    reference,
                ));
            }
            MirInstructionKind::ByteBufferLength { buffer } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .byte_buffer_length(buffer)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Integer(
                    IntegerValue::parse_decimal(&length.to_string(), IntegerKind::Int64)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::ByteBufferReserve {
                buffer,
                additional_capacity,
            } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Integer(additional) = value(values, *additional_capacity)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let additional = additional
                    .signed()
                    .filter(|additional| *additional >= 0)
                    .and_then(|additional| u64::try_from(additional).ok())
                    .ok_or_else(|| self.bounds_violation())?;
                self.runtime
                    .byte_buffer_reserve(buffer, additional)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferClear { buffer } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                self.runtime
                    .byte_buffer_clear(buffer)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferWriteByte {
                buffer,
                value: written,
            } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let written = u8::try_from(integer_u64(&value(values, *written)?.visible)?)
                    .map_err(|_| ExecutionError::TypeMismatch)?;
                self.runtime
                    .byte_buffer_append(buffer, &[written])
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferWriteBytes {
                buffer,
                value: written,
            } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Bytes(written) = value(values, *written)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .immutable_bytes_length(written)
                    .map_err(ExecutionError::Runtime)?;
                self.runtime
                    .byte_buffer_append_immutable_range(buffer, written, 0, length)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferWriteView {
                buffer,
                value: written,
            } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::View(written) = &value(values, *written)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if written.kind != pop_mir::MirViewKind::Bytes {
                    return Err(ExecutionError::TypeMismatch);
                }
                self.runtime
                    .byte_buffer_append_immutable_range(
                        buffer,
                        view_bytes_reference(written)?,
                        u64::try_from(written.byte_offset)
                            .map_err(|_| ExecutionError::InvalidControlFlow)?,
                        u64::try_from(written.byte_length)
                            .map_err(|_| ExecutionError::InvalidControlFlow)?,
                    )
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferWriteInteger {
                buffer,
                value: written,
                kind,
                order,
            } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Integer(written) = value(values, *written)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if written.kind() != *kind {
                    return Err(ExecutionError::TypeMismatch);
                }
                let width = match kind {
                    IntegerKind::UInt16 => 2,
                    IntegerKind::UInt32 => 4,
                    IntegerKind::UInt64 => 8,
                    _ => return Err(ExecutionError::TypeMismatch),
                };
                let bits = written.unsigned().ok_or(ExecutionError::TypeMismatch)?;
                let bytes = match order {
                    pop_types::ByteOrder::BigEndian => bits.to_be_bytes(),
                    pop_types::ByteOrder::LittleEndian => bits.to_le_bytes(),
                };
                let written = match order {
                    pop_types::ByteOrder::BigEndian => &bytes[bytes.len() - width..],
                    pop_types::ByteOrder::LittleEndian => &bytes[..width],
                };
                self.runtime
                    .byte_buffer_append(buffer, written)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Nil
            }
            MirInstructionKind::ByteBufferMaterialize { buffer, .. } => {
                let MirValue::ByteBuffer(buffer) = value(values, *buffer)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let reference = self
                    .runtime
                    .materialize_byte_buffer(buffer)
                    .map_err(ExecutionError::Runtime)?;
                MirValue::Bytes(reference)
            }
            MirInstructionKind::ListCreate {
                capacity,
                element_map,
            } => {
                let capacity = if let Some(capacity) = capacity {
                    let MirValue::Integer(capacity) = value(values, *capacity)?.visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    let Some(capacity) = capacity
                        .signed()
                        .filter(|capacity| *capacity >= 0)
                        .and_then(|capacity| u32::try_from(capacity).ok())
                    else {
                        return Err(ExecutionError::Runtime(
                            self.runtime
                                .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                        ));
                    };
                    capacity
                } else {
                    0
                };
                let reference = self
                    .runtime
                    .allocate_table(
                        &TableAllocationRequest::new(
                            RuntimeTypeId::new(instruction.result_type().raw()),
                            AllocationClass::NurseryEligible,
                            capacity,
                            pop_runtime_interface::ArrayElementMap::Scalar,
                            *element_map,
                        )
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                    )
                    .map_err(ExecutionError::Runtime)?;
                return Ok(RuntimeValue::managed(MirValue::List(Vec::new()), reference));
            }
            MirInstructionKind::RangeCreate { first, last, step } => {
                let MirValue::Integer(first) = value(values, *first)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Integer(last) = value(values, *last)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Integer(step) = value(values, *step)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if step.signed() == Some(0) || step.unsigned() == Some(0) {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::InvalidRangeStep)),
                    ));
                }
                let object_map = ObjectMap::new(3, Vec::new())
                    .map_err(|_| ExecutionError::InvalidControlFlow)?;
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                return Ok(RuntimeValue::managed(
                    MirValue::Range { first, last, step },
                    reference,
                ));
            }
            MirInstructionKind::ListLength { list } => {
                let MirValue::List(elements) = &value(values, *list)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                MirValue::Integer(
                    IntegerValue::parse_decimal(&elements.len().to_string(), IntegerKind::Int64)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::ListGet { list, index } => {
                let (MirValue::List(elements), MirValue::Integer(index)) = (
                    &value(values, *list)?.visible,
                    &value(values, *index)?.visible,
                ) else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let zero_based = index
                    .signed()
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| usize::try_from(index).ok());
                return Ok(RuntimeValue::visible(
                    zero_based
                        .and_then(|index| elements.get(index).cloned())
                        .unwrap_or(MirValue::Nil),
                ));
            }
            MirInstructionKind::ListGetChecked { list, index } => {
                let (MirValue::List(elements), MirValue::Integer(index)) = (
                    &value(values, *list)?.visible,
                    &value(values, *index)?.visible,
                ) else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(element) = index
                    .signed()
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| usize::try_from(index).ok())
                    .and_then(|index| elements.get(index).cloned())
                else {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                    ));
                };
                element
            }
            MirInstructionKind::ListSet {
                list,
                index,
                value: stored,
                ..
            } => {
                let owner = value(values, *list)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let MirValue::Integer(index) = value(values, *index)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(zero_based) = index
                    .signed()
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| usize::try_from(index).ok())
                else {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                    ));
                };
                let stored = value(values, *stored)?.visible.clone();
                let mut updated = false;
                for candidate in values.values_mut() {
                    if candidate.reference != Some(owner) {
                        continue;
                    }
                    let MirValue::List(elements) = &mut candidate.visible else {
                        continue;
                    };
                    let Some(slot) = elements.get_mut(zero_based) else {
                        return Err(ExecutionError::Runtime(
                            self.runtime
                                .raise_trap(Trap::new(TrapKind::BoundsViolation)),
                        ));
                    };
                    *slot = stored.clone();
                    updated = true;
                }
                if !updated {
                    return Err(ExecutionError::TypeMismatch);
                }
                MirValue::Nil
            }
            MirInstructionKind::ListAdd {
                list,
                value: stored,
                ..
            } => {
                let owner = value(values, *list)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let stored = value(values, *stored)?.visible.clone();
                let mut updated = false;
                for candidate in values.values_mut() {
                    if candidate.reference != Some(owner) {
                        continue;
                    }
                    let MirValue::List(elements) = &mut candidate.visible else {
                        continue;
                    };
                    elements.push(stored.clone());
                    updated = true;
                }
                if !updated {
                    return Err(ExecutionError::TypeMismatch);
                }
                MirValue::Nil
            }
            MirInstructionKind::ChannelCreate {
                capacity,
                endpoints,
                ..
            } => {
                let MirValue::Integer(capacity) = value(values, *capacity)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(capacity) = capacity.unsigned() else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let channel = self.fresh_private_symbol();
                self.private_values.insert(
                    channel,
                    PrivateValue::Channel(Rc::new(RefCell::new(ChannelLifecycle::bounded(
                        ChannelId::new(u64::from(channel.raw())),
                        capacity,
                    )))),
                );
                let Ok(reference) = self.runtime.allocate_object(&ObjectAllocationRequest::new(
                    RuntimeTypeId::new(endpoints.raw()),
                    AllocationClass::NurseryEligible,
                    ObjectMap::new(2, Vec::new())
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )) else {
                    self.private_values.remove(&channel);
                    return Ok(RuntimeValue::visible(MirValue::Nil));
                };
                return Ok(RuntimeValue::managed(
                    MirValue::Tuple(vec![
                        MirValue::ChannelSender(channel),
                        MirValue::ChannelReceiver(channel),
                    ]),
                    reference,
                ));
            }
            MirInstructionKind::ChannelTrySend {
                sender,
                value: sent,
                ..
            } => {
                let MirValue::ChannelSender(channel) = value(values, *sender)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let channel = match self.private_values.get(&channel) {
                    Some(PrivateValue::Channel(channel)) => Rc::clone(channel),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                let sent = value(values, *sent)?.clone();
                let root = sent
                    .reference
                    .map(|reference| self.runtime.retain_root(reference))
                    .transpose()
                    .map_err(ExecutionError::Runtime)?;
                let queued = InterpreterChannelValue { value: sent, root };
                let outcome = match channel.borrow_mut().try_send(queued) {
                    Ok(()) => pop_types::ChannelSendOutcomeKind::Accepted,
                    Err(ChannelSendError::Full(unsent)) => {
                        if let Some(root) = unsent.root {
                            self.runtime
                                .release_root(root)
                                .map_err(ExecutionError::Runtime)?;
                        }
                        pop_types::ChannelSendOutcomeKind::Full
                    }
                    Err(ChannelSendError::Closed(unsent)) => {
                        if let Some(root) = unsent.root {
                            self.runtime
                                .release_root(root)
                                .map_err(ExecutionError::Runtime)?;
                        }
                        pop_types::ChannelSendOutcomeKind::Closed
                    }
                };
                MirValue::ChannelSendOutcome(outcome)
            }
            MirInstructionKind::ChannelTryReceive {
                receiver,
                element_map,
                ..
            } => {
                let MirValue::ChannelReceiver(channel) = value(values, *receiver)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let channel = match self.private_values.get(&channel) {
                    Some(PrivateValue::Channel(channel)) => Rc::clone(channel),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                let references = (*element_map
                    == pop_runtime_interface::ArrayElementMap::ManagedReference)
                    .then_some(ObjectSlot::new(1))
                    .into_iter()
                    .collect();
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        ObjectMap::new(2, references)
                            .map_err(|_| ExecutionError::InvalidControlFlow)?,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let (received, closed) = match channel.borrow_mut().try_receive() {
                    ChannelReceive::Item(mut received) => {
                        if let Some(root) = received.root {
                            let relocated = self
                                .runtime
                                .resolve_root(root)
                                .map_err(ExecutionError::Runtime)?;
                            received
                                .value
                                .install_relocated_reference(Some(relocated))?;
                            self.runtime
                                .release_root(root)
                                .map_err(ExecutionError::Runtime)?;
                        }
                        (Some(Box::new(received.value.observed_visible())), false)
                    }
                    ChannelReceive::Empty => (None, false),
                    ChannelReceive::Closed => (None, true),
                };
                return Ok(RuntimeValue::managed(
                    MirValue::ChannelReceiveOutcome {
                        value: received,
                        closed,
                    },
                    reference,
                ));
            }
            MirInstructionKind::ChannelClose {
                endpoint,
                direction,
            } => {
                let channel_symbol = match (&value(values, *endpoint)?.visible, direction) {
                    (MirValue::ChannelSender(channel), pop_types::ChannelDirection::Sender)
                    | (MirValue::ChannelReceiver(channel), pop_types::ChannelDirection::Receiver) => {
                        *channel
                    }
                    _ => return Err(ExecutionError::TypeMismatch),
                };
                let channel = match self.private_values.get(&channel_symbol) {
                    Some(PrivateValue::Channel(channel)) => Rc::clone(channel),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                let changed = match direction {
                    pop_types::ChannelDirection::Sender => channel.borrow_mut().close(),
                    pop_types::ChannelDirection::Receiver => {
                        let was_open = channel.borrow().receiver_count() != 0;
                        let discarded = channel.borrow_mut().release_receiver();
                        let changed = was_open && channel.borrow().receiver_count() == 0;
                        for discarded in discarded {
                            if let Some(root) = discarded.root {
                                self.runtime
                                    .release_root(root)
                                    .map_err(ExecutionError::Runtime)?;
                            }
                        }
                        changed
                    }
                };
                MirValue::Boolean(changed)
            }
            MirInstructionKind::ChannelSendOutcomeTest { outcome, expected } => {
                let MirValue::ChannelSendOutcome(found) = value(values, *outcome)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                MirValue::Boolean(found == *expected)
            }
            MirInstructionKind::ChannelReceiveItem { outcome, .. } => {
                let MirValue::ChannelReceiveOutcome {
                    value: received, ..
                } = &value(values, *outcome)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                received
                    .as_ref()
                    .map_or(MirValue::Nil, |received| (**received).clone())
            }
            MirInstructionKind::ChannelReceiveOutcomeTest { outcome, expected } => {
                let MirValue::ChannelReceiveOutcome {
                    value: received,
                    closed,
                } = &value(values, *outcome)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let matches = match expected {
                    pop_types::ChannelReceiveOutcomeKind::Empty => received.is_none() && !closed,
                    pop_types::ChannelReceiveOutcomeKind::Closed => received.is_none() && *closed,
                };
                MirValue::Boolean(matches)
            }
            MirInstructionKind::BooleanNot { operand } => match &value(values, *operand)?.visible {
                MirValue::Boolean(value) => MirValue::Boolean(!value),
                _ => return Err(ExecutionError::TypeMismatch),
            },
            MirInstructionKind::BooleanAnd { left, right } => {
                return boolean_binary(values, *left, *right, |left, right| left && right)
                    .map(RuntimeValue::visible);
            }
            MirInstructionKind::BooleanOr { left, right } => {
                return boolean_binary(values, *left, *right, |left, right| left || right)
                    .map(RuntimeValue::visible);
            }
            MirInstructionKind::CompareEqual { left, right } => MirValue::Boolean(pop_value_equal(
                &value(values, *left)?.visible,
                &value(values, *right)?.visible,
            )),
            MirInstructionKind::CompareNotEqual { left, right } => {
                MirValue::Boolean(!pop_value_equal(
                    &value(values, *left)?.visible,
                    &value(values, *right)?.visible,
                ))
            }
            MirInstructionKind::FfiHandleOpen { value: managed } => {
                let managed = value(values, *managed)?.clone();
                let reference = managed.reference.ok_or(ExecutionError::TypeMismatch)?;
                let handle = self
                    .runtime
                    .retain_root(reference)
                    .map_err(ExecutionError::Runtime)?;
                if handle.raw() == 0 {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::ImpossibleState)),
                    ));
                }
                self.ffi_handles.insert(handle, managed);
                MirValue::FfiHandle(handle.raw())
            }
            MirInstructionKind::FfiHandleGet { handle } => {
                let MirValue::FfiHandle(raw) = value(values, *handle)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let handle = RootHandle::new(raw);
                let reference = self
                    .runtime
                    .resolve_root(handle)
                    .map_err(ExecutionError::Runtime)?;
                if reference.raw() == 0 {
                    return Err(ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::ImpossibleState)),
                    ));
                }
                let managed = self.ffi_handles.get_mut(&handle).ok_or_else(|| {
                    ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::ImpossibleState)),
                    )
                })?;
                managed.install_relocated_reference(Some(reference))?;
                return Ok(managed.clone());
            }
            MirInstructionKind::FfiCallbackOpenScoped {
                callback,
                owner,
                function,
                site,
                ..
            } => {
                let callback = value(values, *callback)?;
                let reference = callback.reference.ok_or(ExecutionError::TypeMismatch)?;
                let target = self.interpreter_callback_target(callback, *owner, *function)?;
                let site = runtime_callback_site(*owner, *site)?;
                let request = FfiCallbackOpenRequest::new(
                    Some(reference),
                    site,
                    SchedulerId::new(1),
                    FfiCallbackLifetime::CallScoped,
                    FfiCallbackThread::CallingThread,
                );
                let registration = match self.runtime.ffi_callback_open(request) {
                    Ok(registration) => registration,
                    Err(
                        FfiCallbackOpenFailure::Allocation | FfiCallbackOpenFailure::Invariant(_),
                    ) => {
                        return Err(self.runtime_invariant());
                    }
                };
                self.ffi_callbacks.insert(
                    registration.id(),
                    InterpreterCallback {
                        registration,
                        site,
                        target,
                        environment: reference,
                        closed: false,
                    },
                );
                return Ok(RuntimeValue::managed(
                    MirValue::FfiRegisteredCallback {
                        registration: registration.id().raw(),
                        reference,
                    },
                    reference,
                ));
            }
            MirInstructionKind::FfiCallbackOpenOwned {
                callback,
                owner,
                function,
                site,
                thread,
                result,
                success,
                failure,
                ..
            } => {
                let callback = value(values, *callback)?;
                let reference = callback.reference.ok_or(ExecutionError::TypeMismatch)?;
                let target = self.interpreter_callback_target(callback, *owner, *function)?;
                let site = runtime_callback_site(*owner, *site)?;
                let request = FfiCallbackOpenRequest::new(
                    Some(reference),
                    site,
                    SchedulerId::new(1),
                    FfiCallbackLifetime::Registered,
                    *thread,
                );
                let visible = match self.runtime.ffi_callback_open(request) {
                    Ok(registration) => {
                        self.ffi_callbacks.insert(
                            registration.id(),
                            InterpreterCallback {
                                registration,
                                site,
                                target,
                                environment: reference,
                                closed: false,
                            },
                        );
                        MirValue::Result {
                            definition: *result,
                            case: *success,
                            arguments: vec![MirValue::FfiRegisteredCallback {
                                registration: registration.id().raw(),
                                reference,
                            }],
                        }
                    }
                    Err(FfiCallbackOpenFailure::Allocation) => MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::FfiCallbackOpenError],
                    },
                    Err(FfiCallbackOpenFailure::Invariant(_)) => {
                        return Err(self.runtime_invariant());
                    }
                };
                return Ok(RuntimeValue::visible(visible));
            }
            MirInstructionKind::FfiCallbackCloseOwned {
                callback,
                result,
                success,
                failure,
            } => {
                let MirValue::FfiRegisteredCallback { registration, .. } =
                    value(values, *callback)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let id = FfiCallbackRegistrationId::new(registration)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let state = self
                    .ffi_callbacks
                    .get_mut(&id)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let case = if state.closed {
                    *success
                } else {
                    match self.runtime.ffi_callback_close(
                        id,
                        state.registration.context(),
                        state.site,
                    ) {
                        Ok(()) => {
                            state.closed = true;
                            *success
                        }
                        Err(FfiCallbackCloseFailure::InUse) => *failure,
                        Err(FfiCallbackCloseFailure::Invariant(_)) => {
                            return Err(self.runtime_invariant());
                        }
                    }
                };
                let arguments = if case == *success {
                    vec![MirValue::Nil]
                } else {
                    vec![MirValue::FfiCallbackInUseError]
                };
                MirValue::Result {
                    definition: *result,
                    case,
                    arguments,
                }
            }
            MirInstructionKind::FfiBufferOpen {
                length,
                element_size,
                alignment,
                layout,
                result,
                success,
                failure,
                ..
            } => {
                let length = integer_u64(&value(values, *length)?.visible)?;
                let request = FfiBufferOpenRequest::new(length, *element_size, *alignment, *layout)
                    .map_err(|_| self.runtime_invariant())?;
                match self.runtime.ffi_buffer_open(&request) {
                    Ok(reference) if reference.raw() != 0 => MirValue::Result {
                        definition: *result,
                        case: *success,
                        arguments: vec![MirValue::FfiBuffer(reference)],
                    },
                    Ok(_) | Err(FfiBufferOpenFailure::Invariant(_)) => {
                        return Err(self.runtime_invariant());
                    }
                    Err(FfiBufferOpenFailure::Allocation) => MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::FfiAllocationError],
                    },
                }
            }
            MirInstructionKind::FfiBufferLength { buffer, layout } => {
                let reference = value(values, *buffer)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let length = self
                    .runtime
                    .ffi_buffer_length(reference, *layout)
                    .map_err(|_| self.runtime_invariant())?;
                MirValue::Integer(
                    IntegerValue::parse_decimal(&length.to_string(), IntegerKind::UInt64)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::FfiBufferRead {
                buffer,
                index,
                layout,
            } => {
                let reference = value(values, *buffer)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let index = integer_u64(&value(values, *index)?.visible)?;
                let entry = self
                    .mir
                    .ffi_layouts()
                    .get(*layout)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let mut bytes = vec![
                    0;
                    usize::try_from(entry.size())
                        .map_err(|_| ExecutionError::InvalidControlFlow)?
                ];
                self.runtime
                    .ffi_buffer_read(reference, *layout, index, &mut bytes)
                    .map_err(|_| self.runtime_invariant())?;
                unmarshal(&bytes, entry, self.mir.ffi_layouts(), self.arena, self.mir)?
            }
            MirInstructionKind::FfiBufferBorrow {
                buffer,
                expected_length,
                layout,
                region,
            } => {
                let reference = value(values, *buffer)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let expected = integer_u64(&value(values, *expected_length)?.visible)?;
                let borrow = self
                    .runtime
                    .ffi_buffer_borrow(reference, *layout)
                    .map_err(|_| self.runtime_invariant())?;
                if borrow.length() != expected
                    || self
                        .ffi_buffer_borrows
                        .insert(*region, borrow.id())
                        .is_some()
                {
                    return Err(self.runtime_invariant());
                }
                borrow.address().map_or(MirValue::Nil, MirValue::FfiPointer)
            }
            MirInstructionKind::FfiBytesBorrow { bytes, region } => {
                if self.ffi_bytes_borrows.contains_key(region) {
                    return Err(self.runtime_invariant());
                }
                let owner = value(values, *bytes)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let borrow = self
                    .runtime
                    .ffi_bytes_borrow(owner)
                    .map_err(|_| self.runtime_invariant())?;
                let state = FfiBytesBorrowState {
                    owner,
                    borrow: borrow.id(),
                    length: borrow.length(),
                };
                self.ffi_bytes_borrows.insert(*region, state);
                borrow.address().map_or(MirValue::Nil, MirValue::FfiPointer)
            }
            MirInstructionKind::FfiBytesBorrowLength { bytes, region } => {
                let owner = value(values, *bytes)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let state = self
                    .ffi_bytes_borrows
                    .get(region)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                if state.owner != owner {
                    return Err(self.runtime_invariant());
                }
                MirValue::Integer(
                    IntegerValue::parse_decimal(&state.length.to_string(), IntegerKind::UInt64)
                        .map_err(|_| ExecutionError::InvalidControlFlow)?,
                )
            }
            MirInstructionKind::FfiUnsafeLoad { pointer, layout } => {
                let address = ffi_pointer(&value(values, *pointer)?.visible)?;
                let entry = self
                    .mir
                    .ffi_layouts()
                    .get(*layout)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.verify_ffi_alignment(address, entry.alignment())?;
                let mut bytes = vec![
                    0;
                    usize::try_from(entry.size())
                        .map_err(|_| ExecutionError::InvalidControlFlow)?
                ];
                self.runtime
                    .ffi_unsafe_read(address, &mut bytes)
                    .map_err(|_| self.runtime_invariant())?;
                unmarshal(&bytes, entry, self.mir.ffi_layouts(), self.arena, self.mir)?
            }
            MirInstructionKind::FfiUnsafeAdvance {
                pointer,
                elements,
                layout,
                ..
            } => {
                let address = ffi_pointer(&value(values, *pointer)?.visible)?;
                let elements = integer_i64(&value(values, *elements)?.visible)?;
                let entry = self
                    .mir
                    .ffi_layouts()
                    .get(*layout)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let offset = i128::from(elements)
                    .checked_mul(i128::from(entry.size()))
                    .ok_or_else(|| self.integer_overflow())?;
                let raw = i128::from(address.raw())
                    .checked_add(offset)
                    .and_then(|raw| u64::try_from(raw).ok())
                    .and_then(ForeignAddress::new)
                    .ok_or_else(|| self.integer_overflow())?;
                self.runtime
                    .ffi_unsafe_read(raw, &mut [])
                    .map_err(|_| self.runtime_invariant())?;
                MirValue::FfiPointer(raw)
            }
            MirInstructionKind::FfiUnsafeAddress { pointer, .. } => {
                let address = ffi_pointer(&value(values, *pointer)?.visible)?;
                MirValue::Integer(integer_from_u64(
                    address.raw(),
                    instruction.result_type(),
                    self.mir.ffi_layouts(),
                    self.arena,
                )?)
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 2..=24 | 59..=63) => {
                self.evaluate_atomic_standard_call(function.raw(), arguments, values)?
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 25..=34) => {
                self.evaluate_actor_standard_call(function.raw(), arguments, values)?
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 35..=58 | 64..=122 | 128..=182) => {
                self.evaluate_net_standard_call(function.raw(), arguments, values)?
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 123..=127) => {
                self.evaluate_live_time_standard_call(function.raw(), arguments, values)?
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 183..=186) && arguments.is_empty() => {
                match function.raw() {
                    183 | 184 => {
                        let value = if function.raw() == 183 {
                            pop_standard::pop_std_rust_process_id()
                        } else {
                            pop_standard::pop_std_rust_available_parallelism()
                        };
                        MirValue::Integer(
                            IntegerValue::parse_decimal(&value.to_string(), IntegerKind::Int64)
                                .map_err(|_| ExecutionError::TypeMismatch)?,
                        )
                    }
                    185 => MirValue::Boolean(pop_standard::pop_std_rust_stdout_is_terminal()),
                    186 => MirValue::Boolean(pop_standard::pop_std_rust_stderr_is_terminal()),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                }
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if function.raw() == 227 && arguments.is_empty() => std::env::current_exe()
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok())
                .map_or(MirValue::Nil, MirValue::String),
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 230..=231) && arguments.is_empty() => {
                let value = if function.raw() == 230 {
                    pop_standard::pop_std_rust_native_operating_system()
                } else {
                    pop_standard::pop_std_rust_native_architecture()
                };
                MirValue::Integer(
                    IntegerValue::parse_decimal(&value.to_string(), IntegerKind::UInt8)
                        .map_err(|_| ExecutionError::TypeMismatch)?,
                )
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 187..=194) => {
                let unsigned = |index: usize| {
                    arguments
                        .get(index)
                        .copied()
                        .ok_or(ExecutionError::WrongArity)
                        .and_then(|argument| value(values, argument))
                        .and_then(|argument| integer_u64(&argument.visible))
                };
                let result = match function.raw() {
                    187 if arguments.len() == 1 => {
                        pop_standard::pop_std_rust_net_ipv4_is_link_local(unsigned(0)?)
                    }
                    188 if arguments.len() == 1 => {
                        pop_standard::pop_std_rust_net_ipv4_is_multicast(unsigned(0)?)
                    }
                    189 if arguments.len() == 1 => {
                        pop_standard::pop_std_rust_net_ipv4_is_broadcast(unsigned(0)?)
                    }
                    190 if arguments.len() == 1 => {
                        pop_standard::pop_std_rust_net_ipv4_is_documentation(unsigned(0)?)
                    }
                    191 if arguments.len() == 4 => {
                        pop_standard::pop_std_rust_net_ipv6_is_multicast(
                            unsigned(0)?,
                            unsigned(1)?,
                            unsigned(2)?,
                            unsigned(3)?,
                        )
                    }
                    192 if arguments.len() == 4 => {
                        pop_standard::pop_std_rust_net_ipv6_is_unique_local(
                            unsigned(0)?,
                            unsigned(1)?,
                            unsigned(2)?,
                            unsigned(3)?,
                        )
                    }
                    193 if arguments.len() == 4 => {
                        pop_standard::pop_std_rust_net_ipv6_is_unicast_link_local(
                            unsigned(0)?,
                            unsigned(1)?,
                            unsigned(2)?,
                            unsigned(3)?,
                        )
                    }
                    194 if arguments.len() == 4 => {
                        pop_standard::pop_std_rust_net_ipv6_is_documentation(
                            unsigned(0)?,
                            unsigned(1)?,
                            unsigned(2)?,
                            unsigned(3)?,
                        )
                    }
                    187..=194 => return Err(ExecutionError::WrongArity),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                MirValue::Boolean(result)
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 195..=198) => {
                if arguments.len() != 1 {
                    return Err(ExecutionError::WrongArity);
                }
                let MirValue::String(path) = &value(values, arguments[0])?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let result = match function.raw() {
                    195 => std::env::var_os(path).is_some(),
                    196 => Path::new(path).exists(),
                    197 => Path::new(path).is_file(),
                    198 => Path::new(path).is_dir(),
                    _ => return Err(ExecutionError::InvalidControlFlow),
                };
                MirValue::Boolean(result)
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if function.raw() == 199 => {
                if arguments.len() != 3 {
                    return Err(ExecutionError::WrongArity);
                }
                let MirValue::String(path) = &value(values, arguments[0])?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::ByteBuffer(buffer) = value(values, arguments[1])?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let maximum = integer_u64(&value(values, arguments[2])?.visible)?;
                if maximum > 64 * 1024 * 1024 {
                    return Ok(RuntimeValue::visible(MirValue::Integer(
                        IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                    )));
                }
                self.runtime
                    .byte_buffer_clear(buffer)
                    .map_err(ExecutionError::Runtime)?;
                let Ok(mut file) = std::fs::File::open(path) else {
                    return Ok(RuntimeValue::visible(MirValue::Integer(
                        IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                    )));
                };
                let mut bytes = Vec::new();
                if std::io::Read::by_ref(&mut file)
                    .take(maximum)
                    .read_to_end(&mut bytes)
                    .is_err()
                    || self.runtime.byte_buffer_append(buffer, &bytes).is_err()
                {
                    return Ok(RuntimeValue::visible(MirValue::Integer(
                        IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                    )));
                }
                MirValue::Integer(
                    IntegerValue::parse_decimal(&bytes.len().to_string(), IntegerKind::Int64)
                        .map_err(|_| ExecutionError::TypeMismatch)?,
                )
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if function.raw() == 200 => {
                if arguments.len() != 1 {
                    return Err(ExecutionError::WrongArity);
                }
                let MirValue::String(name) = &value(values, arguments[0])?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                match std::env::var_os(name).and_then(|value| value.into_string().ok()) {
                    Some(value) => MirValue::String(value),
                    None => MirValue::Nil,
                }
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 219..=220 | 228) => {
                if arguments.len() != 1 {
                    return Err(ExecutionError::WrongArity);
                }
                let MirValue::String(value) = &value(values, arguments[0])?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let _ = value;
                MirValue::Boolean(true)
            }
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if function.raw() == 229 && arguments.is_empty() => MirValue::Boolean(true),
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } if matches!(function.raw(), 201..=226) => {
                let argument = |index: usize| value(values, arguments[index]);
                match function.raw() {
                    201 if arguments.len() == 1 => {
                        let MirValue::String(root) = &argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let Ok(root) = std::fs::canonicalize(root) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        if !root.is_dir() {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        }
                        let symbol = self.fresh_private_symbol();
                        self.private_values
                            .insert(symbol, PrivateValue::FileAccess(root));
                        MirValue::FfiHandle(u64::from(symbol.raw()))
                    }
                    202 if arguments.len() == 1 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        MirValue::Boolean(matches!(
                            self.private_values.remove(&symbol),
                            Some(PrivateValue::FileAccess(_))
                        ))
                    }
                    203 | 204 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::FileAccess(root)) = self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Boolean(false)));
                        };
                        let path = scoped_file_path(root, relative);
                        MirValue::Boolean(match function.raw() {
                            203 => path.is_some_and(|path| path.exists()),
                            204 => path.is_some_and(|path| path.is_file()),
                            _ => return Err(ExecutionError::InvalidControlFlow),
                        })
                    }
                    205 if arguments.len() == 4 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::ByteBuffer(buffer) = argument(2)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let maximum = integer_u64(&argument(3)?.visible)?;
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::FileAccess(root)) = self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let Some(path) = scoped_file_path(root, relative) else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        if maximum > 64 * 1024 * 1024 {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        self.runtime
                            .byte_buffer_clear(buffer)
                            .map_err(ExecutionError::Runtime)?;
                        let Ok(mut file) = std::fs::File::open(path) else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let mut bytes = Vec::new();
                        if std::io::Read::by_ref(&mut file)
                            .take(maximum)
                            .read_to_end(&mut bytes)
                            .is_err()
                            || self.runtime.byte_buffer_append(buffer, &bytes).is_err()
                        {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        MirValue::Integer(
                            IntegerValue::parse_decimal(
                                &bytes.len().to_string(),
                                IntegerKind::Int64,
                            )
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                        )
                    }
                    206 if arguments.len() == 1 => {
                        let MirValue::String(root) = &argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let Ok(root) = std::fs::canonicalize(root) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        if !root.is_dir() {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        }
                        let symbol = self.fresh_private_symbol();
                        self.private_values
                            .insert(symbol, PrivateValue::DirectoryAccess(root));
                        MirValue::FfiHandle(u64::from(symbol.raw()))
                    }
                    207 if arguments.len() == 1 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        MirValue::Boolean(matches!(
                            self.private_values.remove(&symbol),
                            Some(PrivateValue::DirectoryAccess(_))
                        ))
                    }
                    208 | 209 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::DirectoryAccess(root)) =
                            self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Boolean(false)));
                        };
                        let path = scoped_file_path(root, relative);
                        MirValue::Boolean(match function.raw() {
                            208 => path.is_some_and(|path| path.exists()),
                            209 => path.is_some_and(|path| path.is_dir()),
                            _ => return Err(ExecutionError::InvalidControlFlow),
                        })
                    }
                    224 | 225 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::DirectoryAccess(root)) =
                            self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Boolean(false)));
                        };
                        let relative_path = Path::new(relative);
                        if relative_path.components().any(|component| {
                            matches!(
                                component,
                                Component::RootDir | Component::Prefix(_) | Component::ParentDir
                            )
                        }) {
                            return Ok(RuntimeValue::visible(MirValue::Boolean(false)));
                        }
                        let candidate = root.join(relative_path);
                        let result = if function.raw() == 224 {
                            !candidate.exists()
                                && candidate.starts_with(root)
                                && std::fs::create_dir(candidate).is_ok()
                        } else {
                            scoped_file_path(root, relative)
                                .is_some_and(|path| std::fs::remove_dir(path).is_ok())
                        };
                        MirValue::Boolean(result)
                    }
                    210 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::FileAccess(root)) = self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Some(path) = scoped_file_path(root, relative) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Ok(file) = std::fs::File::open(path) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let handle = self.fresh_private_symbol();
                        self.private_values
                            .insert(handle, PrivateValue::FileHandle(file));
                        MirValue::FfiHandle(u64::from(handle.raw()))
                    }
                    211 if arguments.len() == 3 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::ByteBuffer(buffer) = argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let maximum = integer_u64(&argument(2)?.visible)?;
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        if maximum > 64 * 1024 * 1024
                            || self.runtime.byte_buffer_clear(buffer).is_err()
                        {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        let Some(PrivateValue::FileHandle(file)) =
                            self.private_values.get_mut(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let mut bytes = Vec::new();
                        if file.take(maximum).read_to_end(&mut bytes).is_err()
                            || self.runtime.byte_buffer_append(buffer, &bytes).is_err()
                        {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        MirValue::Integer(
                            IntegerValue::parse_decimal(
                                &bytes.len().to_string(),
                                IntegerKind::Int64,
                            )
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                        )
                    }
                    212 if arguments.len() == 1 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        MirValue::Boolean(matches!(
                            self.private_values.remove(&symbol),
                            Some(PrivateValue::FileHandle(_))
                                | Some(PrivateValue::FileWriteHandle(_))
                        ))
                    }
                    217 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::FileAccess(root)) = self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Some(path) = scoped_file_path(root, relative) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let handle = self.fresh_private_symbol();
                        self.private_values
                            .insert(handle, PrivateValue::FileWriteHandle(file));
                        MirValue::FfiHandle(u64::from(handle.raw()))
                    }
                    226 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::FileAccess(root)) = self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let relative_path = Path::new(relative);
                        if relative_path.components().any(|component| {
                            matches!(
                                component,
                                Component::RootDir | Component::Prefix(_) | Component::ParentDir
                            )
                        }) {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        }
                        let candidate = root.join(relative_path);
                        let Some(parent) = candidate.parent() else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Ok(parent) = std::fs::canonicalize(parent) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        if candidate.exists() || !parent.starts_with(root) {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        }
                        let Ok(file) = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(candidate)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let handle = self.fresh_private_symbol();
                        self.private_values
                            .insert(handle, PrivateValue::FileWriteHandle(file));
                        MirValue::FfiHandle(u64::from(handle.raw()))
                    }
                    218 if arguments.len() == 3 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::ByteBuffer(buffer) = argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let maximum = integer_u64(&argument(2)?.visible)?;
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::FileWriteHandle(file)) =
                            self.private_values.get_mut(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let Ok(length) = self.runtime.byte_buffer_length(buffer) else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let length = length.min(maximum).min(64 * 1024 * 1024);
                        let Ok(length) = usize::try_from(length) else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let mut bytes = vec![0_u8; length];
                        if self
                            .runtime
                            .byte_buffer_read(buffer, 0, &mut bytes)
                            .is_err()
                            || file.write_all(&bytes).is_err()
                        {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        MirValue::Integer(
                            IntegerValue::parse_decimal(
                                &bytes.len().to_string(),
                                IntegerKind::Int64,
                            )
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                        )
                    }
                    221 if arguments.len() == 3 => {
                        let MirValue::FfiHandle(source_raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::FfiHandle(destination_raw) = argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let maximum = integer_u64(&argument(2)?.visible)?;
                        let source_symbol =
                            SymbolId::from_raw(u32::try_from(source_raw).unwrap_or(u32::MAX));
                        let destination_symbol =
                            SymbolId::from_raw(u32::try_from(destination_raw).unwrap_or(u32::MAX));
                        if source_symbol == destination_symbol || maximum > 64 * 1024 * 1024 {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        let Some(PrivateValue::FileHandle(mut input)) =
                            self.private_values.remove(&source_symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let Some(PrivateValue::FileWriteHandle(mut output)) =
                            self.private_values.remove(&destination_symbol)
                        else {
                            self.private_values
                                .insert(source_symbol, PrivateValue::FileHandle(input));
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        let mut copied = 0_u64;
                        let mut buffer = vec![0_u8; 64 * 1024];
                        let mut failed = false;
                        while copied < maximum {
                            let chunk =
                                usize::try_from((maximum - copied).min(buffer.len() as u64))
                                    .map_err(|_| ExecutionError::TypeMismatch)?;
                            let Ok(read) = input.read(&mut buffer[..chunk]) else {
                                failed = true;
                                break;
                            };
                            if read == 0 {
                                break;
                            }
                            if output.write_all(&buffer[..read]).is_err() {
                                failed = true;
                                break;
                            }
                            copied = copied
                                .checked_add(
                                    u64::try_from(read)
                                        .map_err(|_| ExecutionError::TypeMismatch)?,
                                )
                                .ok_or(ExecutionError::TypeMismatch)?;
                        }
                        self.private_values
                            .insert(source_symbol, PrivateValue::FileHandle(input));
                        self.private_values
                            .insert(destination_symbol, PrivateValue::FileWriteHandle(output));
                        if failed {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("-1", IntegerKind::Int64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        }
                        MirValue::Integer(
                            IntegerValue::parse_decimal(&copied.to_string(), IntegerKind::Int64)
                                .map_err(|_| ExecutionError::TypeMismatch)?,
                        )
                    }
                    213 if arguments.len() == 3 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let MirValue::String(relative) = &argument(1)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let maximum = integer_u64(&argument(2)?.visible)?;
                        if maximum > 65_536 {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        }
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::DirectoryAccess(root)) =
                            self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Some(path) = scoped_file_path(root, relative) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let Ok(entries) = std::fs::read_dir(path) else {
                            return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                        };
                        let limit =
                            usize::try_from(maximum).map_err(|_| ExecutionError::TypeMismatch)?;
                        let mut names = Vec::new();
                        for entry in entries {
                            let Ok(entry) = entry else {
                                return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                            };
                            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                                return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                            };
                            names.push(name);
                            if names.len() > limit {
                                return Ok(RuntimeValue::visible(MirValue::FfiHandle(0)));
                            }
                        }
                        names.sort();
                        let snapshot = self.fresh_private_symbol();
                        self.private_values
                            .insert(snapshot, PrivateValue::DirectorySnapshot(names));
                        MirValue::FfiHandle(u64::from(snapshot.raw()))
                    }
                    214 if arguments.len() == 1 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        MirValue::Boolean(matches!(
                            self.private_values.remove(&symbol),
                            Some(PrivateValue::DirectorySnapshot(_))
                        ))
                    }
                    215 if arguments.len() == 1 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::DirectorySnapshot(names)) =
                            self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Integer(
                                IntegerValue::parse_decimal("0", IntegerKind::UInt64)
                                    .map_err(|_| ExecutionError::TypeMismatch)?,
                            )));
                        };
                        MirValue::Integer(
                            IntegerValue::parse_decimal(
                                &names.len().to_string(),
                                IntegerKind::UInt64,
                            )
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                        )
                    }
                    216 if arguments.len() == 2 => {
                        let MirValue::FfiHandle(raw) = argument(0)?.visible else {
                            return Err(ExecutionError::TypeMismatch);
                        };
                        let index = integer_u64(&argument(1)?.visible)?;
                        let symbol = SymbolId::from_raw(u32::try_from(raw).unwrap_or(u32::MAX));
                        let Some(PrivateValue::DirectorySnapshot(names)) =
                            self.private_values.get(&symbol)
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Nil));
                        };
                        let Some(name) = usize::try_from(index)
                            .ok()
                            .and_then(|index| names.get(index))
                        else {
                            return Ok(RuntimeValue::visible(MirValue::Nil));
                        };
                        MirValue::String(name.clone())
                    }
                    _ => return Err(ExecutionError::WrongArity),
                }
            }
            MirInstructionKind::FfiUnsafePointerFromAddress { address, .. } => {
                let raw = integer_u64(&value(values, *address)?.visible)?;
                ForeignAddress::new(raw).map_or(MirValue::Nil, MirValue::FfiPointer)
            }
            MirInstructionKind::IntegerConstant(_)
            | MirInstructionKind::FloatConstant(_)
            | MirInstructionKind::CheckedIntegerAdd { .. }
            | MirInstructionKind::CheckedIntegerSubtract { .. }
            | MirInstructionKind::CheckedIntegerMultiply { .. }
            | MirInstructionKind::CheckedIntegerDivide { .. }
            | MirInstructionKind::CheckedIntegerRemainder { .. }
            | MirInstructionKind::FloatAdd { .. }
            | MirInstructionKind::FloatSubtract { .. }
            | MirInstructionKind::FloatMultiply { .. }
            | MirInstructionKind::FloatDivide { .. }
            | MirInstructionKind::IntegerNegate { .. }
            | MirInstructionKind::FloatNegate { .. }
            | MirInstructionKind::ConvertInteger { .. }
            | MirInstructionKind::ConvertIntegerToFloat { .. }
            | MirInstructionKind::ConvertFloatToInteger { .. }
            | MirInstructionKind::ConvertFloat { .. }
            | MirInstructionKind::CompareIntegerLess { .. }
            | MirInstructionKind::CompareIntegerLessOrEqual { .. }
            | MirInstructionKind::CompareIntegerGreater { .. }
            | MirInstructionKind::CompareIntegerGreaterOrEqual { .. }
            | MirInstructionKind::CompareFloatLess { .. }
            | MirInstructionKind::CompareFloatLessOrEqual { .. }
            | MirInstructionKind::CompareFloatGreater { .. }
            | MirInstructionKind::CompareFloatGreaterOrEqual { .. }
            | MirInstructionKind::CallStandard { .. }
            | MirInstructionKind::CallDirect { .. }
            | MirInstructionKind::CallForeign { .. }
            | MirInstructionKind::CallReferenced { .. }
            | MirInstructionKind::CallDirectMethod { .. }
            | MirInstructionKind::CallInterface { .. }
            | MirInstructionKind::CallBuiltinInterface { .. }
            | MirInstructionKind::CallIndirect { .. }
            | MirInstructionKind::CallScopedBorrow { .. }
            | MirInstructionKind::CallCallbackPair { .. }
            | MirInstructionKind::RecordMake { .. }
            | MirInstructionKind::ClassMake { .. }
            | MirInstructionKind::RecordUpdate { .. }
            | MirInstructionKind::FieldGet { .. }
            | MirInstructionKind::FieldSet { .. }
            | MirInstructionKind::UnionMake { .. }
            | MirInstructionKind::ResultMake { .. }
            | MirInstructionKind::IterationMake { .. }
            | MirInstructionKind::ErrorMake { .. }
            | MirInstructionKind::InterfaceUpcast { .. }
            | MirInstructionKind::CheckedDowncast { .. }
            | MirInstructionKind::ViewEnd { .. }
            | MirInstructionKind::CaptureCellAllocate { .. }
            | MirInstructionKind::CaptureCellLoad { .. }
            | MirInstructionKind::CaptureCellStore { .. }
            | MirInstructionKind::ClosureEnvironmentAllocate { .. }
            | MirInstructionKind::CaptureLoad { .. }
            | MirInstructionKind::CaptureCellReference { .. }
            | MirInstructionKind::CaptureStore { .. }
            | MirInstructionKind::GcSafePoint { .. }
            | MirInstructionKind::RetainRoot { .. }
            | MirInstructionKind::ReleaseRoot { .. }
            | MirInstructionKind::FfiHandleClose { .. }
            | MirInstructionKind::FfiCallbackCloseScoped { .. }
            | MirInstructionKind::FfiBufferWrite { .. }
            | MirInstructionKind::FfiBufferEndBorrow { .. }
            | MirInstructionKind::FfiBytesEndBorrow { .. }
            | MirInstructionKind::FfiBufferClose { .. }
            | MirInstructionKind::FfiUnsafeStore { .. }
            | MirInstructionKind::FfiUnsafeCopy { .. }
            | MirInstructionKind::Pin { .. }
            | MirInstructionKind::Unpin { .. }
            | MirInstructionKind::WriteBarrier { .. } => {
                return Err(ExecutionError::InvalidControlFlow);
            }
        };
        Ok(RuntimeValue::visible(result))
    }

    fn runtime_invariant(&mut self) -> ExecutionError {
        ExecutionError::Runtime(
            self.runtime
                .raise_trap(Trap::new(TrapKind::ImpossibleState)),
        )
    }

    fn bounds_violation(&mut self) -> ExecutionError {
        ExecutionError::Runtime(
            self.runtime
                .raise_trap(Trap::new(TrapKind::BoundsViolation)),
        )
    }

    fn integer_overflow(&mut self) -> ExecutionError {
        ExecutionError::Runtime(
            self.runtime
                .raise_trap(Trap::new(TrapKind::IntegerOverflow)),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn evaluate_atomic_standard_call(
        &mut self,
        function: u32,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<MirValue, ExecutionError> {
        if let Some(order) = match function {
            2 | 5 | 8 => Some(0),
            3 | 6 | 9 => Some(1),
            4 | 7 | 10 => Some(2),
            11 => Some(3),
            12 => Some(4),
            _ => None,
        } {
            if !arguments.is_empty() {
                return Err(ExecutionError::WrongArity);
            }
            return IntegerValue::parse_decimal(&order.to_string(), IntegerKind::Int64)
                .map(MirValue::Integer)
                .map_err(|_| ExecutionError::InvalidControlFlow);
        }
        let argument = |index: usize| {
            arguments
                .get(index)
                .copied()
                .ok_or(ExecutionError::WrongArity)
                .and_then(|argument| value(values, argument))
        };
        let load_order = |index: usize| -> Result<AtomicLoadOrder, ExecutionError> {
            match integer_i64(&argument(index)?.visible)? {
                0 => Ok(AtomicLoadOrder::Relaxed),
                1 => Ok(AtomicLoadOrder::Acquire),
                2 => Ok(AtomicLoadOrder::SequentiallyConsistent),
                _ => Err(ExecutionError::InvalidControlFlow),
            }
        };
        let store_order = |index: usize| -> Result<AtomicStoreOrder, ExecutionError> {
            match integer_i64(&argument(index)?.visible)? {
                0 => Ok(AtomicStoreOrder::Relaxed),
                1 => Ok(AtomicStoreOrder::Release),
                2 => Ok(AtomicStoreOrder::SequentiallyConsistent),
                _ => Err(ExecutionError::InvalidControlFlow),
            }
        };
        let read_modify_write_order =
            |index: usize| -> Result<AtomicReadModifyWriteOrder, ExecutionError> {
                match integer_i64(&argument(index)?.visible)? {
                    0 => Ok(AtomicReadModifyWriteOrder::Relaxed),
                    1 => Ok(AtomicReadModifyWriteOrder::Acquire),
                    2 => Ok(AtomicReadModifyWriteOrder::Release),
                    3 => Ok(AtomicReadModifyWriteOrder::AcquireRelease),
                    4 => Ok(AtomicReadModifyWriteOrder::SequentiallyConsistent),
                    _ => Err(ExecutionError::InvalidControlFlow),
                }
            };
        match function {
            13 if arguments.len() == 1 => {
                let initial = integer_i64(&argument(0)?.visible)?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::AtomicInt(AtomicInt::new(initial)));
                Ok(MirValue::AtomicInt(symbol))
            }
            14 if arguments.len() == 1 => {
                let MirValue::Boolean(initial) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let symbol = self.fresh_private_symbol();
                self.private_values.insert(
                    symbol,
                    PrivateValue::AtomicBoolean(AtomicBoolean::new(initial)),
                );
                Ok(MirValue::AtomicBoolean(symbol))
            }
            15 if arguments.len() == 2 => {
                let MirValue::AtomicInt(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::AtomicInt(state)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let loaded = state.load(load_order(1)?);
                IntegerValue::parse_decimal(&loaded.to_string(), IntegerKind::Int64)
                    .map(MirValue::Integer)
                    .map_err(|_| ExecutionError::InvalidControlFlow)
            }
            16 if arguments.len() == 2 => {
                let MirValue::AtomicBoolean(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::AtomicBoolean(state)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                Ok(MirValue::Boolean(state.load(load_order(1)?)))
            }
            17 if arguments.len() == 3 => {
                let MirValue::AtomicInt(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let stored = integer_i64(&argument(1)?.visible)?;
                let Some(PrivateValue::AtomicInt(state)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                state.store(stored, store_order(2)?);
                Ok(MirValue::Boolean(true))
            }
            18 if arguments.len() == 3 => {
                let MirValue::AtomicBoolean(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Boolean(stored) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::AtomicBoolean(state)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                state.store(stored, store_order(2)?);
                Ok(MirValue::Boolean(true))
            }
            19 if arguments.len() == 3 => {
                let MirValue::AtomicInt(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let stored = integer_i64(&argument(1)?.visible)?;
                let Some(PrivateValue::AtomicInt(state)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let previous = state.swap(stored, read_modify_write_order(2)?);
                IntegerValue::parse_decimal(&previous.to_string(), IntegerKind::Int64)
                    .map(MirValue::Integer)
                    .map_err(|_| ExecutionError::InvalidControlFlow)
            }
            20 if arguments.len() == 3 => {
                let MirValue::AtomicBoolean(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Boolean(stored) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::AtomicBoolean(state)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                Ok(MirValue::Boolean(
                    state.swap(stored, read_modify_write_order(2)?),
                ))
            }
            21 if arguments.len() == 1 => {
                let MirValue::AtomicInt(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::AtomicInt(_))
                )))
            }
            22 if arguments.len() == 1 => {
                let MirValue::AtomicBoolean(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::AtomicBoolean(_))
                )))
            }
            23 if arguments.len() == 5 => {
                let MirValue::AtomicInt(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let current = integer_i64(&argument(1)?.visible)?;
                let new = integer_i64(&argument(2)?.visible)?;
                let order =
                    AtomicCompareExchangeOrder::new(read_modify_write_order(3)?, load_order(4)?)
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                let Some(PrivateValue::AtomicInt(state)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let observed = state.compare_exchange(current, new, order).previous();
                IntegerValue::parse_decimal(&observed.to_string(), IntegerKind::Int64)
                    .map(MirValue::Integer)
                    .map_err(|_| ExecutionError::InvalidControlFlow)
            }
            24 if arguments.len() == 5 => {
                let MirValue::AtomicBoolean(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Boolean(current) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Boolean(new) = argument(2)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let order =
                    AtomicCompareExchangeOrder::new(read_modify_write_order(3)?, load_order(4)?)
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                let Some(PrivateValue::AtomicBoolean(state)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                Ok(MirValue::Boolean(
                    state.compare_exchange(current, new, order).previous(),
                ))
            }
            59..=63 if arguments.len() == 3 => {
                let MirValue::AtomicInt(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let operand = integer_i64(&argument(1)?.visible)?;
                let order = read_modify_write_order(2)?;
                let Some(PrivateValue::AtomicInt(state)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let previous = match function {
                    59 => state.fetch_add(operand, order),
                    60 => state.fetch_subtract(operand, order),
                    61 => state.fetch_and(operand, order),
                    62 => state.fetch_or(operand, order),
                    63 => state.fetch_xor(operand, order),
                    _ => unreachable!(),
                };
                IntegerValue::parse_decimal(&previous.to_string(), IntegerKind::Int64)
                    .map(MirValue::Integer)
                    .map_err(|_| ExecutionError::InvalidControlFlow)
            }
            _ => Err(ExecutionError::WrongArity),
        }
    }

    fn evaluate_actor_standard_call(
        &mut self,
        function: u32,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<MirValue, ExecutionError> {
        let argument = |index: usize| {
            arguments
                .get(index)
                .copied()
                .ok_or(ExecutionError::WrongArity)
                .and_then(|argument| value(values, argument))
        };
        let unsigned = |index: usize| -> Result<u64, ExecutionError> {
            let MirValue::Integer(value) = argument(index)?.visible else {
                return Err(ExecutionError::TypeMismatch);
            };
            value.unsigned().ok_or(ExecutionError::TypeMismatch)
        };
        match function {
            25 if arguments.len() == 3 => {
                let actor = unsigned(0)?;
                let incarnation = unsigned(1)?;
                let capacity = unsigned(2)?;
                let mut lifecycle = ActorLifecycle::starting(
                    ActorId::new(actor),
                    ActorIncarnation::new(incarnation),
                    capacity,
                );
                if lifecycle.activate().is_err() {
                    return Ok(MirValue::Nil);
                }
                let symbol = self.fresh_private_symbol();
                self.private_values.insert(
                    symbol,
                    PrivateValue::Actor(Rc::new(RefCell::new(lifecycle))),
                );
                Ok(MirValue::ActorInbox(symbol))
            }
            26 if arguments.len() == 1 => {
                let MirValue::ActorInbox(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if !matches!(
                    self.private_values.get(&symbol),
                    Some(PrivateValue::Actor(_))
                ) {
                    return Err(self.runtime_invariant());
                }
                Ok(MirValue::ActorRef(symbol))
            }
            27 if arguments.len() == 2 => {
                let MirValue::ActorRef(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let actor = match self.private_values.get(&symbol) {
                    Some(PrivateValue::Actor(actor)) => Rc::clone(actor),
                    _ => return Err(self.runtime_invariant()),
                };
                let sent = argument(1)?.clone();
                if sent.reference.is_some() {
                    return Err(ExecutionError::TypeMismatch);
                }
                let queued = InterpreterChannelValue {
                    value: sent,
                    root: None,
                };
                let reference = actor.borrow().reference();
                let outcome = match actor.borrow_mut().try_admit(reference, queued) {
                    Ok(()) => pop_types::ActorSendOutcomeKind::Accepted,
                    Err(ActorSendError::Full(_)) => pop_types::ActorSendOutcomeKind::Full,
                    Err(ActorSendError::Closed(_)) => pop_types::ActorSendOutcomeKind::Closed,
                    Err(ActorSendError::Stale(_)) => pop_types::ActorSendOutcomeKind::Stale,
                };
                Ok(MirValue::ActorSendOutcome(outcome))
            }
            28 if arguments.len() == 1 => {
                let MirValue::ActorInbox(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let actor = match self.private_values.get(&symbol) {
                    Some(PrivateValue::Actor(actor)) => Rc::clone(actor),
                    _ => return Err(self.runtime_invariant()),
                };
                match actor.borrow_mut().try_receive() {
                    ActorReceive::Message(received) => Ok(received.value.observed_visible()),
                    ActorReceive::Empty | ActorReceive::Closed => Ok(MirValue::Nil),
                }
            }
            29 if arguments.len() == 1 => {
                let MirValue::ActorInbox(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let actor = match self.private_values.get(&symbol) {
                    Some(PrivateValue::Actor(actor)) => Rc::clone(actor),
                    _ => return Err(self.runtime_invariant()),
                };
                let queued = actor
                    .borrow_mut()
                    .begin_exit(ActorExit::Completed)
                    .map_err(|_| self.runtime_invariant())?;
                for value in queued {
                    if let Some(root) = value.root {
                        self.runtime
                            .release_root(root)
                            .map_err(ExecutionError::Runtime)?;
                    }
                }
                actor
                    .borrow_mut()
                    .complete_exit()
                    .map_err(|_| self.runtime_invariant())?;
                Ok(MirValue::Boolean(true))
            }
            30 if arguments.len() == 1 => {
                let MirValue::ActorInbox(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::Actor(_))
                )))
            }
            31..=34 if arguments.len() == 1 => {
                let MirValue::ActorSendOutcome(outcome) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let expected = match function {
                    31 => pop_types::ActorSendOutcomeKind::Accepted,
                    32 => pop_types::ActorSendOutcomeKind::Full,
                    33 => pop_types::ActorSendOutcomeKind::Closed,
                    _ => pop_types::ActorSendOutcomeKind::Stale,
                };
                Ok(MirValue::Boolean(outcome == expected))
            }
            _ => Err(ExecutionError::WrongArity),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn evaluate_net_standard_call(
        &mut self,
        function: u32,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<MirValue, ExecutionError> {
        let argument = |index: usize| {
            arguments
                .get(index)
                .copied()
                .ok_or(ExecutionError::WrongArity)
                .and_then(|argument| value(values, argument))
        };
        let unsigned = |index: usize| integer_u64(&argument(index)?.visible);
        let integer = |value: u64, kind: IntegerKind| {
            IntegerValue::parse_decimal(&value.to_string(), kind)
                .map(MirValue::Integer)
                .map_err(|_| ExecutionError::InvalidControlFlow)
        };
        let closed_error = |error: &std::io::Error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            )
        };
        match function {
            35 if arguments.len() == 1 => {
                let port = u16::try_from(unsigned(0)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                    .map_err(|_| self.runtime_invariant())?;
                listener
                    .set_nonblocking(true)
                    .map_err(|_| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::TcpListener(listener));
                Ok(MirValue::NetTcpListener(symbol))
            }
            36 if arguments.len() == 1 => {
                let MirValue::NetTcpListener(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpListener(listener)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::from(
                        listener
                            .local_addr()
                            .map_err(|_| self.runtime_invariant())?
                            .port(),
                    ),
                    IntegerKind::UInt16,
                )
            }
            37 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::from(
                        stream
                            .local_addr()
                            .map_err(|_| self.runtime_invariant())?
                            .port(),
                    ),
                    IntegerKind::UInt16,
                )
            }
            38 if arguments.len() == 1 => {
                let port = u16::try_from(unsigned(0)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                    .map_err(|_| self.runtime_invariant())?;
                stream
                    .set_nonblocking(true)
                    .map_err(|_| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::TcpStream(stream));
                Ok(MirValue::NetTcpStream(symbol))
            }
            39 if arguments.len() == 1 => {
                let MirValue::NetTcpListener(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let accepted = match self.private_values.get(&symbol) {
                    Some(PrivateValue::TcpListener(listener)) => listener.accept(),
                    _ => return Err(self.runtime_invariant()),
                };
                match accepted {
                    Ok((stream, _)) => {
                        stream
                            .set_nonblocking(true)
                            .map_err(|_| self.runtime_invariant())?;
                        let stream_symbol = self.fresh_private_symbol();
                        self.private_values
                            .insert(stream_symbol, PrivateValue::TcpStream(stream));
                        Ok(MirValue::NetTcpStream(stream_symbol))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        Ok(MirValue::Nil)
                    }
                    Err(_) => Err(self.runtime_invariant()),
                }
            }
            40 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let byte = u8::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get_mut(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let outcome = match stream.write(&[byte]) {
                    Ok(0) => pop_types::SocketIoOutcomeKind::Closed,
                    Ok(_) => pop_types::SocketIoOutcomeKind::Progress,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        pop_types::SocketIoOutcomeKind::WouldBlock
                    }
                    Err(error) if closed_error(&error) => pop_types::SocketIoOutcomeKind::Closed,
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetSocketIoOutcome(outcome))
            }
            41 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get_mut(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let mut byte = [0_u8; 1];
                let (kind, value) = match stream.read(&mut byte) {
                    Ok(0) => (pop_types::TcpReceiveKind::Closed, None),
                    Ok(_) => (pop_types::TcpReceiveKind::Progress, Some(byte[0])),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::TcpReceiveKind::WouldBlock, None)
                    }
                    Err(error) if closed_error(&error) => (pop_types::TcpReceiveKind::Closed, None),
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTcpReceive { kind, value })
            }
            42 if arguments.len() == 1 => {
                let MirValue::NetTcpListener(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::TcpListener(_))
                )))
            }
            43 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let removed = self.private_values.remove(&symbol);
                if let Some(PrivateValue::TcpStream(stream)) = &removed {
                    let _ = stream.shutdown(Shutdown::Both);
                }
                Ok(MirValue::Boolean(matches!(
                    removed,
                    Some(PrivateValue::TcpStream(_))
                )))
            }
            44..=46 if arguments.len() == 1 => {
                let MirValue::NetSocketIoOutcome(outcome) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let expected = match function {
                    44 => pop_types::SocketIoOutcomeKind::Progress,
                    45 => pop_types::SocketIoOutcomeKind::WouldBlock,
                    _ => pop_types::SocketIoOutcomeKind::Closed,
                };
                Ok(MirValue::Boolean(outcome == expected))
            }
            47 | 49 | 50 if arguments.len() == 1 => {
                let MirValue::NetTcpReceive { kind, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let expected = match function {
                    47 => pop_types::TcpReceiveKind::Progress,
                    49 => pop_types::TcpReceiveKind::WouldBlock,
                    _ => pop_types::TcpReceiveKind::Closed,
                };
                Ok(MirValue::Boolean(kind == expected))
            }
            48 if arguments.len() == 1 => {
                let MirValue::NetTcpReceive { value, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                value.map_or(Ok(MirValue::Nil), |byte| {
                    integer(u64::from(byte), IntegerKind::UInt8)
                })
            }
            51 if arguments.len() == 1 => {
                let port = u16::try_from(unsigned(0)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, port))
                    .map_err(|_| self.runtime_invariant())?;
                socket
                    .set_nonblocking(true)
                    .map_err(|_| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::UdpSocket(socket));
                Ok(MirValue::NetUdpSocket(symbol))
            }
            52 if arguments.len() == 1 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::from(
                        socket
                            .local_addr()
                            .map_err(|_| self.runtime_invariant())?
                            .port(),
                    ),
                    IntegerKind::UInt16,
                )
            }
            53 if arguments.len() == 4 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let address =
                    u32::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let port = u16::try_from(unsigned(2)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let byte = u8::try_from(unsigned(3)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let destination = SocketAddrV4::new(Ipv4Addr::from(address), port);
                let outcome = match socket.send_to(&[byte], destination) {
                    Ok(_) => pop_types::SocketIoOutcomeKind::Progress,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        pop_types::SocketIoOutcomeKind::WouldBlock
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetSocketIoOutcome(outcome))
            }
            54 if arguments.len() == 1 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let mut byte = [0_u8; 1];
                match socket.recv_from(&mut byte) {
                    Ok((_, std::net::SocketAddr::V4(peer))) => Ok(MirValue::NetUdpDatagram {
                        address: u32::from(*peer.ip()),
                        port: peer.port(),
                        value: byte[0],
                    }),
                    Ok((_, std::net::SocketAddr::V6(_))) => Err(self.runtime_invariant()),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        Ok(MirValue::Nil)
                    }
                    Err(_) => Err(self.runtime_invariant()),
                }
            }
            55 if arguments.len() == 1 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::UdpSocket(_))
                )))
            }
            56..=58 if arguments.len() == 1 => {
                let MirValue::NetUdpDatagram {
                    address,
                    port,
                    value,
                } = argument(0)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                match function {
                    56 => integer(u64::from(value), IntegerKind::UInt8),
                    57 => integer(u64::from(address), IntegerKind::UInt32),
                    _ => integer(u64::from(port), IntegerKind::UInt16),
                }
            }
            64 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Bytes(reference) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .immutable_bytes_length(reference)
                    .map_err(|_| self.runtime_invariant())?;
                let mut bytes =
                    vec![0; usize::try_from(length).map_err(|_| ExecutionError::TypeMismatch)?];
                self.runtime
                    .immutable_bytes_read(reference, 0, &mut bytes)
                    .map_err(|_| self.runtime_invariant())?;
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get_mut(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let (kind, count) = match stream.write(&bytes) {
                    Ok(0) if !bytes.is_empty() => (pop_types::SocketIoOutcomeKind::Closed, 0),
                    Ok(count) => (
                        pop_types::SocketIoOutcomeKind::Progress,
                        u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(error) if closed_error(&error) => {
                        (pop_types::SocketIoOutcomeKind::Closed, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            65 if arguments.len() == 3 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::ByteBuffer(buffer) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let capacity = usize::try_from(unsigned(2)?)
                    .ok()
                    .filter(|capacity| *capacity > 0)
                    .ok_or(ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get_mut(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let mut bytes = vec![0; capacity];
                let (kind, count) = match stream.read(&mut bytes) {
                    Ok(0) => (pop_types::SocketIoOutcomeKind::Closed, 0),
                    Ok(count) => {
                        bytes.truncate(count);
                        self.runtime
                            .byte_buffer_append(buffer, &bytes)
                            .map_err(|_| self.runtime_invariant())?;
                        (
                            pop_types::SocketIoOutcomeKind::Progress,
                            u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                        )
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(error) if closed_error(&error) => {
                        (pop_types::SocketIoOutcomeKind::Closed, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            66..=68 if arguments.len() == 1 => {
                let MirValue::NetTransfer { kind, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let expected = match function {
                    66 => pop_types::SocketIoOutcomeKind::Progress,
                    67 => pop_types::SocketIoOutcomeKind::WouldBlock,
                    _ => pop_types::SocketIoOutcomeKind::Closed,
                };
                Ok(MirValue::Boolean(kind == expected))
            }
            69 if arguments.len() == 1 => {
                let MirValue::NetTransfer { count, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                integer(count, IntegerKind::UInt64)
            }
            70 if arguments.len() == 4 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let address =
                    u32::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let port = u16::try_from(unsigned(2)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let MirValue::Bytes(reference) = argument(3)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .immutable_bytes_length(reference)
                    .map_err(|_| self.runtime_invariant())?;
                let mut bytes =
                    vec![0; usize::try_from(length).map_err(|_| ExecutionError::TypeMismatch)?];
                self.runtime
                    .immutable_bytes_read(reference, 0, &mut bytes)
                    .map_err(|_| self.runtime_invariant())?;
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let destination = SocketAddrV4::new(Ipv4Addr::from(address), port);
                let (kind, count) = match socket.send_to(&bytes, destination) {
                    Ok(count) => (
                        pop_types::SocketIoOutcomeKind::Progress,
                        u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            71 if arguments.len() == 3 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::ByteBuffer(buffer) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let capacity = usize::try_from(unsigned(2)?)
                    .ok()
                    .filter(|value| *value > 0 && u16::try_from(*value).is_ok())
                    .ok_or(ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let mut bytes = vec![0; capacity];
                match socket.recv_from(&mut bytes) {
                    Ok((count, std::net::SocketAddr::V4(peer))) => {
                        bytes.truncate(count);
                        self.runtime
                            .byte_buffer_append(buffer, &bytes)
                            .map_err(|_| self.runtime_invariant())?;
                        Ok(MirValue::NetUdpTransfer {
                            address: u32::from(*peer.ip()),
                            port: peer.port(),
                            count: u16::try_from(count).map_err(|_| self.runtime_invariant())?,
                        })
                    }
                    Ok((_, std::net::SocketAddr::V6(_))) => Err(self.runtime_invariant()),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        Ok(MirValue::Nil)
                    }
                    Err(_) => Err(self.runtime_invariant()),
                }
            }
            72..=74 if arguments.len() == 1 => {
                let MirValue::NetUdpTransfer {
                    address,
                    port,
                    count,
                } = argument(0)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                match function {
                    72 => integer(u64::from(count), IntegerKind::UInt64),
                    73 => integer(u64::from(address), IntegerKind::UInt32),
                    _ => integer(u64::from(port), IntegerKind::UInt16),
                }
            }
            75..=77 if arguments.len() == 2 => {
                let MirValue::Record { ref fields, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let bits = &fields.first().ok_or(ExecutionError::TypeMismatch)?.1;
                let address =
                    u32::try_from(integer_u64(bits)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let port = u16::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                if function == 75 {
                    let listener = TcpListener::bind((Ipv4Addr::from(address), port))
                        .map_err(|_| self.runtime_invariant())?;
                    listener
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::TcpListener(listener));
                    Ok(MirValue::NetTcpListener(symbol))
                } else if function == 76 {
                    let stream = TcpStream::connect((Ipv4Addr::from(address), port))
                        .map_err(|_| self.runtime_invariant())?;
                    stream
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::TcpStream(stream));
                    Ok(MirValue::NetTcpStream(symbol))
                } else {
                    let socket = UdpSocket::bind((Ipv4Addr::from(address), port))
                        .map_err(|_| self.runtime_invariant())?;
                    socket
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::UdpSocket(socket));
                    Ok(MirValue::NetUdpSocket(symbol))
                }
            }
            78..=80 if arguments.len() == 2 => {
                let MirValue::Record { ref fields, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if fields.len() != 4 {
                    return Err(ExecutionError::TypeMismatch);
                }
                let word = |index: usize| {
                    u32::try_from(integer_u64(&fields[index].1)?)
                        .map_err(|_| ExecutionError::TypeMismatch)
                };
                let address = Ipv6Addr::new(
                    u16::try_from(word(0)? >> 16).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(0)? & 0xffff).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(1)? >> 16).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(1)? & 0xffff).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(2)? >> 16).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(2)? & 0xffff).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(3)? >> 16).map_err(|_| ExecutionError::TypeMismatch)?,
                    u16::try_from(word(3)? & 0xffff).map_err(|_| ExecutionError::TypeMismatch)?,
                );
                let port = u16::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                if function == 78 {
                    let listener =
                        TcpListener::bind((address, port)).map_err(|_| self.runtime_invariant())?;
                    listener
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::TcpListener(listener));
                    Ok(MirValue::NetTcpListener(symbol))
                } else if function == 79 {
                    let stream = TcpStream::connect((address, port))
                        .map_err(|_| self.runtime_invariant())?;
                    stream
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::TcpStream(stream));
                    Ok(MirValue::NetTcpStream(symbol))
                } else {
                    let socket =
                        UdpSocket::bind((address, port)).map_err(|_| self.runtime_invariant())?;
                    socket
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::UdpSocket(socket));
                    Ok(MirValue::NetUdpSocket(symbol))
                }
            }
            81..=83 if arguments.len() == 2 => {
                let MirValue::Record { ref fields, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let (
                    Some((
                        _,
                        MirValue::Record {
                            fields: address, ..
                        },
                    )),
                    Some((
                        _,
                        MirValue::Record {
                            fields: interface, ..
                        },
                    )),
                ) = (fields.first(), fields.get(1))
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if address.len() != 4 || interface.len() != 1 {
                    return Err(ExecutionError::TypeMismatch);
                }
                let word = |index: usize| {
                    u32::try_from(integer_u64(&address[index].1)?)
                        .map_err(|_| ExecutionError::TypeMismatch)
                };
                let words = [word(0)?, word(1)?, word(2)?, word(3)?];
                let mut octets = [0_u8; 16];
                for (index, word) in words.into_iter().enumerate() {
                    octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                let endpoint = std::net::SocketAddrV6::new(
                    Ipv6Addr::from(octets),
                    u16::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?,
                    0,
                    u32::try_from(integer_u64(&interface[0].1)?)
                        .map_err(|_| ExecutionError::TypeMismatch)?,
                );
                if function == 81 {
                    let listener =
                        TcpListener::bind(endpoint).map_err(|_| self.runtime_invariant())?;
                    listener
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::TcpListener(listener));
                    Ok(MirValue::NetTcpListener(symbol))
                } else if function == 82 {
                    let stream =
                        TcpStream::connect(endpoint).map_err(|_| self.runtime_invariant())?;
                    stream
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::TcpStream(stream));
                    Ok(MirValue::NetTcpStream(symbol))
                } else {
                    let socket = UdpSocket::bind(endpoint).map_err(|_| self.runtime_invariant())?;
                    socket
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::UdpSocket(socket));
                    Ok(MirValue::NetUdpSocket(symbol))
                }
            }
            84 if arguments.is_empty() => {
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::DnsResolver);
                Ok(MirValue::NetDnsResolver(symbol))
            }
            85 if arguments.len() == 3 => {
                let MirValue::NetDnsResolver(resolver) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if !matches!(
                    self.private_values.get(&resolver),
                    Some(PrivateValue::DnsResolver)
                ) {
                    return Err(self.runtime_invariant());
                }
                let MirValue::Record { ref fields, .. } = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some((_, MirValue::String(name))) = fields.first() else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let limit = usize::from(
                    u16::try_from(unsigned(2)?)
                        .ok()
                        .filter(|value| *value > 0)
                        .ok_or(ExecutionError::TypeMismatch)?,
                );
                let addresses = (name.as_str(), 0)
                    .to_socket_addrs()
                    .map_err(|_| self.runtime_invariant())?;
                let mut answers = Vec::new();
                for address in addresses.map(|entry| entry.ip()) {
                    if !answers.contains(&address) {
                        answers.push(address);
                        if answers.len() == limit {
                            break;
                        }
                    }
                }
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::DnsAnswers(answers));
                Ok(MirValue::NetDnsAnswers(symbol))
            }
            86 if arguments.len() == 1 => {
                let MirValue::NetDnsResolver(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::DnsResolver)
                )))
            }
            87 if arguments.len() == 1 => {
                let MirValue::NetDnsAnswers(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::DnsAnswers(answers)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::try_from(answers.len()).map_err(|_| self.runtime_invariant())?,
                    IntegerKind::UInt64,
                )
            }
            88..=90 if arguments.len() >= 2 => {
                let MirValue::NetDnsAnswers(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let index =
                    usize::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::DnsAnswers(answers)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let Some(address) = answers.get(index) else {
                    return if function == 88 {
                        Err(self.runtime_invariant())
                    } else {
                        Ok(MirValue::Nil)
                    };
                };
                match (function, address) {
                    (88, IpAddr::V4(_)) => integer(4, IntegerKind::UInt8),
                    (88, IpAddr::V6(_)) => integer(6, IntegerKind::UInt8),
                    (89, IpAddr::V4(value)) => {
                        integer(u64::from(u32::from(*value)), IntegerKind::UInt32)
                    }
                    (89, IpAddr::V6(_)) => Ok(MirValue::Nil),
                    (90, IpAddr::V6(value)) if arguments.len() == 3 => {
                        let word = usize::from(
                            u8::try_from(unsigned(2)?).map_err(|_| ExecutionError::TypeMismatch)?,
                        );
                        if word >= 4 {
                            return Ok(MirValue::Nil);
                        }
                        let octets = value.octets();
                        let start = word * 4;
                        integer(
                            u64::from(u32::from_be_bytes(
                                octets[start..start + 4]
                                    .try_into()
                                    .map_err(|_| self.runtime_invariant())?,
                            )),
                            IntegerKind::UInt32,
                        )
                    }
                    (90, IpAddr::V4(_)) => Ok(MirValue::Nil),
                    _ => Err(ExecutionError::WrongArity),
                }
            }
            91 if arguments.len() == 1 => {
                let MirValue::NetDnsAnswers(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::DnsAnswers(_))
                )))
            }
            92 | 93 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let direction = if function == 92 {
                    Shutdown::Read
                } else {
                    Shutdown::Write
                };
                Ok(MirValue::Boolean(stream.shutdown(direction).is_ok()))
            }
            94 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Boolean(enabled) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                Ok(MirValue::Boolean(stream.set_nodelay(enabled).is_ok()))
            }
            95 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                Ok(MirValue::Boolean(
                    stream.nodelay().map_err(|_| self.runtime_invariant())?,
                ))
            }
            96 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let ttl = u32::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                Ok(MirValue::Boolean(stream.set_ttl(ttl).is_ok()))
            }
            97 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::from(stream.ttl().map_err(|_| self.runtime_invariant())?),
                    IntegerKind::UInt32,
                )
            }
            98..=104 if arguments.len() == usize::from(matches!(function, 100 | 101)) + 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let peer = matches!(function, 99 | 101 | 103 | 104);
                let endpoint = if peer {
                    stream.peer_addr()
                } else {
                    stream.local_addr()
                }
                .map_err(|_| self.runtime_invariant())?;
                match function {
                    98 | 99 => integer(
                        u64::from(if endpoint.is_ipv4() { 4_u8 } else { 6_u8 }),
                        IntegerKind::UInt8,
                    ),
                    100 | 101 => {
                        let index = usize::from(
                            u8::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?,
                        );
                        let word = match endpoint.ip() {
                            IpAddr::V4(value) if index == 0 => Some(u32::from(value)),
                            IpAddr::V6(value) if index < 4 => {
                                let octets = value.octets();
                                let start = index * 4;
                                Some(u32::from_be_bytes([
                                    octets[start],
                                    octets[start + 1],
                                    octets[start + 2],
                                    octets[start + 3],
                                ]))
                            }
                            _ => None,
                        };
                        word.map_or(Ok(MirValue::Nil), |word| {
                            integer(u64::from(word), IntegerKind::UInt32)
                        })
                    }
                    102 | 103 => integer(
                        u64::from(match endpoint {
                            std::net::SocketAddr::V4(_) => 0,
                            std::net::SocketAddr::V6(value) => value.scope_id(),
                        }),
                        IntegerKind::UInt32,
                    ),
                    104 => integer(u64::from(endpoint.port()), IntegerKind::UInt16),
                    _ => unreachable!(),
                }
            }
            105..=107 if arguments.len() == usize::from(function == 106) + 1 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let endpoint = socket.local_addr().map_err(|_| self.runtime_invariant())?;
                match function {
                    105 => integer(
                        u64::from(if endpoint.is_ipv4() { 4_u8 } else { 6_u8 }),
                        IntegerKind::UInt8,
                    ),
                    106 => {
                        let index = usize::from(
                            u8::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?,
                        );
                        let word = match endpoint.ip() {
                            IpAddr::V4(value) if index == 0 => Some(u32::from(value)),
                            IpAddr::V6(value) if index < 4 => {
                                let octets = value.octets();
                                let start = index * 4;
                                Some(u32::from_be_bytes([
                                    octets[start],
                                    octets[start + 1],
                                    octets[start + 2],
                                    octets[start + 3],
                                ]))
                            }
                            _ => None,
                        };
                        word.map_or(Ok(MirValue::Nil), |word| {
                            integer(u64::from(word), IntegerKind::UInt32)
                        })
                    }
                    _ => integer(
                        u64::from(match endpoint {
                            std::net::SocketAddr::V4(_) => 0,
                            std::net::SocketAddr::V6(value) => value.scope_id(),
                        }),
                        IntegerKind::UInt32,
                    ),
                }
            }
            108 | 110 if arguments.len() == 2 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let accepted = if function == 108 {
                    let MirValue::Boolean(enabled) = argument(1)?.visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    socket.set_broadcast(enabled)
                } else {
                    let ttl =
                        u32::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                    socket.set_ttl(ttl)
                };
                Ok(MirValue::Boolean(accepted.is_ok()))
            }
            109 | 111 if arguments.len() == 1 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                if function == 109 {
                    Ok(MirValue::Boolean(
                        socket.broadcast().map_err(|_| self.runtime_invariant())?,
                    ))
                } else {
                    integer(
                        u64::from(socket.ttl().map_err(|_| self.runtime_invariant())?),
                        IntegerKind::UInt32,
                    )
                }
            }
            112 | 113 if arguments.len() == 3 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let ipv4 = |index: usize| -> Result<Ipv4Addr, ExecutionError> {
                    let MirValue::Record { ref fields, .. } = argument(index)?.visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    let bits = &fields.first().ok_or(ExecutionError::TypeMismatch)?.1;
                    Ok(Ipv4Addr::from(
                        u32::try_from(integer_u64(bits)?)
                            .map_err(|_| ExecutionError::TypeMismatch)?,
                    ))
                };
                let group = ipv4(1)?;
                let interface = ipv4(2)?;
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let accepted = if function == 112 {
                    socket.join_multicast_v4(&group, &interface)
                } else {
                    socket.leave_multicast_v4(&group, &interface)
                };
                Ok(MirValue::Boolean(accepted.is_ok()))
            }
            145 if arguments.is_empty() => {
                let interfaces = capture_interfaces().ok_or_else(|| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::NetInterfaces(interfaces));
                Ok(MirValue::NetInterfacesSnapshot(symbol))
            }
            146 if arguments.len() == 1 => {
                let MirValue::NetInterfacesSnapshot(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::NetInterfaces(_))
                )))
            }
            147 if arguments.len() == 1 => {
                let MirValue::NetInterfacesSnapshot(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::NetInterfaces(interfaces)) =
                    self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::try_from(interfaces.len()).map_err(|_| self.runtime_invariant())?,
                    IntegerKind::UInt64,
                )
            }
            148..=155 => {
                let expected = if function >= 152 { 3 } else { 2 };
                if arguments.len() != expected + usize::from(function == 153) {
                    return Err(ExecutionError::WrongArity);
                }
                let MirValue::NetInterfacesSnapshot(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let interface_index =
                    usize::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::NetInterfaces(interfaces)) =
                    self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let Some(interface) = interfaces.get(interface_index) else {
                    return Err(self.runtime_invariant());
                };
                match function {
                    148 => Ok(MirValue::String(interface.name.clone())),
                    149 => integer(u64::from(interface.index), IntegerKind::UInt32),
                    150 => integer(u64::from(interface.flags), IntegerKind::UInt32),
                    151 => integer(
                        u64::try_from(interface.addresses.len())
                            .map_err(|_| self.runtime_invariant())?,
                        IntegerKind::UInt64,
                    ),
                    152..=155 => {
                        let address_index = usize::try_from(unsigned(2)?)
                            .map_err(|_| ExecutionError::TypeMismatch)?;
                        let Some(address) = interface.addresses.get(address_index) else {
                            return Err(self.runtime_invariant());
                        };
                        match function {
                            152 => integer(u64::from(address.family), IntegerKind::UInt8),
                            153 => {
                                let word = usize::from(
                                    u8::try_from(unsigned(3)?)
                                        .map_err(|_| ExecutionError::TypeMismatch)?,
                                );
                                let Some(value) = address.words.get(word) else {
                                    return Err(self.runtime_invariant());
                                };
                                integer(u64::from(*value), IntegerKind::UInt32)
                            }
                            154 => integer(u64::from(address.prefix), IntegerKind::UInt8),
                            _ => integer(u64::from(address.scope), IntegerKind::UInt32),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            156 if arguments.is_empty() => {
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::NetRoutes(capture_routes()));
                Ok(MirValue::NetRoutesSnapshot(symbol))
            }
            157 if arguments.len() == 1 => {
                let MirValue::NetRoutesSnapshot(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::NetRoutes(_))
                )))
            }
            158 if arguments.len() == 1 => {
                let MirValue::NetRoutesSnapshot(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::NetRoutes(routes)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                integer(
                    u64::try_from(routes.len()).map_err(|_| self.runtime_invariant())?,
                    IntegerKind::UInt64,
                )
            }
            159..=165 => {
                let expected = 2 + usize::from(matches!(function, 160 | 162));
                if arguments.len() != expected {
                    return Err(ExecutionError::WrongArity);
                }
                let MirValue::NetRoutesSnapshot(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let route_index =
                    usize::try_from(unsigned(1)?).map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::NetRoutes(routes)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let Some(route) = routes.get(route_index) else {
                    return Err(self.runtime_invariant());
                };
                match function {
                    159 => integer(u64::from(route.family), IntegerKind::UInt8),
                    160 | 162 => {
                        let word = usize::from(
                            u8::try_from(unsigned(2)?).map_err(|_| ExecutionError::TypeMismatch)?,
                        );
                        let words = if function == 160 {
                            &route.destination
                        } else {
                            &route.gateway
                        };
                        let Some(value) = words.get(word) else {
                            return Err(self.runtime_invariant());
                        };
                        integer(u64::from(*value), IntegerKind::UInt32)
                    }
                    161 => integer(u64::from(route.prefix), IntegerKind::UInt8),
                    163 => integer(u64::from(route.interface), IntegerKind::UInt32),
                    164 => integer(u64::from(route.metric), IntegerKind::UInt32),
                    165 => integer(u64::from(route.flags), IntegerKind::UInt32),
                    _ => unreachable!(),
                }
            }
            166 | 167 if arguments.len() == 3 => {
                let MirValue::NetUdpSocket(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Record {
                    fields: ref address,
                    ..
                } = argument(1)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Record {
                    fields: ref interface,
                    ..
                } = argument(2)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if address.len() != 4 || interface.len() != 1 {
                    return Err(ExecutionError::TypeMismatch);
                }
                let word = |index: usize| {
                    u32::try_from(integer_u64(&address[index].1)?)
                        .map_err(|_| ExecutionError::TypeMismatch)
                };
                let words = [word(0)?, word(1)?, word(2)?, word(3)?];
                let mut octets = [0_u8; 16];
                for (index, word) in words.into_iter().enumerate() {
                    octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                let interface = u32::try_from(integer_u64(&interface[0].1)?)
                    .map_err(|_| ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::UdpSocket(socket)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                let group = Ipv6Addr::from(octets);
                let accepted = if function == 166 {
                    socket.join_multicast_v6(&group, interface)
                } else {
                    socket.leave_multicast_v6(&group, interface)
                };
                Ok(MirValue::Boolean(accepted.is_ok()))
            }
            168 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Boolean(enabled) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                #[cfg(unix)]
                let accepted = interpreter_set_socket_i32(
                    stream,
                    libc::SOL_SOCKET,
                    libc::SO_KEEPALIVE,
                    i32::from(enabled),
                );
                #[cfg(not(unix))]
                let accepted = false;
                Ok(MirValue::Boolean(accepted))
            }
            169 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                #[cfg(unix)]
                let enabled = interpreter_socket_i32(stream, libc::SOL_SOCKET, libc::SO_KEEPALIVE)
                    .ok_or_else(|| self.runtime_invariant())?
                    != 0;
                #[cfg(not(unix))]
                let enabled = false;
                Ok(MirValue::Boolean(enabled))
            }
            170 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let milliseconds = unsigned(1)?;
                if milliseconds == 0 {
                    return Ok(MirValue::Boolean(false));
                }
                let seconds = milliseconds.saturating_add(999) / 1_000;
                let Ok(seconds) = i32::try_from(seconds) else {
                    return Ok(MirValue::Boolean(false));
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                #[cfg(any(target_os = "linux", target_os = "android"))]
                let accepted = interpreter_set_socket_i32(
                    stream,
                    libc::IPPROTO_TCP,
                    libc::TCP_KEEPIDLE,
                    seconds,
                );
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                let accepted = interpreter_set_socket_i32(
                    stream,
                    libc::IPPROTO_TCP,
                    libc::TCP_KEEPALIVE,
                    seconds,
                );
                #[cfg(not(any(
                    target_os = "linux",
                    target_os = "android",
                    target_os = "macos",
                    target_os = "ios"
                )))]
                let accepted = false;
                Ok(MirValue::Boolean(accepted))
            }
            171 if arguments.len() == 2 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let milliseconds = unsigned(1)?;
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                #[cfg(unix)]
                let accepted = interpreter_set_linger(stream, milliseconds);
                #[cfg(not(unix))]
                let accepted = false;
                Ok(MirValue::Boolean(accepted))
            }
            172 if arguments.len() == 1 => {
                let MirValue::NetTcpStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::TcpStream(stream)) = self.private_values.get(&symbol) else {
                    return Err(self.runtime_invariant());
                };
                #[cfg(unix)]
                let milliseconds =
                    interpreter_linger(stream).ok_or_else(|| self.runtime_invariant())?;
                #[cfg(not(unix))]
                let milliseconds = 0;
                integer(milliseconds, IntegerKind::UInt64)
            }
            173 if arguments.is_empty() => {
                install_interpreter_tls_provider();
                let config =
                    ClientConfig::with_platform_verifier().map_err(|_| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::TlsClientConfig(Arc::new(config)));
                Ok(MirValue::NetTlsClientConfig(symbol))
            }
            174 if arguments.len() == 1 => {
                install_interpreter_tls_provider();
                let MirValue::Bytes(reference) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .immutable_bytes_length(reference)
                    .map_err(|_| self.runtime_invariant())?;
                let mut certificate =
                    vec![0; usize::try_from(length).map_err(|_| ExecutionError::TypeMismatch)?];
                self.runtime
                    .immutable_bytes_read(reference, 0, &mut certificate)
                    .map_err(|_| self.runtime_invariant())?;
                let mut roots = RootCertStore::empty();
                roots
                    .add(CertificateDer::from(certificate))
                    .map_err(|_| self.runtime_invariant())?;
                let config = ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth();
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::TlsClientConfig(Arc::new(config)));
                Ok(MirValue::NetTlsClientConfig(symbol))
            }
            175 if arguments.len() == 2 => {
                install_interpreter_tls_provider();
                let bytes = |value: &RuntimeValue| -> Result<ManagedReference, ExecutionError> {
                    let MirValue::Bytes(reference) = value.visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    Ok(reference)
                };
                let certificate = bytes(argument(0)?)?;
                let private_key = bytes(argument(1)?)?;
                let read = |reference: ManagedReference,
                            runtime: &R|
                 -> Result<Vec<u8>, ExecutionError> {
                    let length = runtime
                        .immutable_bytes_length(reference)
                        .map_err(ExecutionError::Runtime)?;
                    let mut output =
                        vec![0; usize::try_from(length).map_err(|_| ExecutionError::TypeMismatch)?];
                    runtime
                        .immutable_bytes_read(reference, 0, &mut output)
                        .map_err(ExecutionError::Runtime)?;
                    Ok(output)
                };
                let certificate = read(certificate, self.runtime)?;
                let private_key = read(private_key, self.runtime)?;
                let config = ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(
                        vec![CertificateDer::from(certificate)],
                        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(private_key)),
                    )
                    .map_err(|_| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::TlsServerConfig(Arc::new(config)));
                Ok(MirValue::NetTlsServerConfig(symbol))
            }
            176 | 177 if arguments.len() == 1 => {
                let symbol = match (function, &argument(0)?.visible) {
                    (176, MirValue::NetTlsClientConfig(symbol))
                    | (177, MirValue::NetTlsServerConfig(symbol)) => *symbol,
                    _ => return Err(ExecutionError::TypeMismatch),
                };
                let closed = matches!(
                    (function, self.private_values.remove(&symbol)),
                    (176, Some(PrivateValue::TlsClientConfig(_)))
                        | (177, Some(PrivateValue::TlsServerConfig(_)))
                );
                Ok(MirValue::Boolean(closed))
            }
            178 | 179 if arguments.len() == if function == 178 { 5 } else { 4 } => {
                let (config_symbol, stream_symbol) =
                    match (function, &argument(0)?.visible, &argument(1)?.visible) {
                        (
                            178,
                            MirValue::NetTlsClientConfig(config),
                            MirValue::NetTcpStream(stream),
                        )
                        | (
                            179,
                            MirValue::NetTlsServerConfig(config),
                            MirValue::NetTcpStream(stream),
                        ) => (*config, *stream),
                        _ => return Err(ExecutionError::TypeMismatch),
                    };
                let deadline_index = if function == 178 { 3 } else { 2 };
                let cancellation_index = deadline_index + 1;
                let MirValue::TimeLiveDeadline(deadline_symbol) = argument(deadline_index)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::CancellationToken(cancellation_symbol) =
                    argument(cancellation_index)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let deadline = match self.private_values.get(&deadline_symbol) {
                    Some(PrivateValue::LiveDeadline { target, .. }) => *target,
                    _ => return Err(self.runtime_invariant()),
                };
                let cancellation = match self.private_values.get(&cancellation_symbol) {
                    Some(PrivateValue::CancellationToken(state)) => Rc::clone(state),
                    _ => return Err(self.runtime_invariant()),
                };
                let Some(PrivateValue::TcpStream(stream)) =
                    self.private_values.remove(&stream_symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let value = if function == 178 {
                    let config = match self.private_values.get(&config_symbol) {
                        Some(PrivateValue::TlsClientConfig(config)) => Arc::clone(config),
                        _ => return Err(self.runtime_invariant()),
                    };
                    let MirValue::String(ref name) = argument(2)?.visible else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    let server_name =
                        ServerName::try_from(name.clone()).map_err(|_| self.runtime_invariant())?;
                    let connection = ClientConnection::new(config, server_name)
                        .map_err(|_| self.runtime_invariant())?;
                    let mut stream = rustls::StreamOwned::new(connection, stream);
                    if !complete_interpreter_tls_handshake(deadline, &cancellation, || {
                        stream
                            .conn
                            .complete_io(&mut stream.sock)
                            .map(|_| stream.conn.is_handshaking())
                    }) {
                        return Err(self.runtime_invariant());
                    }
                    PrivateValue::TlsClientStream(stream)
                } else {
                    let config = match self.private_values.get(&config_symbol) {
                        Some(PrivateValue::TlsServerConfig(config)) => Arc::clone(config),
                        _ => return Err(self.runtime_invariant()),
                    };
                    let connection =
                        ServerConnection::new(config).map_err(|_| self.runtime_invariant())?;
                    let mut stream = rustls::StreamOwned::new(connection, stream);
                    if !complete_interpreter_tls_handshake(deadline, &cancellation, || {
                        stream
                            .conn
                            .complete_io(&mut stream.sock)
                            .map(|_| stream.conn.is_handshaking())
                    }) {
                        return Err(self.runtime_invariant());
                    }
                    PrivateValue::TlsServerStream(stream)
                };
                let symbol = self.fresh_private_symbol();
                self.private_values.insert(symbol, value);
                Ok(MirValue::NetTlsStream(symbol))
            }
            180 if arguments.len() == 2 => {
                let MirValue::NetTlsStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Bytes(reference) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .immutable_bytes_length(reference)
                    .map_err(|_| self.runtime_invariant())?;
                let mut bytes =
                    vec![0; usize::try_from(length).map_err(|_| ExecutionError::TypeMismatch)?];
                self.runtime
                    .immutable_bytes_read(reference, 0, &mut bytes)
                    .map_err(|_| self.runtime_invariant())?;
                let stream: &mut dyn std::io::Write = match self.private_values.get_mut(&symbol) {
                    Some(PrivateValue::TlsClientStream(stream)) => stream,
                    Some(PrivateValue::TlsServerStream(stream)) => stream,
                    _ => return Err(self.runtime_invariant()),
                };
                let (kind, count) = match stream.write(&bytes) {
                    Ok(0) if !bytes.is_empty() => (pop_types::SocketIoOutcomeKind::Closed, 0),
                    Ok(count) => (
                        pop_types::SocketIoOutcomeKind::Progress,
                        u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(error) if closed_error(&error) => {
                        (pop_types::SocketIoOutcomeKind::Closed, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            181 if arguments.len() == 3 => {
                let MirValue::NetTlsStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::ByteBuffer(buffer) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let capacity = usize::try_from(unsigned(2)?)
                    .ok()
                    .filter(|capacity| *capacity > 0)
                    .ok_or(ExecutionError::TypeMismatch)?;
                let mut bytes = vec![0; capacity];
                let stream: &mut dyn std::io::Read = match self.private_values.get_mut(&symbol) {
                    Some(PrivateValue::TlsClientStream(stream)) => stream,
                    Some(PrivateValue::TlsServerStream(stream)) => stream,
                    _ => return Err(self.runtime_invariant()),
                };
                let (kind, count) = match stream.read(&mut bytes) {
                    Ok(0) => (pop_types::SocketIoOutcomeKind::Closed, 0),
                    Ok(count) => {
                        self.runtime
                            .byte_buffer_append(buffer, &bytes[..count])
                            .map_err(|_| self.runtime_invariant())?;
                        (
                            pop_types::SocketIoOutcomeKind::Progress,
                            u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                        )
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(error) if closed_error(&error) => {
                        (pop_types::SocketIoOutcomeKind::Closed, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            182 if arguments.len() == 1 => {
                let MirValue::NetTlsStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::TlsClientStream(_) | PrivateValue::TlsServerStream(_))
                )))
            }
            #[cfg(unix)]
            114 | 115 if arguments.len() == 1 => {
                let MirValue::String(ref path) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if function == 114 {
                    let listener =
                        UnixListener::bind(path.as_str()).map_err(|_| self.runtime_invariant())?;
                    listener
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::UnixListener(listener));
                    Ok(MirValue::NetUnixListener(symbol))
                } else {
                    let stream =
                        UnixStream::connect(path.as_str()).map_err(|_| self.runtime_invariant())?;
                    stream
                        .set_nonblocking(true)
                        .map_err(|_| self.runtime_invariant())?;
                    let symbol = self.fresh_private_symbol();
                    self.private_values
                        .insert(symbol, PrivateValue::UnixStream(stream));
                    Ok(MirValue::NetUnixStream(symbol))
                }
            }
            #[cfg(unix)]
            116 if arguments.len() == 1 => {
                let MirValue::NetUnixListener(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let accepted = match self.private_values.get(&symbol) {
                    Some(PrivateValue::UnixListener(listener)) => listener.accept(),
                    _ => return Err(self.runtime_invariant()),
                };
                match accepted {
                    Ok((stream, _)) => {
                        stream
                            .set_nonblocking(true)
                            .map_err(|_| self.runtime_invariant())?;
                        let stream_symbol = self.fresh_private_symbol();
                        self.private_values
                            .insert(stream_symbol, PrivateValue::UnixStream(stream));
                        Ok(MirValue::NetUnixStream(stream_symbol))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        Ok(MirValue::Nil)
                    }
                    Err(_) => Err(self.runtime_invariant()),
                }
            }
            #[cfg(unix)]
            117 if arguments.len() == 2 => {
                let MirValue::NetUnixStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::Bytes(reference) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let length = self
                    .runtime
                    .immutable_bytes_length(reference)
                    .map_err(|_| self.runtime_invariant())?;
                let mut bytes =
                    vec![0; usize::try_from(length).map_err(|_| ExecutionError::TypeMismatch)?];
                self.runtime
                    .immutable_bytes_read(reference, 0, &mut bytes)
                    .map_err(|_| self.runtime_invariant())?;
                let Some(PrivateValue::UnixStream(stream)) = self.private_values.get_mut(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let (kind, count) = match stream.write(&bytes) {
                    Ok(0) if !bytes.is_empty() => (pop_types::SocketIoOutcomeKind::Closed, 0),
                    Ok(count) => (
                        pop_types::SocketIoOutcomeKind::Progress,
                        u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(error) if closed_error(&error) => {
                        (pop_types::SocketIoOutcomeKind::Closed, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            #[cfg(unix)]
            118 if arguments.len() == 3 => {
                let MirValue::NetUnixStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::ByteBuffer(buffer) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let capacity = usize::try_from(unsigned(2)?)
                    .ok()
                    .filter(|capacity| *capacity > 0)
                    .ok_or(ExecutionError::TypeMismatch)?;
                let Some(PrivateValue::UnixStream(stream)) = self.private_values.get_mut(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let mut bytes = vec![0; capacity];
                let (kind, count) = match stream.read(&mut bytes) {
                    Ok(0) => (pop_types::SocketIoOutcomeKind::Closed, 0),
                    Ok(count) => {
                        bytes.truncate(count);
                        self.runtime
                            .byte_buffer_append(buffer, &bytes)
                            .map_err(|_| self.runtime_invariant())?;
                        (
                            pop_types::SocketIoOutcomeKind::Progress,
                            u64::try_from(count).map_err(|_| self.runtime_invariant())?,
                        )
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        (pop_types::SocketIoOutcomeKind::WouldBlock, 0)
                    }
                    Err(error) if closed_error(&error) => {
                        (pop_types::SocketIoOutcomeKind::Closed, 0)
                    }
                    Err(_) => return Err(self.runtime_invariant()),
                };
                Ok(MirValue::NetTransfer { kind, count })
            }
            #[cfg(unix)]
            119 | 120 if arguments.len() == 1 => {
                let MirValue::NetUnixStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::UnixStream(stream)) = self.private_values.get(&symbol)
                else {
                    return Err(self.runtime_invariant());
                };
                let direction = if function == 119 {
                    Shutdown::Read
                } else {
                    Shutdown::Write
                };
                Ok(MirValue::Boolean(stream.shutdown(direction).is_ok()))
            }
            #[cfg(unix)]
            121 if arguments.len() == 1 => {
                let MirValue::NetUnixListener(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&symbol),
                    Some(PrivateValue::UnixListener(_))
                )))
            }
            #[cfg(unix)]
            122 if arguments.len() == 1 => {
                let MirValue::NetUnixStream(symbol) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let removed = self.private_values.remove(&symbol);
                if let Some(PrivateValue::UnixStream(stream)) = &removed {
                    let _ = stream.shutdown(Shutdown::Both);
                }
                Ok(MirValue::Boolean(matches!(
                    removed,
                    Some(PrivateValue::UnixStream(_))
                )))
            }
            133 | 134 | 135 | 143 | 144 => {
                let (base, base_count, deadline_index, cancel_index) = match function {
                    133 => (64, 2, 2, 3),
                    134 => (65, 3, 3, 4),
                    135 => (70, 4, 4, 5),
                    143 => (117, 2, 2, 3),
                    144 => (118, 3, 3, 4),
                    _ => unreachable!(),
                };
                if arguments.len() != cancel_index + 1 {
                    return Err(ExecutionError::WrongArity);
                }
                let deadline = argument(deadline_index)?.visible.clone();
                let cancel = argument(cancel_index)?.visible.clone();
                loop {
                    if self.wait_cancelled(&cancel)? {
                        break Ok(MirValue::NetWaitTransfer { kind: 3, count: 0 });
                    }
                    let attempt =
                        self.evaluate_net_standard_call(base, &arguments[..base_count], values)?;
                    let MirValue::NetTransfer { kind, count } = attempt else {
                        return Err(ExecutionError::TypeMismatch);
                    };
                    match kind {
                        pop_types::SocketIoOutcomeKind::Progress => {
                            break Ok(MirValue::NetWaitTransfer { kind: 0, count });
                        }
                        pop_types::SocketIoOutcomeKind::Closed => {
                            break Ok(MirValue::NetWaitTransfer { kind: 1, count: 0 });
                        }
                        pop_types::SocketIoOutcomeKind::WouldBlock => {
                            if self.wait_deadline_retry(&deadline)? {
                                continue;
                            }
                            break Ok(MirValue::NetWaitTransfer { kind: 2, count: 0 });
                        }
                    }
                }
            }
            136 if arguments.len() == 5 => {
                let deadline = argument(3)?.visible.clone();
                let cancel = argument(4)?.visible.clone();
                loop {
                    if self.wait_cancelled(&cancel)? {
                        break Ok(MirValue::NetUdpWaitTransfer {
                            kind: 3,
                            address: 0,
                            port: 0,
                            count: 0,
                        });
                    }
                    match self.evaluate_net_standard_call(71, &arguments[..3], values)? {
                        MirValue::NetUdpTransfer {
                            address,
                            port,
                            count,
                        } => {
                            break Ok(MirValue::NetUdpWaitTransfer {
                                kind: 0,
                                address,
                                port,
                                count: u64::from(count),
                            });
                        }
                        MirValue::Nil => {
                            if self.wait_deadline_retry(&deadline)? {
                                continue;
                            }
                            break Ok(MirValue::NetUdpWaitTransfer {
                                kind: 2,
                                address: 0,
                                port: 0,
                                count: 0,
                            });
                        }
                        _ => return Err(ExecutionError::TypeMismatch),
                    }
                }
            }
            128..=131 if arguments.len() == 1 => {
                let MirValue::NetWaitTransfer { kind, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(
                    kind == u8::try_from(function - 128).unwrap_or(u8::MAX),
                ))
            }
            132 if arguments.len() == 1 => {
                let MirValue::NetWaitTransfer { count, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                integer(count, IntegerKind::UInt64)
            }
            137..=139 if arguments.len() == 1 => {
                let MirValue::NetUdpWaitTransfer { kind, .. } = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let expected = match function {
                    137 => 0,
                    138 => 2,
                    _ => 3,
                };
                Ok(MirValue::Boolean(kind == expected))
            }
            140..=142 if arguments.len() == 1 => {
                let MirValue::NetUdpWaitTransfer {
                    address,
                    port,
                    count,
                    ..
                } = argument(0)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                match function {
                    140 => integer(count, IntegerKind::UInt64),
                    141 => integer(u64::from(address), IntegerKind::UInt32),
                    _ => integer(u64::from(port), IntegerKind::UInt16),
                }
            }
            _ => Err(ExecutionError::WrongArity),
        }
    }

    fn wait_cancelled(&mut self, cancel: &MirValue) -> Result<bool, ExecutionError> {
        let MirValue::CancellationToken(symbol) = cancel else {
            return Err(ExecutionError::TypeMismatch);
        };
        let Some(PrivateValue::CancellationToken(state)) = self.private_values.get(symbol) else {
            return Err(self.runtime_invariant());
        };
        Ok(state.borrow().requested)
    }

    fn wait_deadline_retry(&mut self, deadline: &MirValue) -> Result<bool, ExecutionError> {
        let MirValue::TimeLiveDeadline(symbol) = deadline else {
            return Err(ExecutionError::TypeMismatch);
        };
        let Some(PrivateValue::LiveDeadline { target, .. }) = self.private_values.get(symbol)
        else {
            return Err(self.runtime_invariant());
        };
        let remaining = target.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(1)));
        Ok(true)
    }

    fn evaluate_live_time_standard_call(
        &mut self,
        function: u32,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<MirValue, ExecutionError> {
        let argument = |index: usize| {
            arguments
                .get(index)
                .copied()
                .ok_or(ExecutionError::WrongArity)
                .and_then(|argument| value(values, argument))
        };
        match function {
            123 if arguments.is_empty() => {
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::MonotonicClock(Instant::now()));
                Ok(MirValue::TimeMonotonicClock(symbol))
            }
            124 if arguments.len() == 2 => {
                let MirValue::TimeMonotonicClock(clock) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::MonotonicClock(origin)) = self.private_values.get(&clock)
                else {
                    return Err(self.runtime_invariant());
                };
                let now = origin
                    .checked_add(origin.elapsed())
                    .ok_or_else(|| self.runtime_invariant())?;
                let milliseconds = integer_u64(&argument(1)?.visible)?;
                let target = now
                    .checked_add(Duration::from_millis(milliseconds))
                    .ok_or_else(|| self.runtime_invariant())?;
                let symbol = self.fresh_private_symbol();
                self.private_values
                    .insert(symbol, PrivateValue::LiveDeadline { clock, target });
                Ok(MirValue::TimeLiveDeadline(symbol))
            }
            125 if arguments.len() == 2 => {
                let MirValue::TimeMonotonicClock(clock) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let MirValue::TimeLiveDeadline(deadline) = argument(1)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if !matches!(
                    self.private_values.get(&clock),
                    Some(PrivateValue::MonotonicClock(_))
                ) {
                    return Err(self.runtime_invariant());
                }
                let Some(PrivateValue::LiveDeadline {
                    clock: owner,
                    target,
                }) = self.private_values.get(&deadline)
                else {
                    return Err(self.runtime_invariant());
                };
                if *owner != clock {
                    return Err(self.runtime_invariant());
                }
                Ok(MirValue::Boolean(Instant::now() >= *target))
            }
            126 if arguments.len() == 1 => {
                let MirValue::TimeLiveDeadline(deadline) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(MirValue::Boolean(matches!(
                    self.private_values.remove(&deadline),
                    Some(PrivateValue::LiveDeadline { .. })
                )))
            }
            127 if arguments.len() == 1 => {
                let MirValue::TimeMonotonicClock(clock) = argument(0)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                if !matches!(
                    self.private_values.remove(&clock),
                    Some(PrivateValue::MonotonicClock(_))
                ) {
                    return Ok(MirValue::Boolean(false));
                }
                self.private_values.retain(|_, value| {
                    !matches!(value, PrivateValue::LiveDeadline { clock: owner, .. } if *owner == clock)
                });
                Ok(MirValue::Boolean(true))
            }
            _ => Err(ExecutionError::WrongArity),
        }
    }

    fn verify_ffi_alignment(
        &mut self,
        address: ForeignAddress,
        alignment: u64,
    ) -> Result<(), ExecutionError> {
        if alignment != 0 && address.raw().is_multiple_of(alignment) {
            Ok(())
        } else {
            Err(self.runtime_invariant())
        }
    }

    fn evaluate_effect_instruction(
        &mut self,
        instruction: &pop_mir::MirInstruction,
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<(), ExecutionError> {
        let returned = match instruction.kind() {
            MirInstructionKind::ViewEnd { .. } => return Ok(()),
            MirInstructionKind::CallStandard {
                function,
                arguments,
                ..
            } => {
                if arguments.len() != 1 {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                match (function.raw(), &value(values, arguments[0])?.visible) {
                    (0, MirValue::Integer(value)) => {
                        let value = value.signed().ok_or(ExecutionError::TypeMismatch)?;
                        pop_standard::pop_std_print_int(value);
                    }
                    (1, MirValue::String(value)) => pop_standard::print_string(value),
                    (0 | 1, _) => return Err(ExecutionError::TypeMismatch),
                    (219 | 220, MirValue::String(_)) => {}
                    _ => return Err(ExecutionError::InvalidControlFlow),
                }
                return Ok(());
            }
            MirInstructionKind::CallDirect {
                function,
                arguments,
                ..
            } => self.execute_direct_call(*function, arguments, values)?,
            MirInstructionKind::CallForeign {
                function,
                arguments,
                roots,
                safe_point,
                ..
            } => self.execute_foreign_call(
                *function,
                arguments,
                roots,
                *safe_point,
                instruction.effects(),
                values,
            )?,
            MirInstructionKind::CallReferenced { function, .. } => {
                return Err(ExecutionError::UnknownReferencedFunction(*function));
            }
            MirInstructionKind::CallDirectMethod {
                method, arguments, ..
            } => self.execute_method_call(*method, arguments, values)?,
            MirInstructionKind::CallIndirect {
                callee, arguments, ..
            } => self.execute_indirect_call(*callee, arguments, values)?,
            MirInstructionKind::CallInterface {
                method, arguments, ..
            } => {
                let receiver = arguments.first().ok_or(ExecutionError::WrongArity)?;
                let MirValue::Class(class) = &value(values, *receiver)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let implementation = self
                    .mir
                    .declarations()
                    .iter()
                    .find_map(|declaration| match declaration.kind() {
                        pop_mir::MirDeclarationKind::Class(class_declaration)
                            if class_declaration.class() == class.class() =>
                        {
                            class_declaration
                                .interfaces()
                                .iter()
                                .flat_map(pop_mir::MirInterfaceImplementation::methods)
                                .find(|candidate| candidate.interface_method() == *method)
                                .map(|candidate| candidate.class_method())
                        }
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.execute_method_call(implementation, arguments, values)?
            }
            MirInstructionKind::CaptureCellStore {
                cell,
                value: stored,
            } => {
                let MirValue::Function(symbol) = value(values, *cell)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::Cell(cell)) = self.private_values.get(&symbol) else {
                    return Err(ExecutionError::TypeMismatch);
                };
                *cell.borrow_mut() = value(values, *stored)?.clone();
                return Ok(());
            }
            MirInstructionKind::CaptureStore {
                capture,
                value: stored,
                ..
            } => {
                let environment = self
                    .active_captures
                    .as_ref()
                    .ok_or(ExecutionError::InvalidControlFlow)?
                    .clone();
                let slot = capture.raw() as usize;
                let stored = value(values, *stored)?.clone();
                let mut captures = environment.borrow_mut();
                let target = captures
                    .get_mut(slot)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                if let MirValue::Function(symbol) = &target.visible
                    && let Some(PrivateValue::Cell(cell)) = self.private_values.get(symbol)
                {
                    *cell.borrow_mut() = stored;
                } else {
                    *target = stored;
                }
                return Ok(());
            }
            MirInstructionKind::GcSafePoint {
                roots, stack_map, ..
            } => {
                let published_values = roots
                    .iter()
                    .map(|root| value(values, *root).map(|value| value.reference))
                    .collect::<Result<_, _>>()?;
                let mut publication = RootPublication::new(stack_map.clone(), published_values)
                    .map_err(|_| ExecutionError::InvalidControlFlow)?;
                self.runtime
                    .safe_point(&mut publication)
                    .map_err(ExecutionError::Runtime)?;
                install_published_relocations(roots, &publication, values)?;
                return Ok(());
            }
            MirInstructionKind::RetainRoot { value: root } => {
                let reference = value(values, *root)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let handle = self
                    .runtime
                    .retain_root(reference)
                    .map_err(ExecutionError::Runtime)?;
                self.root_handles.insert(instruction.result(), handle);
                return Ok(());
            }
            MirInstructionKind::ReleaseRoot { handle } => {
                let handle = self
                    .root_handles
                    .remove(handle)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.runtime
                    .release_root(handle)
                    .map_err(ExecutionError::Runtime)?;
                return Ok(());
            }
            MirInstructionKind::FfiHandleClose { handle } => {
                let MirValue::FfiHandle(raw) = value(values, *handle)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let handle = RootHandle::new(raw);
                self.runtime
                    .release_root(handle)
                    .map_err(ExecutionError::Runtime)?;
                self.ffi_handles.remove(&handle).ok_or_else(|| {
                    ExecutionError::Runtime(
                        self.runtime
                            .raise_trap(Trap::new(TrapKind::ImpossibleState)),
                    )
                })?;
                return Ok(());
            }
            MirInstructionKind::FfiBufferWrite {
                buffer,
                index,
                value: stored,
                layout,
            } => {
                let reference = value(values, *buffer)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let index = integer_u64(&value(values, *index)?.visible)?;
                let entry = self
                    .mir
                    .ffi_layouts()
                    .get(*layout)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let bytes = marshal(
                    &value(values, *stored)?.visible,
                    entry,
                    self.mir.ffi_layouts(),
                )?;
                self.runtime
                    .ffi_buffer_write(reference, *layout, index, &bytes)
                    .map_err(|_| self.runtime_invariant())?;
                return Ok(());
            }
            MirInstructionKind::FfiBufferEndBorrow { buffer, region } => {
                let reference = value(values, *buffer)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let borrow = self
                    .ffi_buffer_borrows
                    .get(region)
                    .copied()
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.runtime
                    .ffi_buffer_end_borrow(reference, borrow)
                    .map_err(|_| self.runtime_invariant())?;
                self.ffi_buffer_borrows.remove(region);
                return Ok(());
            }
            MirInstructionKind::FfiBytesEndBorrow { bytes, region } => {
                let owner = value(values, *bytes)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let state = self
                    .ffi_bytes_borrows
                    .get(region)
                    .copied()
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                if state.owner != owner {
                    return Err(self.runtime_invariant());
                }
                self.runtime
                    .ffi_bytes_end_borrow(owner, state.borrow)
                    .map_err(|_| self.runtime_invariant())?;
                self.ffi_bytes_borrows.remove(region);
                return Ok(());
            }
            MirInstructionKind::FfiCallbackCloseScoped { callback, .. } => {
                let MirValue::FfiRegisteredCallback { registration, .. } =
                    &value(values, *callback)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let id = FfiCallbackRegistrationId::new(*registration)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let state = self
                    .ffi_callbacks
                    .get_mut(&id)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                if state.closed {
                    return Err(ExecutionError::InvalidControlFlow);
                }
                match self
                    .runtime
                    .ffi_callback_close(id, state.registration.context(), state.site)
                {
                    Ok(()) => state.closed = true,
                    Err(FfiCallbackCloseFailure::InUse | FfiCallbackCloseFailure::Invariant(_)) => {
                        return Err(self.runtime_invariant());
                    }
                }
                return Ok(());
            }
            MirInstructionKind::FfiBufferClose { buffer } => {
                let reference = value(values, *buffer)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                self.runtime
                    .ffi_buffer_close(reference)
                    .map_err(|_| self.runtime_invariant())?;
                return Ok(());
            }
            MirInstructionKind::FfiUnsafeStore {
                pointer,
                value: stored,
                layout,
            } => {
                let address = ffi_pointer(&value(values, *pointer)?.visible)?;
                let entry = self
                    .mir
                    .ffi_layouts()
                    .get(*layout)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.verify_ffi_alignment(address, entry.alignment())?;
                let bytes = marshal(
                    &value(values, *stored)?.visible,
                    entry,
                    self.mir.ffi_layouts(),
                )?;
                self.runtime
                    .ffi_unsafe_write(address, &bytes)
                    .map_err(|_| self.runtime_invariant())?;
                return Ok(());
            }
            MirInstructionKind::FfiUnsafeCopy {
                source,
                destination,
                count,
                layout,
            } => {
                let source = ffi_pointer(&value(values, *source)?.visible)?;
                let destination = ffi_pointer(&value(values, *destination)?.visible)?;
                let count = integer_u64(&value(values, *count)?.visible)?;
                let entry = self
                    .mir
                    .ffi_layouts()
                    .get(*layout)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.verify_ffi_alignment(source, entry.alignment())?;
                self.verify_ffi_alignment(destination, entry.alignment())?;
                let byte_count = count
                    .checked_mul(entry.size())
                    .ok_or_else(|| self.integer_overflow())?;
                self.runtime
                    .ffi_unsafe_copy(source, destination, byte_count)
                    .map_err(|_| self.runtime_invariant())?;
                return Ok(());
            }
            MirInstructionKind::Pin { value: pinned } => {
                let reference = value(values, *pinned)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let handle = self
                    .runtime
                    .pin(reference)
                    .map_err(ExecutionError::Runtime)?;
                self.pin_handles.insert(instruction.result(), handle);
                return Ok(());
            }
            MirInstructionKind::Unpin { handle } => {
                let handle = self
                    .pin_handles
                    .remove(handle)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                self.runtime
                    .unpin(handle)
                    .map_err(ExecutionError::Runtime)?;
                return Ok(());
            }
            MirInstructionKind::WriteBarrier {
                owner,
                slot,
                previous,
                value: stored,
                proof,
            } => {
                if proof.is_some() {
                    return Ok(());
                }
                let owner = value(values, *owner)?
                    .reference
                    .ok_or(ExecutionError::TypeMismatch)?;
                let previous = previous
                    .map(|previous| value(values, previous).map(|value| value.reference))
                    .transpose()?
                    .flatten();
                let stored = stored
                    .map(|stored| value(values, stored).map(|value| value.reference))
                    .transpose()?
                    .flatten();
                self.runtime
                    .write_barrier(WriteBarrier::new(
                        BarrierKind::CombinedSatbGenerational,
                        owner,
                        *slot,
                        previous,
                        stored,
                    ))
                    .map_err(ExecutionError::Runtime)?;
                return Ok(());
            }
            _ => return Err(ExecutionError::InvalidControlFlow),
        };
        if returned.is_empty() {
            Ok(())
        } else {
            Err(ExecutionError::WrongArity)
        }
    }

    fn evaluate_structured_instruction(
        &mut self,
        instruction: &MirInstruction,
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, ExecutionError> {
        let result = match instruction.kind() {
            MirInstructionKind::CallDirect {
                function,
                arguments,
                ..
            } => single_result(self.execute_direct_call(*function, arguments, values)?),
            MirInstructionKind::CallForeign {
                function,
                arguments,
                roots,
                safe_point,
                ..
            } => single_result(self.execute_foreign_call(
                *function,
                arguments,
                roots,
                *safe_point,
                instruction.effects(),
                values,
            )?),
            MirInstructionKind::CallReferenced { function, .. } => {
                return Err(ExecutionError::UnknownReferencedFunction(*function));
            }
            MirInstructionKind::CallDirectMethod {
                method, arguments, ..
            } => single_result(self.execute_method_call(*method, arguments, values)?),
            MirInstructionKind::CallIndirect {
                callee, arguments, ..
            } => single_result(self.execute_indirect_call(*callee, arguments, values)?),
            MirInstructionKind::CallScopedBorrow {
                owner,
                function,
                captures,
                arguments,
                ..
            } => single_result(
                self.execute_scoped_borrow_call(*owner, *function, captures, arguments, values)?,
            ),
            MirInstructionKind::CallCallbackPair {
                callback,
                owner,
                function,
                captures,
                lifetime,
                result,
                success,
                failure,
                ..
            } => {
                let MirValue::FfiRegisteredCallback { registration, .. } =
                    &value(values, *callback)?.visible
                else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let id = FfiCallbackRegistrationId::new(*registration)
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                let state = self
                    .ffi_callbacks
                    .get(&id)
                    .cloned()
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                if state.closed {
                    if *lifetime != FfiCallbackLifetime::Registered {
                        return Err(ExecutionError::InvalidControlFlow);
                    }
                    let (Some(result), Some(failure)) = (result, failure) else {
                        return Err(ExecutionError::InvalidControlFlow);
                    };
                    return Ok(Some(RuntimeValue::visible(MirValue::Result {
                        definition: *result,
                        case: *failure,
                        arguments: vec![MirValue::FfiCallbackClosedError],
                    })));
                }
                let arguments = [
                    RuntimeValue::visible(MirValue::FfiFunction(state.site.raw())),
                    RuntimeValue::visible(MirValue::FfiPointer(state.registration.context())),
                ];
                let returned = self
                    .execute_callback_pair_call(*owner, *function, captures, &arguments, values)?;
                let returned = single_result(returned)?;
                if *lifetime == FfiCallbackLifetime::CallScoped {
                    Ok(returned)
                } else {
                    let (Some(result), Some(success)) = (result, success) else {
                        return Err(ExecutionError::InvalidControlFlow);
                    };
                    Ok(RuntimeValue::visible(MirValue::Result {
                        definition: *result,
                        case: *success,
                        arguments: vec![returned.visible],
                    }))
                }
            }
            MirInstructionKind::CallInterface {
                method, arguments, ..
            } => {
                let receiver = arguments.first().ok_or(ExecutionError::WrongArity)?;
                let MirValue::Class(class) = &value(values, *receiver)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let implementation = self
                    .mir
                    .declarations()
                    .iter()
                    .find_map(|declaration| match declaration.kind() {
                        pop_mir::MirDeclarationKind::Class(class_declaration)
                            if class_declaration.class() == class.class() =>
                        {
                            class_declaration
                                .interfaces()
                                .iter()
                                .flat_map(pop_mir::MirInterfaceImplementation::methods)
                                .find(|implementation| implementation.interface_method() == *method)
                                .map(|implementation| implementation.class_method())
                        }
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidControlFlow)?;
                single_result(self.execute_method_call(implementation, arguments, values)?)
            }
            MirInstructionKind::CallBuiltinInterface {
                method, arguments, ..
            } => {
                if arguments.len() != 1 {
                    return Err(ExecutionError::WrongArity);
                }
                let receiver = value(values, arguments[0])?.clone();
                if let MirValue::Class(class) = &receiver.visible {
                    let implementation = self
                        .mir
                        .declarations()
                        .iter()
                        .find_map(|declaration| match declaration.kind() {
                            pop_mir::MirDeclarationKind::Class(class_declaration)
                                if class_declaration.class() == class.class() =>
                            {
                                class_declaration
                                    .builtin_interfaces()
                                    .iter()
                                    .flat_map(pop_mir::MirBuiltinInterfaceImplementation::methods)
                                    .find(|implementation| {
                                        implementation.protocol_method() == *method
                                    })
                                    .map(|implementation| implementation.class_method())
                            }
                            _ => None,
                        })
                        .ok_or(ExecutionError::InvalidControlFlow)?;
                    return single_result(self.execute_method_call(
                        implementation,
                        arguments,
                        values,
                    )?)
                    .map(Some);
                }
                if method.raw() == 0 {
                    if let MirValue::Function(symbol) = &receiver.visible
                        && matches!(
                            self.private_values.get(symbol),
                            Some(PrivateValue::Iterator { .. })
                        )
                    {
                        Ok(receiver)
                    } else {
                        self.allocate_iteration_session(instruction.result_type(), receiver)
                    }
                } else if method.raw() == 1 {
                    self.advance_iteration_session(instruction.result_type(), &receiver, values)
                } else {
                    return Err(ExecutionError::InvalidControlFlow);
                }
            }
            MirInstructionKind::CaptureCellAllocate {
                initial,
                object_map,
                ..
            } => {
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map.clone(),
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let cell = Rc::new(RefCell::new(value(values, *initial)?.clone()));
                let symbol = self.fresh_private_symbol();
                self.private_values.insert(symbol, PrivateValue::Cell(cell));
                Ok(RuntimeValue::managed(MirValue::Function(symbol), reference))
            }
            MirInstructionKind::CaptureCellLoad { cell } => {
                let MirValue::Function(symbol) = value(values, *cell)?.visible else {
                    return Err(ExecutionError::TypeMismatch);
                };
                let Some(PrivateValue::Cell(cell)) = self.private_values.get(&symbol) else {
                    return Err(ExecutionError::TypeMismatch);
                };
                Ok(cell.borrow().clone())
            }
            MirInstructionKind::CaptureLoad { capture, .. } => {
                let environment = self
                    .active_captures
                    .as_ref()
                    .ok_or(ExecutionError::InvalidControlFlow)?
                    .borrow();
                let captured = environment
                    .get(capture.raw() as usize)
                    .ok_or(ExecutionError::InvalidControlFlow)?
                    .clone();
                let MirValue::Function(symbol) = captured.visible else {
                    return Ok(Some(captured));
                };
                match self.private_values.get(&symbol) {
                    Some(PrivateValue::Cell(cell)) => Ok(cell.borrow().clone()),
                    Some(PrivateValue::Closure { .. } | PrivateValue::Iterator { .. }) => {
                        Ok(captured)
                    }
                    Some(
                        PrivateValue::Task(_)
                        | PrivateValue::CancellationSource(_)
                        | PrivateValue::CancellationToken(_)
                        | PrivateValue::TaskGroup(_)
                        | PrivateValue::Channel(_)
                        | PrivateValue::Actor(_)
                        | PrivateValue::AtomicInt(_)
                        | PrivateValue::AtomicBoolean(_)
                        | PrivateValue::TcpListener(_)
                        | PrivateValue::TcpStream(_)
                        | PrivateValue::FileAccess(_)
                        | PrivateValue::DirectoryAccess(_)
                        | PrivateValue::FileHandle(_)
                        | PrivateValue::FileWriteHandle(_)
                        | PrivateValue::DirectorySnapshot(_)
                        | PrivateValue::TlsClientConfig(_)
                        | PrivateValue::TlsServerConfig(_)
                        | PrivateValue::TlsClientStream(_)
                        | PrivateValue::TlsServerStream(_)
                        | PrivateValue::UdpSocket(_)
                        | PrivateValue::DnsResolver
                        | PrivateValue::DnsAnswers(_)
                        | PrivateValue::NetInterfaces(_)
                        | PrivateValue::NetRoutes(_)
                        | PrivateValue::MonotonicClock(_)
                        | PrivateValue::LiveDeadline { .. },
                    ) => Err(ExecutionError::TypeMismatch),
                    #[cfg(unix)]
                    Some(PrivateValue::UnixListener(_) | PrivateValue::UnixStream(_)) => {
                        Err(ExecutionError::TypeMismatch)
                    }
                    None => Err(ExecutionError::TypeMismatch),
                }
            }
            MirInstructionKind::CaptureCellReference { capture, .. } => {
                let captures = self
                    .active_captures
                    .as_ref()
                    .ok_or(ExecutionError::InvalidControlFlow)?
                    .borrow();
                captures
                    .get(capture.raw() as usize)
                    .cloned()
                    .ok_or(ExecutionError::InvalidControlFlow)
            }
            MirInstructionKind::ClosureEnvironmentAllocate {
                owner,
                function,
                captures,
                object_map,
                ..
            } => {
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map.clone(),
                    ))
                    .map_err(ExecutionError::Runtime)?;
                let self_slots: Vec<_> = captures
                    .iter()
                    .filter(|capture| capture.self_reference())
                    .map(|capture| capture.slot() as usize)
                    .collect();
                let environment_values = captures
                    .iter()
                    .map(|capture| {
                        if capture.self_reference() {
                            Ok(RuntimeValue::visible(MirValue::Nil))
                        } else {
                            value(values, capture.value()).cloned()
                        }
                    })
                    .collect::<Result<Vec<_>, ExecutionError>>()?;
                let symbol = self.fresh_private_symbol();
                let environment = Rc::new(RefCell::new(environment_values));
                self.private_values.insert(
                    symbol,
                    PrivateValue::Closure {
                        owner: *owner,
                        function: *function,
                        captures: environment.clone(),
                    },
                );
                let closure = RuntimeValue::managed(MirValue::Function(symbol), reference);
                for slot in self_slots {
                    environment.borrow_mut()[slot] = closure.clone();
                }
                Ok(closure)
            }
            MirInstructionKind::RecordMake { record, fields, .. } => {
                Ok(RuntimeValue::visible(MirValue::Record {
                    record: *record,
                    fields: evaluate_visible_fields(fields, values)?,
                }))
            }
            MirInstructionKind::ClassMake {
                class,
                fields,
                object_map,
                ..
            } => {
                let definition = canonical_class_identity(
                    self.mir,
                    self.arena,
                    *class,
                    instruction.result_type(),
                )
                .ok_or(ExecutionError::InvalidControlFlow)?;
                let reference = self
                    .runtime
                    .allocate_object(&ObjectAllocationRequest::new(
                        RuntimeTypeId::new(instruction.result_type().raw()),
                        AllocationClass::NurseryEligible,
                        object_map.clone(),
                    ))
                    .map_err(ExecutionError::Runtime)?;
                Ok(RuntimeValue::managed(
                    MirValue::Class(MirClassValue::new(
                        *class,
                        definition,
                        reference,
                        evaluate_fields(fields, values)?,
                    )),
                    reference,
                ))
            }
            MirInstructionKind::RecordUpdate {
                record,
                base,
                fields,
                ..
            } => update_record(*record, *base, fields, values),
            MirInstructionKind::FieldGet { base, field } => get_field(*base, *field, values),
            MirInstructionKind::FieldSet {
                base,
                field,
                value: new_value,
            } => set_field(*base, *field, *new_value, values),
            MirInstructionKind::UnionMake {
                union,
                case,
                arguments,
                ..
            } => Ok(RuntimeValue::visible(MirValue::Union {
                union: *union,
                case: *case,
                arguments: arguments
                    .iter()
                    .map(|argument| value(values, *argument).map(|value| value.visible.clone()))
                    .collect::<Result<_, _>>()?,
            })),
            MirInstructionKind::ResultMake {
                result,
                case,
                arguments,
                ..
            } => Ok(RuntimeValue::visible(MirValue::Result {
                definition: *result,
                case: *case,
                arguments: arguments
                    .iter()
                    .map(|argument| value(values, *argument).map(|value| value.visible.clone()))
                    .collect::<Result<_, _>>()?,
            })),
            MirInstructionKind::IterationMake {
                iteration,
                case,
                arguments,
                ..
            } => Ok(RuntimeValue::visible(MirValue::Iteration {
                definition: *iteration,
                case: *case,
                arguments: arguments
                    .iter()
                    .map(|argument| value(values, *argument).map(|value| value.visible.clone()))
                    .collect::<Result<_, _>>()?,
            })),
            MirInstructionKind::ErrorMake {
                error,
                case,
                arguments,
                ..
            } => Ok(RuntimeValue::visible(MirValue::Error {
                error: *error,
                case: *case,
                arguments: arguments
                    .iter()
                    .map(|argument| value(values, *argument).map(|value| value.visible.clone()))
                    .collect::<Result<_, _>>()?,
            })),
            MirInstructionKind::InterfaceUpcast { value: base, .. } => {
                Ok(value(values, *base)?.clone())
            }
            MirInstructionKind::CheckedDowncast {
                value: base,
                target_type,
                target_class,
                ..
            } => {
                let candidate = value(values, *base)?;
                match &candidate.visible {
                    MirValue::Class(class)
                        if class_is_or_descends_from(
                            self.mir,
                            self.arena,
                            class.definition(),
                            *target_class,
                            *target_type,
                        ) =>
                    {
                        Ok(candidate.clone())
                    }
                    MirValue::Class(_) => Ok(RuntimeValue::visible(MirValue::Nil)),
                    _ => Err(ExecutionError::TypeMismatch),
                }
            }
            _ => return Ok(None),
        }?;
        Ok(Some(result))
    }

    fn allocate_iteration_session(
        &mut self,
        iterator_type: TypeId,
        source: RuntimeValue,
    ) -> Result<RuntimeValue, ExecutionError> {
        let (expected_length, range_current) = match &source.visible {
            MirValue::Array(elements) => (elements.len(), None),
            MirValue::List(elements) => (elements.len(), None),
            MirValue::String(text) => (text.len(), None),
            MirValue::Table(entries) => (entries.len(), None),
            MirValue::Range { first, .. } => (0, Some(*first)),
            _ => return Err(ExecutionError::TypeMismatch),
        };
        let reference_slots = source
            .reference
            .map(|_| vec![ObjectSlot::new(0)])
            .unwrap_or_default();
        let object_map = ObjectMap::new(u32::from(source.reference.is_some()), reference_slots)
            .map_err(|_| ExecutionError::InvalidControlFlow)?;
        let reference = self
            .runtime
            .allocate_object(&ObjectAllocationRequest::new(
                RuntimeTypeId::new(iterator_type.raw()),
                AllocationClass::NurseryEligible,
                object_map,
            ))
            .map_err(ExecutionError::Runtime)?;
        let symbol = self.fresh_private_symbol();
        self.private_values.insert(
            symbol,
            PrivateValue::Iterator {
                source,
                expected_length,
                position: 0,
                range_current,
                range_started: false,
            },
        );
        Ok(RuntimeValue::managed(MirValue::Function(symbol), reference))
    }

    fn advance_iteration_session(
        &mut self,
        iteration_type: TypeId,
        iterator: &RuntimeValue,
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<RuntimeValue, ExecutionError> {
        let MirValue::Function(symbol) = iterator.visible else {
            return Err(ExecutionError::TypeMismatch);
        };
        let (source, expected_length, position, range_current, range_started) =
            match self.private_values.get(&symbol) {
                Some(PrivateValue::Iterator {
                    source,
                    expected_length,
                    position,
                    range_current,
                    range_started,
                }) => (
                    source.clone(),
                    *expected_length,
                    *position,
                    *range_current,
                    *range_started,
                ),
                _ => return Err(ExecutionError::TypeMismatch),
            };
        let current = source.reference.and_then(|owner| {
            values
                .values()
                .find(|candidate| candidate.reference == Some(owner))
                .cloned()
        });
        let current = current.as_ref().unwrap_or(&source);
        let (length, item, next_range, next_position) = match &current.visible {
            MirValue::Array(elements) => {
                (elements.len(), elements.get(position).cloned(), None, None)
            }
            MirValue::List(elements) => {
                (elements.len(), elements.get(position).cloned(), None, None)
            }
            MirValue::String(text) => {
                let item = text.get(position..).and_then(|remaining| {
                    remaining
                        .chars()
                        .next()
                        .map(|value| MirValue::Rune(u32::from(value)))
                });
                let next_position = item.as_ref().and_then(|item| match item {
                    MirValue::Rune(value) => char::from_u32(*value)
                        .map(|value| position.saturating_add(value.len_utf8())),
                    _ => None,
                });
                (text.len(), item, None, next_position)
            }
            MirValue::Table(entries) => (
                entries.len(),
                entries
                    .get(position)
                    .map(|(key, value)| MirValue::Tuple(vec![key.clone(), value.clone()])),
                None,
                None,
            ),
            MirValue::Range { last, step, .. } => {
                let Some(current) = range_current else {
                    return self.iteration_result(iteration_type, None);
                };
                let next = if range_started {
                    current.checked_add(*step).map_err(|error| match error {
                        pop_types::NumericError::KindMismatch => ExecutionError::TypeMismatch,
                        _ => ExecutionError::Runtime(
                            self.runtime
                                .raise_trap(Trap::new(TrapKind::IntegerOverflow)),
                        ),
                    })?
                } else {
                    current
                };
                let ordering = next
                    .compare(*last)
                    .map_err(|_| ExecutionError::TypeMismatch)?;
                let positive = step.signed().map_or_else(
                    || step.unsigned().is_some_and(|value| value > 0),
                    |value| value > 0,
                );
                let in_range = if positive {
                    !ordering.is_gt()
                } else {
                    !ordering.is_lt()
                };
                if !in_range {
                    if let Some(PrivateValue::Iterator { range_current, .. }) =
                        self.private_values.get_mut(&symbol)
                    {
                        *range_current = None;
                    }
                    return self.iteration_result(iteration_type, None);
                }
                let following = (!ordering.is_eq()).then_some(next);
                (0, Some(MirValue::Integer(next)), following, None)
            }
            _ => return Err(ExecutionError::TypeMismatch),
        };
        if !matches!(current.visible, MirValue::Range { .. }) && length != expected_length {
            return Err(ExecutionError::Runtime(
                self.runtime
                    .raise_trap(Trap::new(TrapKind::ConcurrentModification)),
            ));
        }
        if item.is_some()
            && let Some(PrivateValue::Iterator {
                position,
                range_current,
                range_started,
                ..
            }) = self.private_values.get_mut(&symbol)
        {
            if matches!(current.visible, MirValue::Range { .. }) {
                *range_current = next_range;
                *range_started = true;
            } else {
                *position = next_position.unwrap_or_else(|| position.saturating_add(1));
            }
        }
        self.iteration_result(iteration_type, item)
    }

    fn iteration_result(
        &self,
        iteration_type: TypeId,
        item: Option<MirValue>,
    ) -> Result<RuntimeValue, ExecutionError> {
        let definition = match self.arena.get(iteration_type) {
            Some(SemanticType::Builtin { definition, .. }) => *definition,
            _ => return Err(ExecutionError::TypeMismatch),
        };
        Ok(RuntimeValue::visible(MirValue::Iteration {
            definition,
            case: pop_foundation::IterationCaseId::from_raw(u32::from(item.is_none())),
            arguments: item.into_iter().collect(),
        }))
    }

    fn execute_direct_call(
        &mut self,
        function: SymbolId,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let arguments = evaluated_arguments(arguments, values)?;
        self.call(function, &arguments)
    }

    fn execute_method_call(
        &mut self,
        method: pop_foundation::MethodId,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let arguments = evaluated_arguments(arguments, values)?;
        let function = self
            .mir
            .methods()
            .iter()
            .find(|candidate| candidate.method() == method)
            .ok_or(ExecutionError::InvalidControlFlow)?
            .function();
        if function.parameters().len() != arguments.len() {
            return Err(ExecutionError::WrongArity);
        }
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::CallDepthLimit)?;
        if self.depth > self.limits.maximum_call_depth {
            return Err(ExecutionError::CallDepthLimit);
        }
        let returned = self.execute(
            function.parameters(),
            function.results(),
            function.blocks(),
            &arguments,
            None,
        );
        self.depth -= 1;
        returned
    }

    fn execute_indirect_call(
        &mut self,
        callee: ValueId,
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let callee = value(values, callee)?.clone();
        let arguments = evaluated_arguments(arguments, values)?;
        self.execute_indirect_value(&callee, &arguments)
    }

    fn execute_scoped_borrow_call(
        &mut self,
        owner: SymbolId,
        function: NestedFunctionId,
        captures: &[pop_mir::MirClosureCapture],
        arguments: &[ValueId],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        if captures.iter().any(|capture| capture.self_reference()) {
            return Err(ExecutionError::InvalidControlFlow);
        }
        let nested = self
            .mir
            .nested_functions()
            .iter()
            .find(|candidate| candidate.owner() == owner && candidate.function() == function)
            .ok_or(ExecutionError::InvalidControlFlow)?;
        let capture_values = captures
            .iter()
            .map(|capture| value(values, capture.value()).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let arguments = evaluated_arguments(arguments, values)?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::CallDepthLimit)?;
        if self.depth > self.limits.maximum_call_depth {
            return Err(ExecutionError::CallDepthLimit);
        }
        let result = self.execute(
            nested.parameters(),
            nested.results(),
            nested.blocks(),
            &arguments,
            Some(Rc::new(RefCell::new(capture_values))),
        );
        self.depth -= 1;
        result
    }

    fn execute_callback_pair_call(
        &mut self,
        owner: SymbolId,
        function: NestedFunctionId,
        captures: &[pop_mir::MirClosureCapture],
        arguments: &[RuntimeValue],
        values: &BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        if captures.iter().any(|capture| capture.self_reference()) {
            return Err(ExecutionError::InvalidControlFlow);
        }
        let nested = self
            .mir
            .nested_functions()
            .iter()
            .find(|candidate| candidate.owner() == owner && candidate.function() == function)
            .ok_or(ExecutionError::InvalidControlFlow)?;
        let capture_values = captures
            .iter()
            .map(|capture| value(values, capture.value()).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::CallDepthLimit)?;
        if self.depth > self.limits.maximum_call_depth {
            return Err(ExecutionError::CallDepthLimit);
        }
        let result = self.execute(
            nested.parameters(),
            nested.results(),
            nested.blocks(),
            arguments,
            Some(Rc::new(RefCell::new(capture_values))),
        );
        self.depth -= 1;
        result
    }

    fn interpreter_callback_target(
        &self,
        callback: &RuntimeValue,
        expected_owner: SymbolId,
        expected_function: NestedFunctionId,
    ) -> Result<InterpreterCallbackTarget, ExecutionError> {
        let MirValue::Function(symbol) = callback.visible else {
            return Err(ExecutionError::TypeMismatch);
        };
        match self.private_values.get(&symbol) {
            Some(PrivateValue::Closure {
                owner,
                function,
                captures,
            }) if *owner == expected_owner && *function == expected_function => {
                Ok(InterpreterCallbackTarget::Closure {
                    owner: *owner,
                    function: *function,
                    captures: captures.clone(),
                })
            }
            _ => Err(ExecutionError::InvalidControlFlow),
        }
    }

    fn execute_callback_target(
        &mut self,
        target: &InterpreterCallbackTarget,
        arguments: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let InterpreterCallbackTarget::Closure {
            owner,
            function,
            captures,
        } = target;
        let nested = self
            .mir
            .nested_functions()
            .iter()
            .find(|candidate| candidate.owner() == *owner && candidate.function() == *function)
            .ok_or(ExecutionError::InvalidControlFlow)?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::CallDepthLimit)?;
        if self.depth > self.limits.maximum_call_depth {
            return Err(ExecutionError::CallDepthLimit);
        }
        let returned = self.execute(
            nested.parameters(),
            nested.results(),
            nested.blocks(),
            arguments,
            Some(captures.clone()),
        );
        self.depth -= 1;
        returned
    }

    fn invoke_ffi_callback(
        &mut self,
        function: &MirValue,
        context: &MirValue,
        arguments: &[MirValue],
    ) -> Result<Vec<MirValue>, ExecutionError> {
        let MirValue::FfiFunction(raw_site) = function else {
            return Err(ExecutionError::TypeMismatch);
        };
        let context = ffi_pointer(context)?;
        let site = FfiCallbackSiteId::new(*raw_site).ok_or(ExecutionError::TypeMismatch)?;
        let state = self
            .ffi_callbacks
            .values()
            .find(|state| state.site == site && state.registration.context() == context)
            .cloned()
            .filter(|state| !state.closed)
            .ok_or(ExecutionError::UnsupportedFfiCallback {
                function: *raw_site,
                context,
            })?;
        let InterpreterCallbackTarget::Closure {
            owner,
            function: callback,
            ..
        } = &state.target;
        let nested = self
            .mir
            .nested_functions()
            .iter()
            .find(|candidate| candidate.owner() == *owner && candidate.function() == *callback)
            .ok_or(ExecutionError::InvalidControlFlow)?;
        if nested.parameters().len() != arguments.len() || nested.results().len() != 1 {
            return Err(ExecutionError::WrongArity);
        }
        let mut context_parameters = 0_u8;
        for (parameter, argument) in nested.parameters().iter().zip(arguments) {
            let is_context = matches!(
                self.arena.get(*parameter),
                Some(SemanticType::Builtin { definition, arguments })
                    if *definition == FFI_CALLBACK_CONTEXT_TYPE_ID && arguments.is_empty()
            );
            if is_context {
                context_parameters = context_parameters.saturating_add(1);
                if argument != &MirValue::FfiPointer(context) {
                    return Err(ExecutionError::TypeMismatch);
                }
            }
            if !callback_abi_value_matches(self.mir, self.arena, *parameter, argument)? {
                return Err(ExecutionError::TypeMismatch);
            }
        }
        if context_parameters != 1 {
            return Err(ExecutionError::InvalidControlFlow);
        }
        let entry = self
            .runtime
            .ffi_callback_enter(context, site)
            .map_err(ExecutionError::Runtime)?;
        if entry.environment() != Some(state.environment) {
            let _ = self.runtime.ffi_callback_leave(entry.transition());
            return Err(self.runtime_invariant());
        }
        let runtime_arguments = arguments
            .iter()
            .cloned()
            .map(RuntimeValue::visible)
            .collect::<Vec<_>>();
        let invocation = self.execute_callback_target(&state.target, &runtime_arguments);
        self.runtime
            .ffi_callback_leave(entry.transition())
            .map_err(ExecutionError::Runtime)?;
        let returned = invocation?;
        let [returned] = returned.as_slice() else {
            return Err(ExecutionError::WrongArity);
        };
        if !callback_abi_value_matches(
            self.mir,
            self.arena,
            nested.results()[0],
            &returned.visible,
        )? {
            return Err(ExecutionError::TypeMismatch);
        }
        Ok(vec![returned.visible.clone()])
    }

    fn execute_indirect_value(
        &mut self,
        callee: &RuntimeValue,
        arguments: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, ExecutionError> {
        let MirValue::Function(function) = &callee.visible else {
            return Err(ExecutionError::TypeMismatch);
        };
        let closure = match self.private_values.get(function) {
            Some(PrivateValue::Closure {
                owner,
                function,
                captures,
            }) => Some((*owner, *function, captures.clone())),
            _ => None,
        };
        if let Some((owner, function, captures)) = closure {
            let nested = self
                .mir
                .nested_functions()
                .iter()
                .find(|candidate| candidate.owner() == owner && candidate.function() == function)
                .ok_or(ExecutionError::InvalidControlFlow)?;
            self.depth = self
                .depth
                .checked_add(1)
                .ok_or(ExecutionError::CallDepthLimit)?;
            if self.depth > self.limits.maximum_call_depth {
                return Err(ExecutionError::CallDepthLimit);
            }
            let result = self.execute(
                nested.parameters(),
                nested.results(),
                nested.blocks(),
                arguments,
                Some(captures),
            );
            self.depth -= 1;
            result
        } else {
            self.call(*function, arguments)
        }
    }

    fn fresh_private_symbol(&mut self) -> SymbolId {
        let symbol = SymbolId::from_raw(self.next_private_value);
        self.next_private_value = self.next_private_value.saturating_sub(1);
        symbol
    }

    fn assign_block_arguments(
        blocks: &[pop_mir::MirBlock],
        target: pop_foundation::BlockId,
        arguments: &[ValueId],
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<(), ExecutionError> {
        let target = blocks
            .get(target.raw() as usize)
            .ok_or(ExecutionError::InvalidControlFlow)?;
        if target.arguments().len() != arguments.len() {
            return Err(ExecutionError::WrongArity);
        }
        let incoming: Result<Vec<_>, _> = arguments
            .iter()
            .map(|argument| value(values, *argument).cloned())
            .collect();
        for (parameter, incoming) in target.arguments().iter().zip(incoming?) {
            values.insert(parameter.value(), incoming);
        }
        Ok(())
    }

    fn assign_runtime_block_arguments(
        blocks: &[pop_mir::MirBlock],
        target: pop_foundation::BlockId,
        arguments: &[MirValue],
        values: &mut BTreeMap<ValueId, RuntimeValue>,
    ) -> Result<(), ExecutionError> {
        let target = blocks
            .get(target.raw() as usize)
            .ok_or(ExecutionError::InvalidControlFlow)?;
        if target.arguments().len() != arguments.len() {
            return Err(ExecutionError::WrongArity);
        }
        for (parameter, argument) in target.arguments().iter().zip(arguments) {
            values.insert(parameter.value(), RuntimeValue::visible(argument.clone()));
        }
        Ok(())
    }

    fn step(&mut self) -> Result<(), ExecutionError> {
        self.steps = self.steps.checked_add(1).ok_or(ExecutionError::StepLimit)?;
        if self.steps > self.limits.maximum_steps {
            Err(ExecutionError::StepLimit)
        } else {
            Ok(())
        }
    }
}

fn checked_view_range(owner_length: usize, start: i64, length: i64) -> Option<(usize, usize)> {
    let start = start
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())?;
    let length = usize::try_from(length).ok()?;
    let end = start.checked_add(length)?;
    if end > owner_length || (length == 0 && start > owner_length) {
        return None;
    }
    if length != 0 && start >= owner_length {
        return None;
    }
    Some((start, length))
}

fn install_published_relocations(
    roots: &[ValueId],
    publication: &RootPublication,
    values: &mut BTreeMap<ValueId, RuntimeValue>,
) -> Result<(), ExecutionError> {
    for (root, (_, relocated)) in roots.iter().copied().zip(publication.root_values()) {
        let previous = value(values, root)?.reference;
        if previous.is_some() != relocated.is_some() {
            return Err(ExecutionError::Runtime(RuntimeFailure::runtime_invariant()));
        }
        for candidate in values.values_mut() {
            if candidate.reference == previous {
                candidate.install_relocated_reference(relocated)?;
            }
        }
    }
    Ok(())
}

fn scalar_byte_offset(text: &str, scalar_index: usize) -> Option<usize> {
    if scalar_index == text.chars().count() {
        return Some(text.len());
    }
    text.char_indices()
        .nth(scalar_index)
        .map(|(offset, _)| offset)
}

fn view_text(view: &MirViewValue) -> Result<&str, ExecutionError> {
    let MirViewLenderValue::Text(text) = &view.lender else {
        return Err(ExecutionError::TypeMismatch);
    };
    let end = view
        .byte_offset
        .checked_add(view.byte_length)
        .filter(|end| *end <= text.len())
        .ok_or(ExecutionError::InvalidControlFlow)?;
    text.get(view.byte_offset..end)
        .ok_or(ExecutionError::InvalidControlFlow)
}

fn view_bytes_reference(view: &MirViewValue) -> Result<ManagedReference, ExecutionError> {
    match &view.lender {
        MirViewLenderValue::Bytes(reference) => Ok(*reference),
        MirViewLenderValue::Text(_) => Err(ExecutionError::TypeMismatch),
    }
}

fn class_definition(bubble: &MirBubble, class: ClassId) -> Option<SymbolIdentity> {
    bubble
        .declarations()
        .iter()
        .find_map(|declaration| match declaration.kind() {
            MirDeclarationKind::Class(candidate) if candidate.class() == class => {
                Some(candidate.definition())
            }
            _ => None,
        })
        .or_else(|| {
            bubble
                .nominal_references()
                .classes()
                .iter()
                .find(|reference| reference.class() == class)
                .map(|reference| reference.identity().definition())
        })
}

fn canonical_class_identity(
    bubble: &MirBubble,
    arena: &TypeArena,
    class: ClassId,
    type_id: TypeId,
) -> Option<pop_types::CanonicalNominalIdentity> {
    if let Some(reference) = bubble
        .nominal_references()
        .classes()
        .iter()
        .find(|reference| reference.class() == class && reference.type_id() == type_id)
    {
        return Some(reference.identity().canonical().clone());
    }
    let definition = class_definition(bubble, class)?;
    let SemanticType::Class {
        class: found,
        arguments,
    } = arena.get(type_id)?
    else {
        return None;
    };
    if *found != class {
        return None;
    }
    Some(pop_types::CanonicalNominalIdentity::new(
        definition,
        arguments
            .iter()
            .map(|argument| canonical_type_identity(bubble, arena, *argument))
            .collect::<Option<Vec<_>>>()?,
    ))
}

fn canonical_type_identity(
    bubble: &MirBubble,
    arena: &TypeArena,
    type_id: TypeId,
) -> Option<pop_types::CanonicalTypeIdentity> {
    use pop_types::CanonicalTypeIdentity as Canonical;
    Some(match arena.get(type_id)? {
        SemanticType::Primitive(primitive) => Canonical::Primitive(*primitive),
        SemanticType::Record(_) => {
            let declaration = bubble.declarations().iter().find(|declaration| {
                matches!(declaration.kind(), MirDeclarationKind::Record(record)
                    if record.type_id() == type_id)
            })?;
            Canonical::Record(SymbolIdentity::new(bubble.bubble(), declaration.symbol()))
        }
        SemanticType::Class { class, .. } => {
            Canonical::Class(canonical_class_identity(bubble, arena, *class, type_id)?)
        }
        SemanticType::Interface {
            interface,
            arguments,
        } => {
            if let Some(reference) =
                bubble
                    .nominal_references()
                    .interfaces()
                    .iter()
                    .find(|reference| {
                        reference.interface() == *interface && reference.type_id() == type_id
                    })
            {
                Canonical::Interface(reference.identity().canonical().clone())
            } else {
                let declaration = bubble.declarations().iter().find(|declaration| {
                    matches!(declaration.kind(), MirDeclarationKind::Interface(candidate)
                        if candidate.interface() == *interface)
                })?;
                Canonical::Interface(pop_types::CanonicalNominalIdentity::new(
                    SymbolIdentity::new(bubble.bubble(), declaration.symbol()),
                    arguments
                        .iter()
                        .map(|argument| canonical_type_identity(bubble, arena, *argument))
                        .collect::<Option<Vec<_>>>()?,
                ))
            }
        }
        SemanticType::Tuple(elements) => Canonical::Tuple(
            elements
                .iter()
                .map(|element| canonical_type_identity(bubble, arena, *element))
                .collect::<Option<Vec<_>>>()?,
        ),
        SemanticType::Function {
            is_async,
            parameters,
            results,
            effects,
            lifetime_summary,
        } => Canonical::Function {
            is_async: *is_async,
            parameters: parameters
                .iter()
                .map(|parameter| canonical_type_identity(bubble, arena, *parameter))
                .collect::<Option<Vec<_>>>()?,
            results: results
                .iter()
                .map(|result| canonical_type_identity(bubble, arena, *result))
                .collect::<Option<Vec<_>>>()?,
            effects: *effects,
            lifetime_summary: lifetime_summary.clone(),
        },
        SemanticType::Array(element) => {
            Canonical::Array(Box::new(canonical_type_identity(bubble, arena, *element)?))
        }
        SemanticType::Table { key, value } => Canonical::Table {
            key: Box::new(canonical_type_identity(bubble, arena, *key)?),
            value: Box::new(canonical_type_identity(bubble, arena, *value)?),
        },
        SemanticType::Optional(element) => {
            Canonical::Optional(Box::new(canonical_type_identity(bubble, arena, *element)?))
        }
        SemanticType::Builtin {
            definition,
            arguments,
        } => Canonical::Builtin {
            definition: *definition,
            arguments: arguments
                .iter()
                .map(|argument| canonical_type_identity(bubble, arena, *argument))
                .collect::<Option<Vec<_>>>()?,
        },
        SemanticType::Union(elements) => Canonical::Union(
            elements
                .iter()
                .map(|element| canonical_type_identity(bubble, arena, *element))
                .collect::<Option<Vec<_>>>()?,
        ),
        SemanticType::TaggedUnion { .. }
        | SemanticType::ErrorUnion { .. }
        | SemanticType::Enum { .. }
        | SemanticType::Attribute { .. }
        | SemanticType::TypeParameter(_)
        | SemanticType::Opaque(_)
        | SemanticType::Error => return None,
    })
}

fn class_is_or_descends_from(
    bubble: &MirBubble,
    arena: &TypeArena,
    concrete: &pop_types::CanonicalNominalIdentity,
    target: ClassId,
    target_type: TypeId,
) -> bool {
    let Some(target) = canonical_class_identity(bubble, arena, target, target_type) else {
        return false;
    };
    let mut classes = BTreeMap::new();
    for declaration in bubble.declarations() {
        let MirDeclarationKind::Class(class) = declaration.kind() else {
            continue;
        };
        let Some(identity) =
            canonical_class_identity(bubble, arena, class.class(), class.type_id())
        else {
            continue;
        };
        let base = class.base().and_then(|base| {
            bubble
                .declarations()
                .iter()
                .find_map(|declaration| match declaration.kind() {
                    MirDeclarationKind::Class(base_class) if base_class.class() == base => {
                        canonical_class_identity(bubble, arena, base, base_class.type_id())
                    }
                    _ => None,
                })
        });
        classes.insert(identity, base);
    }
    for reference in bubble.nominal_references().classes() {
        let base = reference
            .base()
            .zip(reference.base_type())
            .and_then(|(base, base_type)| {
                bubble
                    .nominal_references()
                    .classes()
                    .iter()
                    .find(|candidate| candidate.class() == base && candidate.type_id() == base_type)
                    .map(|candidate| candidate.identity().canonical().clone())
            });
        classes.insert(reference.identity().canonical().clone(), base);
    }
    let mut current = concrete.clone();
    let mut visited = BTreeSet::new();
    while visited.insert(current.clone()) {
        if current == target {
            return true;
        }
        let Some(base) = classes.get(&current).cloned().flatten() else {
            return false;
        };
        current = base;
    }
    false
}

impl<R: RuntimeAdapter> FfiCallbackInvoker for Engine<'_, '_, R> {
    fn invoke(
        &mut self,
        function: &MirValue,
        context: &MirValue,
        arguments: &[MirValue],
    ) -> Result<Vec<MirValue>, ExecutionError> {
        self.invoke_ffi_callback(function, context, arguments)
    }
}
