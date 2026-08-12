use super::{checked_range, pop_rt_text_view_get_rune, scalar_byte_offset};
use crate::state::lock_native_runtime_test;
use crate::text::allocate_utf8_string_literal;

#[test]
fn checked_ranges_are_one_based_and_overflow_safe() {
    assert_eq!(checked_range(4, 1, 4), Some((0, 4)));
    assert_eq!(checked_range(4, 5, 0), Some((4, 0)));
    assert_eq!(checked_range(4, 0, 0), None);
    assert_eq!(checked_range(4, 6, 0), None);
    assert_eq!(checked_range(4, 4, 2), None);
    assert_eq!(checked_range(4, 1, -1), None);
    assert_eq!(checked_range(4, 2, i64::MAX), None);
}

#[test]
fn scalar_offsets_never_split_utf8() {
    let text = "AéZ";
    assert_eq!(scalar_byte_offset(text, 0), Some(0));
    assert_eq!(scalar_byte_offset(text, 1), Some(1));
    assert_eq!(scalar_byte_offset(text, 2), Some(3));
    assert_eq!(scalar_byte_offset(text, 3), Some(4));
    assert_eq!(scalar_byte_offset(text, 4), None);
}

#[test]
fn rune_access_decodes_exact_utf8_scalars_and_rejects_invalid_ranges() {
    let _guard = lock_native_runtime_test();
    let text = "Aé中😀z";
    let reference = allocate_utf8_string_literal(text.as_bytes());
    assert_ne!(reference, 0);

    for (index, expected) in [0x41, 0xE9, 0x4E2D, 0x1F600, 0x7A].into_iter().enumerate() {
        let mut found = u32::MAX;
        // SAFETY: `found` is writable for one `u32`.
        let present = unsafe {
            pop_rt_text_view_get_rune(
                reference,
                0,
                u64::try_from(text.len()).expect("bounded test text"),
                5,
                i64::try_from(index + 1).expect("bounded scalar index"),
                &raw mut found,
            )
        };
        assert_eq!(present, 1);
        assert_eq!(found, expected);
    }

    for index in [i64::MIN, -1, 0, 6, i64::MAX] {
        let mut found = u32::MAX;
        // SAFETY: `found` is writable for one `u32`.
        let present = unsafe {
            pop_rt_text_view_get_rune(
                reference,
                0,
                u64::try_from(text.len()).expect("bounded test text"),
                5,
                index,
                &raw mut found,
            )
        };
        assert_eq!(present, 0);
        assert_eq!(found, u32::MAX);
    }

    let mut sliced = 0;
    // SAFETY: `sliced` is writable for one `u32`.
    assert_eq!(
        unsafe { pop_rt_text_view_get_rune(reference, 1, 9, 3, 3, &raw mut sliced) },
        1
    );
    assert_eq!(sliced, 0x1F600);
    // SAFETY: each output pointer is writable for one `u32`.
    assert_eq!(
        unsafe { pop_rt_text_view_get_rune(reference, 2, 9, 3, 1, &raw mut sliced) },
        0
    );
    // SAFETY: each output pointer is writable for one `u32`.
    assert_eq!(
        unsafe { pop_rt_text_view_get_rune(0, 0, 1, 1, 1, &raw mut sliced) },
        0
    );
    // SAFETY: a null output is explicitly rejected before dereferencing.
    assert_eq!(
        unsafe { pop_rt_text_view_get_rune(reference, 0, 1, 1, 1, std::ptr::null_mut()) },
        0
    );
}
