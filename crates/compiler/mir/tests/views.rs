use pop_driver::{FrontEndBubbleInput, FrontEndModule, analyze_bubble};
use pop_foundation::{
    AllocationSiteId, BubbleId, BuiltinTypeId, FileId, LifetimeId, ModuleId, NamespaceId,
};
use pop_mir::{
    MirVerificationError, MirViewBoundaryProof, MirViewKind, MirViewLender, MirViewRangeUnit,
    MirViewTrap, lower_hir_bubble, optimize_mir, parse_mir_dump, verify_mir_bubble,
};
use pop_source::SourceFile;
use pop_types::{BYTES_VIEW_TYPE_ID, SemanticType, TEXT_VIEW_TYPE_ID, TypeArena};

fn view_types() -> TypeArena {
    let mut types = TypeArena::new();
    let integer = types.source_type("Int").expect("Int");
    let byte = types.source_type("Byte").expect("Byte");
    let rune = types.source_type("Rune").expect("Rune");
    let string = types.source_type("String").expect("String");
    let bytes = types
        .intern(SemanticType::Builtin {
            definition: BuiltinTypeId::from_raw(0),
            arguments: Vec::new(),
        })
        .expect("Bytes");
    let bytes_view = types
        .intern(SemanticType::Builtin {
            definition: BYTES_VIEW_TYPE_ID,
            arguments: Vec::new(),
        })
        .expect("Bytes.View");
    let text_view = types
        .intern(SemanticType::Builtin {
            definition: TEXT_VIEW_TYPE_ID,
            arguments: Vec::new(),
        })
        .expect("Text.View");
    let _ = types.optional(byte).expect("Byte?");
    let _ = types.optional(rune).expect("Rune?");
    let _ = (integer, string, bytes, bytes_view, text_view);
    types
}

fn rune_operations_text(types: &TypeArena) -> String {
    let integer = types.source_type("Int").expect("Int");
    let rune = types.source_type("Rune").expect("Rune");
    let uint32 = types.source_type("UInt32").expect("UInt32");
    let string = types.source_type("String").expect("String");
    let nil = types.source_type("nil").expect("nil");
    let text_view = types
        .find(&SemanticType::Builtin {
            definition: TEXT_VIEW_TYPE_ID,
            arguments: Vec::new(),
        })
        .expect("Text.View");
    let optional_rune = types
        .find(&SemanticType::Union(vec![nil, rune]))
        .expect("Rune?");
    format!(
        concat!(
            "mir bubble b0 namespace n0\n",
            "dependencies\n",
            "function s0 f0(t{uint32}) -> (t{optional_rune})\n",
            "  b0(v0:t{uint32}):\n",
            "    v1:t{optional_rune} = runeFromCodePoint v0\n",
            "    return (v1)\n",
            "function s1 f1(t{rune}) -> (t{uint32})\n",
            "  b0(v0:t{rune}):\n",
            "    v1:t{uint32} = runeCodePoint v0\n",
            "    return (v1)\n",
            "function s2 f2(t{string}, t{integer}) -> (t{optional_rune})\n",
            "  b0(v0:t{string}, v1:t{integer}):\n",
            "    v2:t{text_view} = viewCreate text v0 lender parameter#0 unit scalars boundary utf8 lifetime#1\n",
            "    v3:t{optional_rune} = viewGetRune v2 v1\n",
            "    do v4 viewEnd lifetime#1\n",
            "    return (v3)\n",
        ),
        integer = integer.raw(),
        rune = rune.raw(),
        uint32 = uint32.raw(),
        string = string.raw(),
        text_view = text_view.raw(),
        optional_rune = optional_rune.raw(),
    )
}

fn bytes_view_text(types: &TypeArena) -> String {
    let integer = types.source_type("Int").expect("Int");
    let byte = types.source_type("Byte").expect("Byte");
    let nil = types.source_type("nil").expect("nil");
    let bytes = types
        .find(&SemanticType::Builtin {
            definition: BuiltinTypeId::from_raw(0),
            arguments: Vec::new(),
        })
        .expect("Bytes");
    let view = types
        .find(&SemanticType::Builtin {
            definition: BYTES_VIEW_TYPE_ID,
            arguments: Vec::new(),
        })
        .expect("Bytes.View");
    let optional_byte = types
        .find(&SemanticType::Union(vec![nil, byte]))
        .expect("Byte?");
    format!(
        concat!(
            "mir bubble b0 namespace n0\n",
            "dependencies\n",
            "function s0 f0(t{bytes}, t{integer}, t{integer}) -> (t{bytes}, t{integer}, t{optional_byte}) effects[Allocates,MayTrap,GcSafePoint,Roots]\n",
            "  b0(v0:t{bytes}, v1:t{integer}, v2:t{integer}):\n",
            "    v3:t{view} = viewCreate bytes v0 lender parameter#0 unit bytes boundary none lifetime#1\n",
            "    v4:t{view} = viewSlice bytes v3 v1 v2 lender parameter#0 unit bytes boundary none parent lifetime#1 lifetime#2 trap BoundsViolation\n",
            "    v5:t{integer} = viewLength bytes v4\n",
            "    v6:t{optional_byte} = viewGetByte v4 v1\n",
            "    do v7 gcSafePoint sp0 roots (v0)\n",
            "    v8:t{bytes} = viewMaterialize bytes v4 allocation#7\n",
            "    do v9 viewEnd lifetime#2\n",
            "    do v10 viewEnd lifetime#1\n",
            "    return (v8,v5,v6)\n",
        ),
        bytes = bytes.raw(),
        integer = integer.raw(),
        optional_byte = optional_byte.raw(),
        view = view.raw(),
    )
}

