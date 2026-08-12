#[test]
fn numeric_interface_scope_remains_explicit_and_host_free() {
    let source = include_str!("../pop/src/netScope.pop");
    for declaration in [
        "public record InterfaceId",
        "public record ScopedIpv6Address",
        "public function interfaceId(index: UInt32): InterfaceId?",
        "public function scopedIpv6(address: Ipv6Address, identity: InterfaceId): ScopedIpv6Address",
        "public function parseScopedIpv6(text: String): ScopedIpv6Address?",
        "public function formatScopedIpv6(value: ScopedIpv6Address): String",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Dynamic",
        "Any",
        "lookup",
        "interfaces(",
        "Dns.",
        "Socket.",
        "runtime",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden scope behavior: {forbidden}"
        );
    }
}
