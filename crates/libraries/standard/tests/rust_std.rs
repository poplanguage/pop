use pop_library_bridge::{FoundationBubble, NativeEffect};
use pop_standard::{
    RUST_STD_EXPORTS, pop_std_rust_available_parallelism, pop_std_rust_net_ipv4_is_documentation,
    pop_std_rust_net_ipv4_is_link_local, pop_std_rust_net_ipv6_is_unique_local,
    pop_std_rust_process_id,
};

#[test]
fn rust_std_host_bridge_has_typed_exports() {
    assert_eq!(RUST_STD_EXPORTS.len(), 16);
    assert!(
        RUST_STD_EXPORTS
            .iter()
            .all(|export| export.bubble() == FoundationBubble::Standard)
    );
    assert!(RUST_STD_EXPORTS[..4].iter().all(|export| {
        export.parameters().is_empty() && export.effects() == [NativeEffect::AmbientIo]
    }));
    assert!(
        RUST_STD_EXPORTS[8..]
            .iter()
            .all(|export| export.effects().is_empty())
    );
    assert_eq!(RUST_STD_EXPORTS[0].namespace(), "Pop.Process");
    assert_eq!(RUST_STD_EXPORTS[0].name(), "id");
    assert_eq!(RUST_STD_EXPORTS[2].namespace(), "Pop.Terminal");
    assert!(pop_std_rust_process_id() > 0);
    assert!(pop_std_rust_available_parallelism() > 0);
    assert!(pop_std_rust_net_ipv4_is_link_local(u64::from(
        u32::from_be_bytes([169, 254, 1, 1])
    )));
    assert!(pop_std_rust_net_ipv4_is_documentation(u64::from(
        u32::from_be_bytes([192, 0, 2, 1])
    )));
    assert!(pop_std_rust_net_ipv6_is_unique_local(0xfd00_0000, 0, 0, 1));
}
