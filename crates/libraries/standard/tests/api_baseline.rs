use std::fmt::Write as _;

use pop_standard::{
    ApiBaselineError, ApiKind, ApiStatus, parse_standard_api_baseline, standard_api_baseline,
};
use pop_types::embedded_bootstrap_schema;

#[test]
fn sequence_callbacks_are_invoked_once_per_loop_item() {
    let source = include_str!("../pop/src/sequence.pop");
    for (name, invocation) in [
        ("each", "action(value)"),
        ("findLastOr", "predicate(value)"),
        ("indexLastOr", "predicate(value)"),
        ("minByOr", "select(value)"),
        ("maxByOr", "select(value)"),
        ("sortBy", "select(value)"),
        ("containsBy", "equal(item, value)"),
    ] {
        let marker = format!("public function {name}<");
        let function = source
            .split_once(&marker)
            .map_or_else(
                || panic!("missing Sequence.{name}"),
                |(_, remainder)| remainder,
            )
            .split_once("\npublic function ")
            .map_or_else(|| source, |(body, _)| body);
        assert_eq!(
            function.matches(invocation).count(),
            1,
            "Sequence.{name} must call its closed pure predicate exactly once per loop item"
        );
    }
}

#[allow(clippy::too_many_lines)]
#[test]
fn frozen_standard_api_baseline_has_exact_prelude_and_prototype_boundaries() {
    let baseline = standard_api_baseline().expect("valid embedded API baseline");
    assert_eq!(baseline.schema_version(), 1);
    assert_eq!(baseline.entries().len(), 338);

    let prelude_names = baseline
        .entries()
        .iter()
        .filter(|entry| entry.prelude())
        .map(|entry| (entry.kind(), entry.name()))
        .collect::<Vec<_>>();
    assert!(prelude_names.contains(&(ApiKind::Namespace, "Sequence")));
    assert!(prelude_names.contains(&(ApiKind::Attribute, "RetainMetadata")));
    assert!(prelude_names.contains(&(ApiKind::Namespace, "Metadata")));
    assert!(prelude_names.contains(&(ApiKind::Namespace, "Codec")));
    assert!(prelude_names.contains(&(ApiKind::Primitive, "Rune")));
    assert!(!prelude_names.contains(&(ApiKind::Namespace, "Math")));
    assert!(!prelude_names.iter().any(|(_, name)| *name == "Option"));
    assert!(!prelude_names.iter().any(|(_, name)| *name == "Actor"));
    assert!(!prelude_names.iter().any(|(_, name)| *name == "Cluster"));

    let prototypes = baseline
        .entries()
        .iter()
        .filter(|entry| entry.status() == ApiStatus::Prototype)
        .map(|entry| entry.identity())
        .collect::<Vec<_>>();
    assert_eq!(prototypes.len(), 61);
    assert_eq!(
        &prototypes[..4],
        ["namespace:0", "namespace:1", "function:0", "function:1"]
    );
    assert_eq!(prototypes.last(), Some(&"api:84"));

    let portable_names = baseline
        .entries()
        .iter()
        .filter(|entry| entry.kind() == ApiKind::Api)
        .map(|entry| (entry.namespace(), entry.name()))
        .collect::<Vec<_>>();
    assert_eq!(
        portable_names,
        [
            ("Pop.Sequence", "map"),
            ("Pop.Sequence", "filter"),
            ("Pop.Sequence", "fold"),
            ("Pop.Sequence", "collect"),
            ("Pop.Sequence", "any"),
            ("Pop.Sequence", "all"),
            ("Pop.Sequence", "count"),
            ("Pop.Math", "min"),
            ("Pop.Math", "max"),
            ("Pop.Math", "abs"),
            ("Pop.Math", "gcd"),
            ("Pop.Sequence", "isEmpty"),
            ("Pop.Sequence", "firstOr"),
            ("Pop.Sequence", "lastOr"),
            ("Pop.Sequence", "each"),
            ("Pop.Sequence", "none"),
            ("Pop.Sequence", "countWhere"),
            ("Pop.Math", "sign"),
            ("Pop.Math", "lcm"),
            ("Pop.Math", "coprime"),
            ("Pop.Sequence", "take"),
            ("Pop.Sequence", "drop"),
            ("Pop.Sequence", "takeWhile"),
            ("Pop.Sequence", "dropWhile"),
            ("Pop.Sequence", "concat"),
            ("Pop.Sequence", "sum"),
            ("Pop.Sequence", "product"),
            ("Pop.Sequence", "minOr"),
            ("Pop.Sequence", "maxOr"),
            ("Pop.Sequence", "findOr"),
            ("Pop.Sequence", "indexOr"),
            ("Pop.Sequence", "sumBy"),
            ("Pop.Sequence", "productBy"),
            ("Pop.Sequence", "minByOr"),
            ("Pop.Sequence", "maxByOr"),
            ("Pop.Sequence", "append"),
            ("Pop.Sequence", "prepend"),
            ("Pop.Sequence", "scan"),
            ("Pop.Sequence", "elementAtOr"),
            ("Pop.Sequence", "findLastOr"),
            ("Pop.Sequence", "indexLastOr"),
            ("Pop.Sequence", "reduceOr"),
            ("Pop.Task", "cancellationSource"),
            ("Pop.Task", "cancelToken"),
            ("Pop.Task", "cancel"),
            ("Pop.Task", "group"),
            ("Pop.Task", "start"),
            ("Pop.Bytes", "view"),
            ("Pop.Bytes", "slice"),
            ("Pop.Bytes", "slice"),
            ("Pop.Bytes", "length"),
            ("Pop.Bytes", "get"),
            ("Pop.Bytes", "toBytes"),
            ("Pop.Text", "view"),
            ("Pop.Text", "slice"),
            ("Pop.Text", "slice"),
            ("Pop.Text", "length"),
            ("Pop.Text", "toString"),
            ("Pop.Math", "clamp"),
            ("Pop.Math", "power"),
            ("Pop.Math", "floorDivide"),
            ("Pop.Math", "floorRemainder"),
            ("Pop.Bytes", "equals"),
            ("Pop.Bytes", "compare"),
            ("Pop.Bytes", "startsWith"),
            ("Pop.Bytes", "endsWith"),
            ("Pop.Bytes", "contains"),
            ("Pop.Bytes", "indexOf"),
            ("Pop.Bytes", "readUInt16BigEndian"),
            ("Pop.Bytes", "readUInt16LittleEndian"),
            ("Pop.Bytes", "readUInt32BigEndian"),
            ("Pop.Bytes", "readUInt32LittleEndian"),
            ("Pop.Bytes", "readUInt64BigEndian"),
            ("Pop.Bytes", "readUInt64LittleEndian"),
            ("Pop.Unicode", "fromCodePoint"),
            ("Pop.Unicode", "codePoint"),
            ("Pop.Text", "get"),
            ("Pop.Text", "get"),
            ("Pop.Unicode", "isAscii"),
            ("Pop.Unicode", "isAsciiLetter"),
            ("Pop.Unicode", "isAsciiDigit"),
            ("Pop.Unicode", "isAsciiAlphanumeric"),
            ("Pop.Unicode", "isAsciiWhitespace"),
            ("Pop.Unicode", "toAsciiLower"),
            ("Pop.Unicode", "toAsciiUpper"),
            ("Pop.Bytes", "create"),
            ("Pop.Bytes", "withCapacity"),
            ("Pop.Bytes", "length"),
            ("Pop.Bytes", "reserve"),
            ("Pop.Bytes", "clear"),
            ("Pop.Bytes", "write"),
            ("Pop.Bytes", "write"),
            ("Pop.Bytes", "write"),
            ("Pop.Bytes", "writeUInt16BigEndian"),
            ("Pop.Bytes", "writeUInt16LittleEndian"),
            ("Pop.Bytes", "writeUInt32BigEndian"),
            ("Pop.Bytes", "writeUInt32LittleEndian"),
            ("Pop.Bytes", "writeUInt64BigEndian"),
            ("Pop.Bytes", "writeUInt64LittleEndian"),
            ("Pop.Bytes", "toBytes"),
            ("Pop.Text", "encodeUtf8"),
            ("Pop.Text", "encodeUtf8"),
            ("Pop.Text", "decodeUtf8"),
            ("Pop.Text", "decodeUtf8"),
            ("Pop.Bytes", "hexEncode"),
            ("Pop.Bytes", "hexDecode"),
            ("Pop.Bytes", "base64Encode"),
            ("Pop.Bytes", "base64Decode"),
            ("Pop.Bytes", "base32Encode"),
            ("Pop.Bytes", "base32Decode"),
            ("Pop.Bytes", "bitwiseAnd"),
            ("Pop.Bytes", "bitwiseOr"),
            ("Pop.Bytes", "bitwiseXor"),
            ("Pop.Bytes", "bitwiseNot"),
            ("Pop.Unicode", "isWhitespace"),
            ("Pop.Text", "trimStart"),
            ("Pop.Text", "trimEnd"),
            ("Pop.Text", "trim"),
            ("Pop.Text", "replace"),
            ("Pop.Text", "split"),
            ("Pop.Text", "join"),
            ("Pop.Text", "parseInt"),
            ("Pop.Text", "startsWith"),
            ("Pop.Text", "endsWith"),
            ("Pop.Text", "contains"),
            ("Pop.Text", "indexOf"),
            ("Pop.Text", "toAsciiLower"),
            ("Pop.Text", "toAsciiUpper"),
            ("Pop.Text", "equalsAsciiIgnoreCase"),
            ("Pop.Random", "State"),
            ("Pop.Random", "seed"),
            ("Pop.Random", "next"),
            ("Pop.Random", "fill"),
            ("Pop.Random", "shuffle"),
            ("Pop.Random", "nextInt"),
            ("Pop.Random", "nextFloat"),
            ("Pop.Random", "chance"),
            ("Pop.Sequence", "reverse"),
            ("Pop.Sequence", "sort"),
            ("Pop.Sequence", "sortBy"),
            ("Pop.Sequence", "containsBy"),
            ("Pop.Sequence", "equalsBy"),
            ("Pop.Sequence", "first"),
            ("Pop.Sequence", "last"),
            ("Pop.Version", "Value"),
            ("Pop.Version", "parse"),
            ("Pop.Version", "format"),
            ("Pop.Version", "compare"),
            ("Pop.Version", "matches"),
            ("Pop.Mime", "Parameter"),
            ("Pop.Mime", "Value"),
            ("Pop.Mime", "parse"),
            ("Pop.Mime", "format"),
            ("Pop.Mime", "parameter"),
            ("Pop.Mime", "matches"),
            ("Pop.Uri", "Value"),
            ("Pop.Uri", "parse"),
            ("Pop.Uri", "format"),
            ("Pop.Uri", "percentEncode"),
            ("Pop.Uri", "percentDecode"),
            ("Pop.Uri", "resolve"),
            ("Pop.Guid", "Value"),
            ("Pop.Guid", "newVersion4"),
            ("Pop.Guid", "parse"),
            ("Pop.Guid", "format"),
            ("Pop.Guid", "fromBytes"),
            ("Pop.Guid", "toBytes"),
            ("Pop.Guid", "isNil"),
            ("Pop.Guid", "isVersion4"),
            ("Pop.Path", "Value"),
            ("Pop.Path", "normalize"),
            ("Pop.Path", "format"),
            ("Pop.Path", "isAbsolute"),
            ("Pop.Path", "join"),
            ("Pop.Path", "parent"),
            ("Pop.Path", "name"),
            ("Pop.Path", "extension"),
            ("Pop.Time", "Duration"),
            ("Pop.Time", "fromSeconds"),
            ("Pop.Time", "fromMilliseconds"),
            ("Pop.Time", "fromNanoseconds"),
            ("Pop.Time", "compare"),
            ("Pop.Time", "isZero"),
            ("Pop.Time", "isNegative"),
            ("Pop.Time", "secondsPart"),
            ("Pop.Time", "nanosecondsPart"),
            ("Pop.Time", "Instant"),
            ("Pop.Time", "Deadline"),
            ("Pop.Time", "TestClock"),
            ("Pop.Time", "instant"),
            ("Pop.Time", "testClock"),
            ("Pop.Time", "now"),
            ("Pop.Time", "advance"),
            ("Pop.Time", "deadlineAfter"),
            ("Pop.Time", "isExpired"),
            ("Pop.Time", "Date"),
            ("Pop.Time", "date"),
            ("Pop.Time", "isLeapYear"),
            ("Pop.Time", "daysInMonth"),
            ("Pop.Time", "compareDates"),
            ("Pop.Time", "TimeOfDay"),
            ("Pop.Time", "LocalDateTime"),
            ("Pop.Time", "UtcOffset"),
            ("Pop.Time", "OffsetDateTime"),
            ("Pop.Time", "timeOfDay"),
            ("Pop.Time", "localDateTime"),
            ("Pop.Time", "utcOffset"),
            ("Pop.Time", "offsetDateTime"),
            ("Pop.Time", "isUtc"),
            ("Pop.Locale", "Tag"),
            ("Pop.Locale", "parse"),
            ("Pop.Locale", "format"),
            ("Pop.Locale", "sameLanguage"),
            ("Pop.Glob", "Pattern"),
            ("Pop.Glob", "compile"),
            ("Pop.Glob", "matches"),
            ("Pop.Csv", "parse"),
            ("Pop.Csv", "format"),
            ("Pop.Net", "Ipv4Address"),
            ("Pop.Net", "ipv4"),
            ("Pop.Net", "parseIpv4"),
            ("Pop.Net", "formatIpv4"),
            ("Pop.Net", "ipv4Octet"),
            ("Pop.Net", "isIpv4Loopback"),
            ("Pop.Net", "isIpv4Private"),
            ("Pop.Net", "Ipv4Prefix"),
            ("Pop.Net", "Ipv4SocketAddress"),
            ("Pop.Net", "ipv4Prefix"),
            ("Pop.Net", "networkIpv4"),
            ("Pop.Net", "containsIpv4"),
            ("Pop.Net", "ipv4Socket"),
            ("Pop.Net", "parseIpv4Socket"),
            ("Pop.Net", "formatIpv4Socket"),
            ("Pop.Channel", "bounded"),
            ("Pop.Channel", "trySend"),
            ("Pop.Channel", "tryReceive"),
            ("Pop.Channel", "close"),
            ("Pop.Channel", "closeReceiver"),
            ("Pop.Channel", "sendAccepted"),
            ("Pop.Channel", "sendFull"),
            ("Pop.Channel", "sendClosed"),
            ("Pop.Channel", "received"),
            ("Pop.Channel", "receiveEmpty"),
            ("Pop.Channel", "receiveClosed"),
            ("Pop.Net", "Ipv6Address"),
            ("Pop.Net", "ipv6"),
            ("Pop.Net", "parseIpv6"),
            ("Pop.Net", "formatIpv6"),
            ("Pop.Net", "ipv6Segment"),
            ("Pop.Net", "isIpv6Loopback"),
            ("Pop.Net", "isIpv6Unspecified"),
            ("Pop.Net", "Ipv6Prefix"),
            ("Pop.Net", "Ipv6SocketAddress"),
            ("Pop.Net", "ipv6Prefix"),
            ("Pop.Net", "networkIpv6"),
            ("Pop.Net", "containsIpv6"),
            ("Pop.Net", "ipv6Socket"),
            ("Pop.Net", "parseIpv6Socket"),
            ("Pop.Net", "formatIpv6Socket"),
            ("Pop.Net", "Address"),
            ("Pop.Net", "parseAddress"),
            ("Pop.Net", "formatAddress"),
            ("Pop.Net", "isAddressLoopback"),
            ("Pop.Net", "isAddressUnspecified"),
            ("Pop.Net", "Prefix"),
            ("Pop.Net", "SocketAddress"),
            ("Pop.Net", "networkAddress"),
            ("Pop.Net", "containsAddress"),
            ("Pop.Net", "parseSocketAddress"),
            ("Pop.Net", "formatSocketAddress"),
            ("Pop.Net", "InterfaceId"),
            ("Pop.Net", "ScopedIpv6Address"),
            ("Pop.Net", "interfaceId"),
            ("Pop.Net", "scopedIpv6"),
            ("Pop.Net", "parseScopedIpv6"),
            ("Pop.Net", "formatScopedIpv6"),
        ]
    );
}

