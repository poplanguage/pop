use pop_mir::is_managed_reference_type_id;
use pop_types::TypeArena;

#[test]
fn exact_optional_managed_values_remain_precise_gc_references() {
    let mut types = TypeArena::new();
    let string = types.source_type("String").expect("String");
    let integer = types.source_type("Int").expect("Int");
    let boolean = types.source_type("Boolean").expect("Boolean");
    let optional_string = types.optional(string).expect("String?");
    let optional_integer = types.optional(integer).expect("Int?");
    let wider_union = types
        .union([string, integer, boolean])
        .expect("String | Int | Boolean");

    assert!(is_managed_reference_type_id(string, Some(&types)));
    assert!(is_managed_reference_type_id(optional_string, Some(&types)));
    assert!(!is_managed_reference_type_id(
        optional_integer,
        Some(&types)
    ));
    assert!(!is_managed_reference_type_id(wider_union, Some(&types)));
}
