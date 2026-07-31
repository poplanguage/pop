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
            "Net.Ipv4Address must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
