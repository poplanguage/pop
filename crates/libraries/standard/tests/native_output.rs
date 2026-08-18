use pop_library_bridge::{FoundationBubble, NativeEffect, PopAbiType};
use pop_standard::{
    NATIVE_EXPORTS, pop_std_print_int, pop_std_print_string, pop_std_terminal_flush,
    pop_std_terminal_write, pop_std_terminal_write_error, pop_std_terminal_write_line,
    print_string,
};

#[test]
fn native_output_adapters_keep_the_bootstrap_signatures() {
    let _: extern "C" fn(i64) = pop_std_print_int;
    let _: extern "C" fn(u64) = pop_std_print_string;
    let _: fn(&str) = print_string;
    let _: extern "C" fn(u64) -> bool = pop_std_terminal_write;
    let _: extern "C" fn(u64) -> bool = pop_std_terminal_write_line;
    let _: extern "C" fn(u64) -> bool = pop_std_terminal_write_error;
    let _: extern "C" fn() -> bool = pop_std_terminal_flush;

    assert_eq!(NATIVE_EXPORTS.len(), 2);
    assert!(NATIVE_EXPORTS.iter().all(|export| {
        export.bubble() == FoundationBubble::Standard
            && export.namespace() == "Pop"
            && export.name() == "print"
            && export.results().is_empty()
            && export.effects() == [NativeEffect::AmbientIo]
    }));
    assert_eq!(NATIVE_EXPORTS[0].parameters(), [PopAbiType::Int]);
    assert_eq!(NATIVE_EXPORTS[1].parameters(), [PopAbiType::String]);
}
