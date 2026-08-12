//! Allocation-free scalar steps for one validated immutable UTF-8 String.

use pop_runtime_collector::StableGenerationalRuntime;
use pop_runtime_interface::{ManagedReference, ObjectSlot};
use pop_runtime_native_abi::IterationStatus;

use super::IterationStep;

pub(super) fn string_iteration_item(
    runtime: &StableGenerationalRuntime,
    source: u64,
    position: u64,
) -> Result<IterationStep, IterationStatus> {
    let source = ManagedReference::new(source);
    let length = runtime
        .array_length(source)
        .ok_or(IterationStatus::Failure)?;
    if position >= length {
        return Ok(IterationStep {
            mutation_token: length,
            item: None,
            next_position: position,
            state: 2,
        });
    }
    let first = load_string_byte(runtime, source, position)?;
    let width = match first {
        0x00..=0x7f => 1_u64,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return Err(IterationStatus::Failure),
    };
    let end = position
        .checked_add(width)
        .filter(|end| *end <= length)
        .ok_or(IterationStatus::Failure)?;
    let mut encoded = [0_u8; 4];
    for offset in 0..width {
        encoded[usize::try_from(offset).map_err(|_| IterationStatus::Failure)?] =
            load_string_byte(runtime, source, position + offset)?;
    }
    let width = usize::try_from(width).map_err(|_| IterationStatus::Failure)?;
    let text = std::str::from_utf8(&encoded[..width]).map_err(|_| IterationStatus::Failure)?;
    let mut scalars = text.chars();
    let scalar = scalars.next().ok_or(IterationStatus::Failure)?;
    if scalars.next().is_some() || scalar.len_utf8() != width {
        return Err(IterationStatus::Failure);
    }
    Ok(IterationStep {
        mutation_token: length,
        item: Some(u64::from(u32::from(scalar))),
        next_position: end,
        state: 0,
    })
}

fn load_string_byte(
    runtime: &StableGenerationalRuntime,
    source: ManagedReference,
    position: u64,
) -> Result<u8, IterationStatus> {
    let slot = u32::try_from(position).map_err(|_| IterationStatus::Failure)?;
    runtime
        .load_array_value(source, ObjectSlot::new(slot))
        .map_err(|_| IterationStatus::Failure)
        .and_then(|value| u8::try_from(value).map_err(|_| IterationStatus::Failure))
}
