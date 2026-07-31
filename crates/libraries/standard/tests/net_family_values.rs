#[test]
fn closed_prefix_and_socket_unions_remain_static_and_exhaustive() {
    let source = include_str!("../pop/src/netFamilyValues.pop");
    for declaration in [
        "public union Prefix",
        "Ipv4(value: Ipv4Prefix)",
        "Ipv6(value: Ipv6Prefix)",
        "public union SocketAddress",
        "Ipv4(value: Ipv4SocketAddress)",
        "Ipv6(value: Ipv6SocketAddress)",
        "public function networkAddress(prefix: Prefix): Address",
        "public function containsAddress(prefix: Prefix, address: Address): Boolean",
        "public function parseSocketAddress(text: String): SocketAddress?",
        "public function formatSocketAddress(value: SocketAddress): String",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in ["Dynamic", "Any", "Table", "Dns.", "connect(", "runtime"] {
        assert!(
            !source.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
}
