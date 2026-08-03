use pop_runtime_interface::{
    CLOSED_RUNTIME_ABI_OPERATIONS, RuntimeAbiType, RuntimeOperation, runtime_abi_signature,
};

#[test]
fn atomic_actor_and_network_operations_have_closed_native_signatures() {
    for operation in CLOSED_RUNTIME_ABI_OPERATIONS {
        assert!(runtime_abi_signature(operation).is_some(), "{operation:?}");
    }
}

#[test]
fn signatures_preserve_widths_and_pointer_direction() {
    use RuntimeAbiType::{
        ReadOnlyU8Pointer, U8, U16, U32, U64, WritableU8Pointer, WritableU16Pointer,
        WritableU32Pointer,
    };

    let tcp_send = runtime_abi_signature(RuntimeOperation::TcpSend).expect("TCP send signature");
    assert_eq!(tcp_send.parameters(), &[U64, ReadOnlyU8Pointer, U64]);
    assert_eq!(tcp_send.result(), U64);

    let udp_receive =
        runtime_abi_signature(RuntimeOperation::UdpReceive).expect("UDP receive signature");
    assert_eq!(
        udp_receive.parameters(),
        &[
            U64,
            WritableU8Pointer,
            U64,
            WritableU32Pointer,
            WritableU16Pointer,
        ]
    );
    assert_eq!(udp_receive.result(), U64);

    let local_port =
        runtime_abi_signature(RuntimeOperation::TcpLocalPort).expect("local port signature");
    assert_eq!(local_port.parameters(), &[U64, WritableU16Pointer]);
    assert_eq!(local_port.result(), U8);

    let send_to = runtime_abi_signature(RuntimeOperation::UdpSendTo).expect("UDP send signature");
    assert_eq!(send_to.parameters()[1..3], [U32, U16]);
}
