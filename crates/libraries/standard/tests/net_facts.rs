#[test]
fn interface_and_route_facts_are_immutable_static_values() {
    let source = include_str!("../pop/src/netFacts.pop");
    for declaration in [
        "public record NetworkInterface",
        "public record InterfaceAddress",
        "public union Route",
        "public function networkInterface(",
        "public function interfaceAddress(identity: InterfaceId, prefix: Prefix): InterfaceAddress",
        "public function ipv4OnLinkRoute(",
        "public function ipv4ViaRoute(",
        "public function ipv6OnLinkRoute(",
        "public function ipv6ViaRoute(",
        "public function routeDestination(route: Route): Prefix",
        "public function routeNextHop(route: Route): Address?",
        "public function routeInterfaceId(route: Route): InterfaceId",
        "public function routeMetric(route: Route): UInt32",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Dynamic",
        "Any",
        "Table",
        "interfaces(",
        "routes(",
        "Dns.",
        "Socket.",
        "runtime",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden fact behavior: {forbidden}"
        );
    }
}
