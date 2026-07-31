#[test]
fn ipv4_prefix_and_socket_values_remain_pure_and_distinct() {
    let source = include_str!("../pop/src/netIpv4Endpoint.pop");
    for declaration in [
        "public record Ipv4Prefix",
        "public record Ipv4SocketAddress",
        "public function ipv4Prefix(address: Ipv4Address, length: Int): Ipv4Prefix?",
        "public function networkIpv4(prefix: Ipv4Prefix): Ipv4Address",
        "public function containsIpv4(prefix: Ipv4Prefix, address: Ipv4Address): Boolean",
        "public function ipv4Socket(address: Ipv4Address, port: UInt16): Ipv4SocketAddress",
        "public function parseIpv4Socket(text: String): Ipv4SocketAddress?",
        "public function formatIpv4Socket(value: Ipv4SocketAddress): String",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Dynamic", "Any", "Dns.", "Socket.", "bind(", "connect(", "runtime",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden host/dynamic surface: {forbidden}"
        );
    }
}
