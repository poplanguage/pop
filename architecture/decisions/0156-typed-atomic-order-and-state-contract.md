# ADR 0156: Typed Atomic Order and State Contract

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0022, 0068, 0110, and 0145
- Supersedes: the planned-only first Atomic foundation slice

## Context

Channel, scheduler, lock, and host-integration implementations need exact atomic
operations. One universal memory-order enum would permit invalid load/store and
compare-exchange combinations to reach a backend or host intrinsic.

## Decision

The backend-neutral runtime contract defines distinct `AtomicLoadOrder`,
`AtomicStoreOrder`, and `AtomicReadModifyWriteOrder` enums. They admit only
orders valid for that operation category. `AtomicCompareExchangeOrder`
validates that its load-only failure order is no stronger than its success
order; invalid pairs are unrepresentable after construction.

The first state types are `AtomicInt` over exact signed 64-bit values and
`AtomicBoolean`. They support allocation-free load, store, swap, and
strong compare-exchange. Compare-exchange returns the observed prior value and
an exact exchanged flag. Spurious failure is absent from this first contract.

The contract maps directly to language-independent acquire/release semantics;
it contains no LLVM objects, target intrinsics, raw managed pointers, dynamic
values, implicit sequential consistency, or string-selected orders. Integer
read-modify-write arithmetic, wait/notify, fences, public compiler-known types,
MIR operations, native ABI handles, and target capability diagnostics are
separate follow-on slices.

## Required conformance

Tests cover every legal order, invalid compare-exchange pairs, load/store/swap,
successful and failed compare-exchange, observed prior values, Boolean and
integer state separation, and deterministic cross-thread release/acquire
publication. Architecture regressions reject backend or managed-pointer
leakage.

## Consequences

Later public and native Atomic layers share one fail-closed ordering contract
instead of independently interpreting unsafe order combinations.
