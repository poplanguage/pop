#[test]
fn bounded_guid_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/guid.pop");

    for declaration in [
        "public record Value",
        "public function newVersion4(randomBytes: Bytes): Value?",
        "public function parse(text: String): Value?",
        "public function format(value: Value): String",
        "public function fromBytes(bytes: Bytes): Value?",
        "public function toBytes(value: Value): Bytes",
        "public function isNil(value: Value): Boolean",
        "public function isVersion4(value: Value): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Random.State",
        "Crypto.",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Guid must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
