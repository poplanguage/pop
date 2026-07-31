#[test]
fn bounded_civil_time_source_keeps_distinct_static_values() {
    let source = include_str!("../pop/src/timeDateTime.pop");

    for declaration in [
        "public record TimeOfDay",
        "public record LocalDateTime",
        "public record UtcOffset",
        "public record OffsetDateTime",
        "public function timeOfDay(hour: Int, minute: Int, second: Int, nanosecond: Int): TimeOfDay?",
        "public function localDateTime(date: Date, time: TimeOfDay): LocalDateTime?",
        "public function utcOffset(seconds: Int): UtcOffset?",
        "public function offsetDateTime(dateTime: LocalDateTime, offset: UtcOffset): OffsetDateTime?",
        "public function isUtc(offset: UtcOffset): Boolean",
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
            "civil Time values must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
