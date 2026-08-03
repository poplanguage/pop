#[test]
fn bounded_dns_names_are_pure_canonical_values() {
    let source = include_str!("../pop/src/netDns.pop");
    for declaration in [
        "public record DnsName",
        "public function parseDnsName(text: String): DnsName?",
        "public function formatDnsName(name: DnsName): String",
        "public function dnsNameLabelCount(name: DnsName): UInt32",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Dns.lookup",
        "resolve",
        "Environment",
        "Socket.",
        "runtime",
        "Dynamic",
        "Any",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden resolver authority: {forbidden}"
        );
    }
}
