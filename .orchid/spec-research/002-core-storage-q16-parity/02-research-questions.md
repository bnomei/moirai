# Research Questions

## Resolved

- Entity liveness needs generation **and** Free/Reserved/Live/Retired state.
- Generation overflow retires a slot; it never wraps into a usable stale identity.
- Component re-registration is idempotent only for an exact type/name/storage/layout match.
- Sparse storage validates liveness at World before indexing by slot.
- `Q16` is an always-present conventional fixed-point newtype, not Wyrd `Signal`.
- Counts, grid integers, f32 values, and Q16 values coexist in one World/build.

## Deferred to implementation spec

- None after Phase 0 accepts the 32-bit slot/32-bit generation and no-raw-conversion lock.
