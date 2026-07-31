#[test]
fn bounded_csv_source_keeps_typed_rows_and_closed_limits() {
    let source = include_str!("../pop/src/csv.pop");

    for declaration in [
        "public function parse(text: String): List<List<String>>?",
        "public function format(rows: List<List<String>>): String?",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "reflect",
        "File.",
        "Io.",
        "runtime",
    ] {
        assert!(
            !source.contains(forbidden),
            "Csv must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