#[test]
fn canonical_view_operations_round_trip_and_survive_optimization() {
    let types = view_types();
    let text = bytes_view_text(&types);
    let bubble = parse_mir_dump(&text).expect("view MIR");
    assert_eq!(verify_mir_bubble(&bubble, &types), Ok(()));

    let dump = bubble.dump();
    assert!(dump.contains("viewCreate bytes v0 lender parameter#0"));
    assert!(dump.contains("parent lifetime#1 lifetime#2 trap BoundsViolation"));
    assert!(dump.contains("viewMaterialize bytes v4 allocation#7"));
    assert_eq!(parse_mir_dump(&dump).expect("view round trip"), bubble);

    let optimized = optimize_mir(bubble, &types).expect("verified optimized view MIR");
    assert_eq!(verify_mir_bubble(&optimized, &types), Ok(()));
    let optimized_dump = optimized.dump();
    assert!(optimized_dump.contains("lifetime#1"));
    assert!(optimized_dump.contains("lifetime#2"));
    assert!(optimized_dump.contains("allocation#7"));
}

#[test]
fn verifier_rejects_corrupt_view_contracts_and_escape() {
    let types = view_types();
    let text = bytes_view_text(&types);
    let corruptions = [
        text.replace("viewCreate bytes", "viewCreate text"),
        text.replace("unit bytes", "unit scalars"),
        text.replace("boundary none", "boundary utf8"),
        text.replace("parent lifetime#1", "parent lifetime#9"),
        text.replace("lifetime#2 trap", "lifetime#1 trap"),
        text.replace("    do v9 viewEnd lifetime#2\n", ""),
        text.replace("roots (v0)", "roots ()"),
        text.replace("return (v8,v5,v6)", "return (v4,v5,v6)"),
    ];
    for corrupt in corruptions {
        let bubble = parse_mir_dump(&corrupt).expect("structurally valid corrupt view MIR");
        assert!(
            verify_mir_bubble(&bubble, &types).is_err(),
            "corrupt view MIR was accepted:\n{corrupt}"
        );
    }
    assert!(
        parse_mir_dump(&text.replace(" trap BoundsViolation", " trap none")).is_err(),
        "the closed view trap vocabulary accepted an invented fallback"
    );
}

#[test]
fn rune_operations_round_trip_and_reject_type_drift() {
    let types = view_types();
    let valid = rune_operations_text(&types);
    let bubble = parse_mir_dump(&valid).expect("Rune operation MIR");
    assert_eq!(verify_mir_bubble(&bubble, &types), Ok(()));
    assert_eq!(
        parse_mir_dump(&bubble.dump()).expect("Rune round trip"),
        bubble
    );

    let rune = types.source_type("Rune").expect("Rune");
    let uint32 = types.source_type("UInt32").expect("UInt32");
    let byte = types.source_type("Byte").expect("Byte");
    let nil = types.source_type("nil").expect("nil");
    let optional_rune = types
        .find(&SemanticType::Union(vec![nil, rune]))
        .expect("Rune?");
    let optional_byte = types
        .find(&SemanticType::Union(vec![nil, byte]))
        .expect("Byte?");
    let wrong_constructor_operand = valid
        .replacen(
            &format!("function s0 f0(t{})", uint32.raw()),
            &format!("function s0 f0(t{})", rune.raw()),
            1,
        )
        .replacen(
            &format!("b0(v0:t{}):", uint32.raw()),
            &format!("b0(v0:t{}):", rune.raw()),
            1,
        );
    let wrong_projection_operand = valid
        .replacen(
            &format!("function s1 f1(t{})", rune.raw()),
            &format!("function s1 f1(t{})", uint32.raw()),
            1,
        )
        .replacen(
            &format!(
                "b0(v0:t{}):\n    v1:t{} = runeCodePoint",
                rune.raw(),
                uint32.raw()
            ),
            &format!(
                "b0(v0:t{}):\n    v1:t{} = runeCodePoint",
                uint32.raw(),
                uint32.raw()
            ),
            1,
        );
    let corruptions = [
        wrong_constructor_operand,
        valid.replacen(
            &format!("v1:t{} = runeFromCodePoint", optional_rune.raw()),
            &format!("v1:t{} = runeFromCodePoint", optional_byte.raw()),
            1,
        ),
        wrong_projection_operand,
        valid.replacen(
            &format!("v1:t{} = runeCodePoint", uint32.raw()),
            &format!("v1:t{} = runeCodePoint", rune.raw()),
            1,
        ),
        valid.replacen(
            &format!("v3:t{} = viewGetRune", optional_rune.raw()),
            &format!("v3:t{} = viewGetRune", optional_byte.raw()),
            1,
        ),
    ];
    for corrupt in corruptions {
        let bubble = parse_mir_dump(&corrupt).expect("structurally valid corrupt Rune MIR");
        assert!(
            verify_mir_bubble(&bubble, &types).is_err(),
            "corrupt Rune MIR was accepted:\n{corrupt}"
        );
    }
}

