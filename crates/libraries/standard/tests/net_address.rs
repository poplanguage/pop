#[test]
fn closed_ip_address_union_remains_static_and_family_preserving() {
    let source = include_str!("../pop/src/netAddress.pop");
    for declaration in [
        "public union Address",
        "Ipv4(value: Ipv4Address)",
        "Ipv6(value: Ipv6Address)",
        "public function parseAddress(text: String): Address?",
        "public function formatAddress(address: Address): String",
        "public function isAddressLoopback(address: Address): Boolean",
        "public function isAddressUnspecified(address: Address): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Dynamic", "Any", "Table", "reflect", "Dns.", "Socket.", "connect(", "runtime",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden erased/host surface: {forbidden}"
        );
    }
}
