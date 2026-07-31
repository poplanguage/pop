#[test]
fn deterministic_test_clock_source_keeps_the_closed_portable_contract() {
    let source = include_str!("../pop/src/timeClock.pop");

    for declaration in [
        "public record Instant",
        "public record Deadline",
        "public class TestClock",
        "public function instant(seconds: Int, nanoseconds: Int): Instant?",
        "public function testClock(start: Instant): TestClock?",
        "public function now(clock: TestClock): Instant",
        "public function advance(clock: TestClock, duration: Duration): Boolean",
        "public function deadlineAfter(clock: TestClock, duration: Duration): Deadline?",
        "public function isExpired(clock: TestClock, deadline: Deadline): Boolean",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
    for forbidden in [
        "Pop.Internal",
        "Dynamic",
        "Any",
        "eval(",
        "load(",
        "sleep(",
        "timer(",
        "wallClock",
        "runtime",
        "reflect",
    ] {
        assert!(
            !source.contains(forbidden),
            "Time.TestClock must remain ordinary portable static Pop: {forbidden}"
        );
    }
}
