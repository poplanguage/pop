use std::fs;
use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("standard crate is below the repository root")
        .to_owned()
}

#[test]
fn portable_ascii_rune_helpers_have_checked_allocation_free_documentation() {
    let path = repository_root().join("crates/libraries/standard/pop/src/unicode.pop");
    let source = fs::read_to_string(&path).expect("read Pop.Unicode source");
    let functions = [
        "isAscii",
        "isAsciiLetter",
        "isAsciiDigit",
        "isAsciiAlphanumeric",
        "isAsciiWhitespace",
        "toAsciiLower",
        "toAsciiUpper",
    ];

    assert_eq!(
        source
            .lines()
            .filter(|line| line.starts_with("public function "))
            .count(),
        functions.len(),
        "the focused Unicode Module must contain only the ADR 0114 ASCII surface"
    );
    for name in functions {
        let marker = format!("public function {name}(");
        let (before, _) = source
            .split_once(&marker)
            .unwrap_or_else(|| panic!("missing Pop.Unicode.{name}"));
        let documentation = before
            .rsplit_once("public function ")
            .map_or(before, |(_, block)| block);
        for required in [
            "--- <summary>",
            "--- <returns>",
            "--- <complexity",
            "--- <allocation>",
        ] {
            assert!(
                documentation.contains(required),
                "Pop.Unicode.{name} documentation lacks {required}"
            );
        }
    }
}