#[test]
fn standard_api_baseline_agrees_with_trusted_bootstrap_identities() {
    let baseline = standard_api_baseline().expect("valid embedded API baseline");
    let bootstrap = embedded_bootstrap_schema().expect("valid bootstrap metadata");

    for entry in baseline.entries() {
        let (_, raw_id) = entry.identity().split_once(':').expect("baseline identity");
        match entry.kind() {
            ApiKind::Primitive => assert!(
                bootstrap
                    .primitives()
                    .iter()
                    .any(|primitive| primitive.source_name() == entry.name())
            ),
            ApiKind::Type => {
                let source_name = entry.namespace().strip_prefix("Pop.").map_or_else(
                    || entry.name().to_owned(),
                    |namespace| format!("{namespace}.{}", entry.name()),
                );
                let metadata = bootstrap
                    .type_by_source_name(&source_name)
                    .unwrap_or_else(|| panic!("missing bootstrap type {source_name}"));
                assert_eq!(metadata.id().raw().to_string(), raw_id);
                assert_eq!(metadata.owner_bubble(), entry.owner_bubble());
                assert_eq!(metadata.is_in_prelude(), entry.prelude());
            }
            ApiKind::Attribute => {
                let metadata = bootstrap
                    .compiler_attributes()
                    .iter()
                    .find(|attribute| attribute.source_name() == entry.name())
                    .unwrap_or_else(|| panic!("missing bootstrap attribute {}", entry.name()));
                assert_eq!(metadata.id().raw().to_string(), raw_id);
                assert_eq!(metadata.owner_bubble(), entry.owner_bubble());
                assert_eq!(metadata.is_in_prelude(), entry.prelude());
            }
            ApiKind::Function if entry.namespace() == "Pop" => {
                let metadata = bootstrap
                    .standard_functions()
                    .iter()
                    .find(|function| function.id().raw().to_string() == raw_id)
                    .unwrap_or_else(|| panic!("missing bootstrap function {}", entry.identity()));
                assert_eq!(metadata.source_name(), entry.name());
                assert_eq!(metadata.owner_bubble(), entry.owner_bubble());
                assert_eq!(metadata.is_in_prelude(), entry.prelude());
            }
            ApiKind::Namespace | ApiKind::Api | ApiKind::Function => {}
        }
    }
}

