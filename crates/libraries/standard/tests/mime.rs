#[test]
fn bounded_media_type_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/mime.pop");

    for declaration in [
        "public record Parameter",
        "public record Value",
        "public function parse(text: String): Value?",
        "public function format(value: Value): String",
        "public function parameter(value: Value, name: String): String?",
        "public function matches(value: Value, pattern: String): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for bound in [
        "private const MAX_MEDIA_TYPE_LENGTH = 1024",
        "private const MAX_PARAMETER_COUNT = 32",
    ] {
        assert!(source.contains(bound), "missing parser bound {bound}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "sniff",
        "registry",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Mime must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
