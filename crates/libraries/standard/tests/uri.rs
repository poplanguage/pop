#[test]
fn bounded_uri_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/uri.pop");

    for declaration in [
        "public record Value",
        "public function parse(text: String): Value?",
        "public function format(value: Value): String",
        "public function percentEncode(value: String): String?",
        "public function percentDecode(value: String): String?",
        "public function resolve(base: Value, reference: Value): Value",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    assert!(source.contains("private const MAX_URI_LENGTH = 4096"));
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Dns.",
        "Http.",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Uri must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
