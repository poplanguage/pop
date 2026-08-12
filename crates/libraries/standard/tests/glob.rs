#[test]
fn bounded_text_glob_source_keeps_the_closed_static_contract() {
    let source = include_str!("../pop/src/glob.pop");

    for declaration in [
        "public class Pattern",
        "public function compile(text: String): Pattern?",
        "public function matches(pattern: Pattern, text: String): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Regex",
        "Directory",
        "shell",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Glob must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
