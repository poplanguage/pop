# ADR 0161: Actor Operation Vocabulary

- Status: accepted
- Date: 2026-08-03

The backend-neutral `RuntimeOperation` vocabulary includes typed local-Actor
create, activate, identity-checked try-send, opaque-reference try-send,
try-receive, begin-exit, complete-exit, and release operations. The opaque
reference adapter recovers the exact stored incarnation before admission; it
does not perform symbolic lookup or retarget an old reference. Native symbol
resolution maps each operation to the closed ABI symbols from ADR 0158.
