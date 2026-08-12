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
fn portable_bytes_inspection_has_checked_documentation_and_no_hidden_materialization() {
    let path = repository_root().join("crates/libraries/standard/pop/src/bytes.pop");
    let source = fs::read_to_string(&path).expect("read Pop.Bytes source");
    let functions = [
        "hexEncode",
        "hexDecode",
        "base64Encode",
        "base64Decode",
        "base32Encode",
        "base32Decode",
        "bitwiseAnd",
        "bitwiseOr",
        "bitwiseXor",
        "bitwiseNot",
        "equals",
        "compare",
        "startsWith",
        "endsWith",
        "contains",
        "indexOf",
        "readUInt16BigEndian",
        "readUInt16LittleEndian",
        "readUInt32BigEndian",
        "readUInt32LittleEndian",
        "readUInt64BigEndian",
        "readUInt64LittleEndian",
    ];
    let inspection_functions = [
        "equals",
        "compare",
        "startsWith",
        "endsWith",
        "contains",
        "indexOf",
        "readUInt16BigEndian",
        "readUInt16LittleEndian",
        "readUInt32BigEndian",
        "readUInt32LittleEndian",
        "readUInt64BigEndian",
        "readUInt64LittleEndian",
    ];

    assert_eq!(
        source
            .lines()
            .filter(|line| line.starts_with("public function "))
            .count(),
        functions.len(),
        "the Bytes Module must contain exactly the accepted ADR 0113 and 0119-0122 surface"
    );
    for name in functions {
        let marker = format!("public function {name}(");
        let (before, _) = source
            .split_once(&marker)
            .unwrap_or_else(|| panic!("missing Pop.Bytes.{name}"));
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
                "Pop.Bytes.{name} documentation lacks {required}"
            );
        }
    }
    for name in inspection_functions {
        let marker = format!("public function {name}(");
        let (_, remainder) = source
            .split_once(&marker)
            .unwrap_or_else(|| panic!("missing Pop.Bytes.{name}"));
        let body = remainder
            .split_once("\npublic function ")
            .map_or(remainder, |(body, _)| body);
        assert!(
            !body.contains("Bytes.toBytes("),
            "Pop.Bytes.{name} inspection must not materialize owned Bytes"
        );
    }
}
