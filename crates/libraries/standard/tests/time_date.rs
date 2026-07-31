#[test]
fn bounded_gregorian_date_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/timeDate.pop");

    for declaration in [
        "public record Date",
        "public function date(year: Int, month: Int, day: Int): Date?",
        "public function isLeapYear(year: Int): Boolean",
        "public function daysInMonth(year: Int, month: Int): Int?",
        "public function compareDates(left: Date, right: Date): Int",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Clock",
        "Zone",
        "Locale",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Time.Date must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
