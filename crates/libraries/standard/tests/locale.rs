#[test]
fn bounded_locale_tag_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/locale.pop");

    for declaration in [
        "public record Tag",
        "public function parse(text: String): Tag?",
        "public function format(tag: Tag): String?",
        "public function sameLanguage(left: Tag, right: Tag): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "currentLocale",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Locale.Tag must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
