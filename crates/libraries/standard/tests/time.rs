#[test]
fn canonical_duration_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/time.pop");

    for declaration in [
        "public record Duration",
        "public function fromSeconds(seconds: Int): Duration",
        "public function fromMilliseconds(milliseconds: Int): Duration",
        "public function fromNanoseconds(nanoseconds: Int): Duration",
        "public function compare(left: Duration, right: Duration): Int",
        "public function isZero(duration: Duration): Boolean",
        "public function isNegative(duration: Duration): Boolean",
        "public function secondsPart(duration: Duration): Int",
        "public function nanosecondsPart(duration: Duration): Int",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "Clock.",
        "sleep(",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Time.Duration must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