#[test]
fn standard_api_baseline_rejects_noncanonical_or_unsupported_metadata() {
    let header = "schemaVersion\t1\nidentity\tkind\townerBubble\tnamespace\tname\tsignature\ttier\tstatus\tprelude\tdocumentation\n";
    let valid = "primitive:0\tPrimitive\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md\n";

    assert!(parse_standard_api_baseline(&(header.to_owned() + valid)).is_ok());
    for invalid in [
        header.to_owned() + valid + valid,
        header.to_owned()
            + "primitive:0\tUnknown\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md\n",
        header.to_owned()
            + "primitive:0\tPrimitive\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\tplanned\ttrue\tarchitecture/02-language-model.md\n",
        header.to_owned()
            + "primitive:1\tPrimitive\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md\n"
            + valid,
    ] {
        assert_eq!(
            parse_standard_api_baseline(&invalid),
            Err(ApiBaselineError::InvalidEntry)
        );
    }
}

#[test]
fn standard_api_baseline_rejects_noncanonical_identity_namespace_and_tier_fields() {
    let header = "schemaVersion\t1\nidentity\tkind\townerBubble\tnamespace\tname\tsignature\ttier\tstatus\tprelude\tdocumentation\n";
    for invalid_entry in [
        "primitive:00\tPrimitive\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md\n",
        "primitive:0\tPrimitive\tPop.Internal\tPopcorn\tBoolean\tBoolean\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md\n",
        "primitive:0\tPrimitive\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\timplemented\tfalse\tarchitecture/02-language-model.md\n",
        "primitive:0\tPrimitive\tPop.Internal\tPop\tBoolean\tBoolean\tprelude\timplemented\ttrue\tarchitecture/../ROADMAP.md\n",
    ] {
        assert_eq!(
            parse_standard_api_baseline(&(header.to_owned() + invalid_entry)),
            Err(ApiBaselineError::InvalidEntry)
        );
    }
}

