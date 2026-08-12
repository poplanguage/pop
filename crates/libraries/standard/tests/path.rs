#[test]
fn bounded_portable_path_source_keeps_the_closed_lexical_contract() {
    let source = include_str!("../pop/src/path.pop");

    for declaration in [
        "public record Value",
        "public function normalize(text: String): Value?",
        "public function format(value: Value): String",
        "public function isAbsolute(value: Value): Boolean",
        "public function join(base: Value, child: String): Value?",
        "public function parent(value: Value): Value?",
        "public function name(value: Value): String?",
        "public function extension(value: Value): String?",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    assert!(source.contains("MAX_PATH_LENGTH = 4096"));
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "File.",
        "Directory.",
        "Environment.",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Path must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