#[test]
fn parameter_lender_provenance_survives_exact_block_argument_joins() {
    let types = view_types();
    let string = types.source_type("String").expect("String");
    let boolean = types.source_type("Boolean").expect("Boolean");
    let integer = types.source_type("Int").expect("Int");
    let view = types
        .find(&SemanticType::Builtin {
            definition: TEXT_VIEW_TYPE_ID,
            arguments: Vec::new(),
        })
        .expect("Text.View");
    let valid = format!(
        concat!(
            "mir bubble b0 namespace n0\n",
            "dependencies\n",
            "function s0 f0(t{string}, t{boolean}, t{string}) -> (t{integer})\n",
            "  b0(v0:t{string}, v1:t{boolean}, v2:t{string}):\n",
            "    condBranch v1 b1 b2\n",
            "  b1():\n",
            "    branch b3 (v0)\n",
            "  b2():\n",
            "    branch b3 (v0)\n",
            "  b3(v3:t{string}):\n",
            "    v4:t{view} = viewCreate text v3 lender parameter#0 unit scalars boundary utf8 lifetime#1\n",
            "    v5:t{integer} = viewLength text v4\n",
            "    do v6 viewEnd lifetime#1\n",
            "    return (v5)\n",
        ),
        string = string.raw(),
        boolean = boolean.raw(),
        integer = integer.raw(),
        view = view.raw(),
    );
    let joined = parse_mir_dump(&valid).expect("joined parameter lender MIR");
    assert_eq!(verify_mir_bubble(&joined, &types), Ok(()));

    let mixed = parse_mir_dump(&valid.replacen("    branch b3 (v0)\n", "    branch b3 (v2)\n", 1))
        .expect("mixed lender join MIR");
    assert!(matches!(
        verify_mir_bubble(&mixed, &types),
        Err(errors) if errors.iter().any(|error| matches!(
            error,
            MirVerificationError::InvalidViewOperation { instruction }
                if instruction.raw() == 4
        ))
    ));
}

#[test]
fn view_contract_vocabulary_is_closed_and_typed() {
    assert_eq!(MirViewKind::Bytes.range_unit(), MirViewRangeUnit::Bytes);
    assert_eq!(
        MirViewKind::Text.boundary_proof(),
        MirViewBoundaryProof::Utf8Scalar
    );
    assert_eq!(MirViewTrap::BoundsViolation.to_string(), "BoundsViolation");
    assert_eq!(
        MirViewLender::Parameter { index: 3 }.parameter_index(),
        Some(3)
    );
    assert_ne!(LifetimeId::from_raw(4), LifetimeId::from_raw(5));
    assert_ne!(AllocationSiteId::from_raw(7), AllocationSiteId::from_raw(8));
    let _ = MirVerificationError::InvalidViewLifetime {
        lifetime: LifetimeId::from_raw(1),
    };
}

#[test]
fn direct_alias_call_contract_round_trips_and_rejects_corruption() {
    let source = SourceFile::new(
        FileId::from_raw(0),
        "src/views.pop",
        "namespace Main\n\
         private function middle(view: Bytes.View): Bytes.View\n\
             return Bytes.slice(view, 2, 2)\n\
         end\n\
         public function copyMiddle(bytes: Bytes): Bytes\n\
             local whole = Bytes.view(bytes)\n\
             return Bytes.toBytes(middle(whole))\n\
         end\n",
    )
    .expect("view source");
    let front_end = analyze_bubble(FrontEndBubbleInput::new(
        BubbleId::from_raw(0),
        NamespaceId::from_raw(0),
        Vec::new(),
        vec![FrontEndModule::new(ModuleId::from_raw(0), source)],
    ));
    assert!(front_end.diagnostics().is_empty());
    let mir = lower_hir_bubble(front_end.hir().expect("HIR"), front_end.types())
        .expect("call-borrow MIR");
    let dump = mir.dump();
    assert!(
        dump.contains("lifetimeSummary(v1;parameters=DoesNotRetain;results=ReturnsAlias#0) viewResult(bytes,source#0,lifetime#"),
        "{dump}"
    );
    assert_eq!(
        parse_mir_dump(&dump)
            .expect("call-borrow round trip")
            .dump(),
        dump
    );

    let corrupt = dump.replacen("viewResult(bytes,source#0", "viewResult(bytes,source#9", 1);
    let corrupt = parse_mir_dump(&corrupt).expect("structurally valid corrupt call borrow");
    assert!(verify_mir_bubble(&corrupt, front_end.types()).is_err());
}
