#[test]
fn bounded_semantic_version_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/version.pop");

    for declaration in [
        "public record Value",
        "public function parse(text: String): Value?",
        "public function format(value: Value): String",
        "public function compare(left: Value, right: Value): Int",
        "public function matches(value: Value, requirement: String): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for bound in [
        "private const MAX_VERSION_TEXT_LENGTH = 1024",
        "private const MAX_VERSION_COMPONENT = 2147483646",
    ] {
        assert!(source.contains(bound), "missing parser bound {bound}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Version must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
