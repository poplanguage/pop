#[test]
fn canonical_ipv4_source_keeps_the_pure_static_contract() {
    let source = include_str!("../pop/src/net.pop");

    for declaration in [
        "public record Ipv4Address",
        "public function ipv4(first: Byte, second: Byte, third: Byte, fourth: Byte): Ipv4Address",
        "public function parseIpv4(text: String): Ipv4Address?",
        "public function formatIpv4(address: Ipv4Address): String",
        "public function ipv4Octet(address: Ipv4Address, index: Int): Byte?",
        "public function isIpv4Loopback(address: Ipv4Address): Boolean",
        "public function isIpv4Private(address: Ipv4Address): Boolean",
        "public function isIpv4LinkLocal(address: Ipv4Address): Boolean",
        "public function isIpv4Multicast(address: Ipv4Address): Boolean",
        "public function isIpv4Broadcast(address: Ipv4Address): Boolean",
        "public function isIpv4Documentation(address: Ipv4Address): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Dns.",
        "Socket.",
        "connect(",
        "RustNet.",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Net.Ipv4Address must remain ordinary portable static Pop: {forbidden}"
        );
    }
}

#[test]
fn canonical_ipv6_source_keeps_the_pure_static_contract() {
    let source = include_str!("../pop/src/netIpv6.pop");

    for declaration in [
        "public record Ipv6Address",
        "public function ipv6(",
        "public function parseIpv6(text: String): Ipv6Address?",
        "public function formatIpv6(address: Ipv6Address): String",
        "public function ipv6Segment(address: Ipv6Address, index: Int): UInt16?",
        "public function isIpv6Loopback(address: Ipv6Address): Boolean",
        "public function isIpv6Unspecified(address: Ipv6Address): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Dns.",
        "Socket.",
        "connect(",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Net.Ipv6Address must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
