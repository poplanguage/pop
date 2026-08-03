# ADR 0161: Actor Operation Vocabulary

- Status: accepted
- Date: 2026-08-03

The backend-neutral `RuntimeOperation` vocabulary includes typed local-Actor
create, activate, try-send, try-receive, begin-exit, complete-exit, and release
operations. Native symbol resolution maps each operation to the closed ABI
symbols from ADR 0158. Scheduler integration, compiler message-safety call
checking, and public Actor lowering remain subsequent layers.