#[test]
fn standard_api_baseline_loading_is_bounded() {
    let header = "schemaVersion\t1\nidentity\tkind\townerBubble\tnamespace\tname\tsignature\ttier\tstatus\tprelude\tdocumentation\n";
    let oversized_entry = format!(
        "primitive:0\tPrimitive\tPop.Internal\tPop\t{}\tBoolean\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md\n",
        "A".repeat(5_000)
    );
    assert_eq!(
        parse_standard_api_baseline(&(header.to_owned() + &oversized_entry)),
        Err(ApiBaselineError::InvalidEntry)
    );

    let mut oversized_inventory = header.to_owned();
    for identity in 0..1_025 {
        let _ = writeln!(
            oversized_inventory,
            "primitive:{identity}\tPrimitive\tPop.Internal\tPop\tBoolean{identity}\tBoolean{identity}\tprelude\timplemented\ttrue\tarchitecture/02-language-model.md"
        );
    }
    assert_eq!(
        parse_standard_api_baseline(&oversized_inventory),
        Err(ApiBaselineError::InvalidEntry)
    );

    let oversized_file = format!("{header}{}", "A".repeat(300_000));
    assert_eq!(
        parse_standard_api_baseline(&oversized_file),
        Err(ApiBaselineError::InvalidEntry)
    );
}
