//! Version-coupled compiler references for private tooling queries.

use std::sync::OnceLock;

use pop_foundation::{BubbleId, FileId, ModuleId, NamespaceId};
use pop_source::SourceFile;

use crate::{FrontEndBubbleInput, FrontEndModule, ReferenceMetadata, analyze_bubble};

pub const TOOLING_INTERNAL_BUBBLE: BubbleId = BubbleId::from_raw(1);
pub const TOOLING_STANDARD_BUBBLE: BubbleId = BubbleId::from_raw(2);

static STANDARD_REFERENCE: OnceLock<ReferenceMetadata> = OnceLock::new();

/// Returns the compiler-verified public `Pop.Standard` reference used by
/// version-coupled editor queries.
///
/// The source is embedded into the tool binary so analysis never depends on a
/// checkout path, ambient Package cache, or CLI output. Repository validation
/// and architecture tests make malformed embedded Standard source a toolchain
/// incident rather than a user diagnostic.
///
/// # Panics
///
/// Panics only when the compiler-coupled embedded `Pop.Standard` sources exceed
/// typed ID bounds, fail their repository-validated analysis, or cannot publish
/// their required public reference metadata.
#[must_use]
pub fn tooling_standard_reference_metadata() -> &'static ReferenceMetadata {
    STANDARD_REFERENCE.get_or_init(|| {
        let modules = [
            (
                "Pop.Standard/src/lib.pop",
                include_str!("../../../libraries/standard/pop/src/lib.pop"),
            ),
            (
                "Pop.Standard/src/math.pop",
                include_str!("../../../libraries/standard/pop/src/math.pop"),
            ),
            (
                "Pop.Standard/src/bytes.pop",
                include_str!("../../../libraries/standard/pop/src/bytes.pop"),
            ),
            (
                "Pop.Standard/src/unicode.pop",
                include_str!("../../../libraries/standard/pop/src/unicode.pop"),
            ),
            (
                "Pop.Standard/src/text.pop",
                include_str!("../../../libraries/standard/pop/src/text.pop"),
            ),
            (
                "Pop.Standard/src/sequence.pop",
                include_str!("../../../libraries/standard/pop/src/sequence.pop"),
            ),
            (
                "Pop.Standard/src/random.pop",
                include_str!("../../../libraries/standard/pop/src/random.pop"),
            ),
            (
                "Pop.Standard/src/file.pop",
                include_str!("../../../libraries/standard/pop/src/file.pop"),
            ),
            (
                "Pop.Standard/src/process.pop",
                include_str!("../../../libraries/standard/pop/src/process.pop"),
            ),
            (
                "Pop.Standard/src/io.pop",
                include_str!("../../../libraries/standard/pop/src/io.pop"),
            ),
            (
                "Pop.Standard/src/csv.pop",
                include_str!("../../../libraries/standard/pop/src/csv.pop"),
            ),
            (
                "Pop.Standard/src/glob.pop",
                include_str!("../../../libraries/standard/pop/src/glob.pop"),
            ),
            (
                "Pop.Standard/src/guid.pop",
                include_str!("../../../libraries/standard/pop/src/guid.pop"),
            ),
            (
                "Pop.Standard/src/locale.pop",
                include_str!("../../../libraries/standard/pop/src/locale.pop"),
            ),
            (
                "Pop.Standard/src/mime.pop",
                include_str!("../../../libraries/standard/pop/src/mime.pop"),
            ),
            (
                "Pop.Standard/src/net.pop",
                include_str!("../../../libraries/standard/pop/src/net.pop"),
            ),
            (
                "Pop.Standard/src/netAddress.pop",
                include_str!("../../../libraries/standard/pop/src/netAddress.pop"),
            ),
            (
                "Pop.Standard/src/netDns.pop",
                include_str!("../../../libraries/standard/pop/src/netDns.pop"),
            ),
            (
                "Pop.Standard/src/netFacts.pop",
                include_str!("../../../libraries/standard/pop/src/netFacts.pop"),
            ),
            (
                "Pop.Standard/src/netFamilyValues.pop",
                include_str!("../../../libraries/standard/pop/src/netFamilyValues.pop"),
            ),
            (
                "Pop.Standard/src/netIpv4Endpoint.pop",
                include_str!("../../../libraries/standard/pop/src/netIpv4Endpoint.pop"),
            ),
            (
                "Pop.Standard/src/netIpv6.pop",
                include_str!("../../../libraries/standard/pop/src/netIpv6.pop"),
            ),
            (
                "Pop.Standard/src/netIpv6Endpoint.pop",
                include_str!("../../../libraries/standard/pop/src/netIpv6Endpoint.pop"),
            ),
            (
                "Pop.Standard/src/netScope.pop",
                include_str!("../../../libraries/standard/pop/src/netScope.pop"),
            ),
            (
                "Pop.Standard/src/path.pop",
                include_str!("../../../libraries/standard/pop/src/path.pop"),
            ),
            (
                "Pop.Standard/src/time.pop",
                include_str!("../../../libraries/standard/pop/src/time.pop"),
            ),
            (
                "Pop.Standard/src/timeClock.pop",
                include_str!("../../../libraries/standard/pop/src/timeClock.pop"),
            ),
            (
                "Pop.Standard/src/timeDate.pop",
                include_str!("../../../libraries/standard/pop/src/timeDate.pop"),
            ),
            (
                "Pop.Standard/src/timeDateTime.pop",
                include_str!("../../../libraries/standard/pop/src/timeDateTime.pop"),
            ),
            (
                "Pop.Standard/src/uri.pop",
                include_str!("../../../libraries/standard/pop/src/uri.pop"),
            ),
            (
                "Pop.Standard/src/version.pop",
                include_str!("../../../libraries/standard/pop/src/version.pop"),
            ),
            (
                "Pop.Standard/src/platform.pop",
                include_str!("../../../libraries/standard/pop/src/platform.pop"),
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (path, text))| {
            let raw = u32::try_from(index).expect("embedded Standard Module count is bounded");
            let file = u32::MAX
                .checked_sub(raw)
                .expect("embedded Standard FileId range is bounded");
            let source = SourceFile::new(FileId::from_raw(file), path, text)
                .expect("repository-validated embedded Pop.Standard source");
            FrontEndModule::new(ModuleId::from_raw(raw), source)
        })
        .collect();
        let result = analyze_bubble(FrontEndBubbleInput::new(
            TOOLING_STANDARD_BUBBLE,
            NamespaceId::from_raw(TOOLING_STANDARD_BUBBLE.raw()),
            vec![TOOLING_INTERNAL_BUBBLE],
            modules,
        ));
        assert!(
            result.diagnostics().is_empty(),
            "repository-validated embedded Pop.Standard must analyze without diagnostics: {}",
            result.diagnostic_snapshot()
        );
        result
            .reference_metadata()
            .expect("verified Pop.Standard publishes reference metadata")
            .clone()
    })
}
