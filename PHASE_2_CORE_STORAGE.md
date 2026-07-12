# Phase 2 — Checked identity, storage, and fixed-point math

**Status:** complete · 2026-07-12
**Depends on:** [Phase 1](./PHASE_1_SCAFFOLD.md)
**Research:** [packet 002](./.orchid/spec-research/002-core-storage-q16-parity/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)

## Goal

Implement the final identity, registration, sparse-storage, and `Q16` foundations with their
invariants proved. The phase ends with a small executable sparse-world path using final types—not
temporary public scaffolding—so registration, spawn, lookup, mutation, despawn, and stale-handle
behavior are tested together.

Tables/archetypes and the full command/event/resource lifecycle belong to Phase 3.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R201 | WHEN an entity is Free, Reserved, stale, or Retired `World::is_alive` SHALL return false. |
| R202 | WHEN a generation exhausts THE SLOT SHALL retire and never wrap into a usable identity. |
| R203 | WHEN EntityId is exposed IT SHALL remain an opaque 8-byte 32/32 World-relative handle with no raw conversion. |
| R204 | WHEN ComponentId crosses World ownership THE RC OWNER CHECK SHALL reject it. |
| R205 | WHEN component registration repeats ONLY an exact type/name/storage/layout match SHALL be idempotent. |
| R206 | WHEN registration fails THE REGISTRY SHALL remain unchanged and report both definitions. |
| R207 | WHEN sparse storage operates WORLD SHALL validate entity liveness before slot lookup. |
| R208 | WHEN component values are inserted/mutably accessed THEIR ChangeTick metadata SHALL follow the frozen rules. |
| R209 | WHEN Q16 converts or computes IT SHALL use the frozen checked/saturating and nearest-half-away semantics. |
| R210 | WHEN Phase 2 exits allocator/registry/storage containers SHALL remain private and the public sparse-world slice SHALL pass. |

## Ownership and dependency direction

```text
entity allocator ─┐
component registry├──→ sparse storage ─→ minimal World path
Q16               ┘
```

The allocator, registry, erased storage traits, and sparse container are crate-private.
`EntityId`, `ComponentId`, `ComponentOptions`, `StorageKind`, `WorldBuilder`/`World` accessors, and
`math::Q16` form the public seam.

No storage implementation is re-exported solely to make unit testing easier. Internal property and
state-machine tests live beside the implementation; downstream tests exercise `World`.

## Entity identity

`EntityId` is an opaque 8-byte pair of `u32` slot and `u32` generation. Generation starts at 1;
slot/generation fields and construction remain private. Its documented guarantees are:

- copyable, orderable, hashable, and usable in deterministic diagnostics;
- relative to the originating World by contract;
- rejected after despawn, including before the slot is reused;
- never resurrected by generation overflow;
- not constructible from unchecked public index/generation parts.

The private allocator tracks at least:

```text
generation[slot]
state[slot] = Free | Reserved | Live | Retired
free list
live and reserved counts
```

`is_alive` requires Live state and generation equality. Phase 3 uses Reserved for ids returned by
deferred spawn; Reserved is never query-visible. Despawn validates before mutating, clears liveness
once, increments generation, and returns the slot to the free list only if the generation can
advance safely. Overflow retires the slot permanently. Double free and stale free return contextual
errors and leave allocator state unchanged.

Moirai 1.0 exposes no raw bit constructor or conversion for EntityId. `Debug`, equality, ordering,
and hashing use the logical `(slot, generation)` pair without making layout a persistence protocol.
A compact Copy
EntityId does not carry an owner token: passing it to a different World is caller misuse and may
name a coincident slot/generation. Host persistence and network boundaries use host ids instead.

### Required allocator proof

- deterministic initial allocation order;
- size/alignment tests for the private 32/32 representation without promising FFI layout;
- reuse with a changed generation;
- freed-but-not-reallocated is dead;
- stale lookup, mutation, insert, remove, and despawn fail;
- double despawn is non-destructive;
- capacity growth preserves live handles;
- generation overflow retires rather than wraps;
- randomized allocate/free traces match a simple reference model;
- live, reserved, free, and retired counts remain consistent after every operation.

## Component registration

`ComponentOptions` has private fields and final constructors:

```rust
ComponentOptions::sparse()
ComponentOptions::table()
ComponentOptions::tag()
```

`StorageKind` is the public policy enum. Storage internals and type-erased factories remain
private.

Typed registration defaults its diagnostic name to `type_name::<T>()`. Dynamic/tag registration
may accept an authored name where required by migration. Registration returns an opaque,
registry-local `ComponentId`.

An exact repeated registration is idempotent only when every relevant property agrees:

- Rust `TypeId`, when typed;
- diagnostic/authored name;
- storage kind;
- tag versus data layout;
- size/alignment or erased factory identity as applicable;
- lifecycle/event policy if configuration includes it.

Name, type, storage, and layout collisions return a non-exhaustive contextual registration error.
There is no “first registration silently wins” behavior.

Typed `tag()` registration requires `size_of::<T>() == 0` and `!needs_drop::<T>()`. Moirai never
silently discards a non-ZST or dropping value merely because tag storage has no column. Authored
untyped tags have a name and no Rust payload.

`ComponentId` cannot be fabricated unchecked. It retains the builder/World's private `Rc` owner
token plus its dense slot, so another World rejects it; hot internals resolve it once to the dense
slot. It is not stable across builds or persistence formats. Persistence uses host schema
identifiers, never registry-local component ids.

### Required registration proof

- first typed registration succeeds;
- exact repeat returns the same id;
- same type with conflicting options fails;
- same name with different type/layout fails;
- typed data and tag conflicts fail;
- non-ZST and dropping typed tags fail;
- ids remain dense/stable for the lifetime of the built world;
- a failed registration leaves the registry unchanged;
- diagnostics identify the existing and requested definitions.

## Sparse storage

The private sparse set retains the useful pd-asteroids semantics while tightening errors:

- O(1) membership and lookup by entity slot plus full entity validation;
- dense iteration for occupied values;
- added/changed `ChangeTick` metadata beside each value;
- swap-remove with all reverse indices repaired;
- deterministic behavior for a deterministic operation sequence;
- reusable capacity after removals;
- no unsafe code.

Storage methods do not decide entity liveness. `World` validates the `EntityId` first, then calls
storage with a trusted live identity. This prevents a generation-blind sparse index from accepting
stale entities.

Insertion policy is explicit:

- inserting a missing component adds it;
- replacing an existing component returns/reports the previous value according to the final API;
- removing an absent component is a normal `None` only when the entity itself is live;
- operating on a stale entity is an error, not indistinguishable absence.

Insertion records the World-issued tick as added and changed. Replacement preserves added and
updates changed; removal discards both. Archetype moves in Phase 3 preserve ticks for retained
components.

Typed mutable access conservatively advances World ChangeTick and marks the component before
returning `&mut T`. The sparse container receives an already-issued tick from World; it does not own
a clock.

The vertical downstream test registers one sparse component, spawns entities, inserts/replaces,
iterates, despawns, reuses a slot, and proves the stale id cannot observe the replacement entity.

## Conventional Q16

`moirai::math::Q16` is always available:

```rust
#[repr(transparent)]
pub struct Q16(i32);
```

Required constants and boundaries:

- `FRAC_BITS = 16`;
- `ZERO`, `ONE`, `MIN`, `MAX`;
- `from_bits(i32)` and `to_bits() -> i32` for exact protocol/save boundaries;
- checked and saturating construction from integers;
- `try_from_f32` rejects NaN/infinity/out-of-range, scales by `2^16`, and rounds to nearest with
  halfway cases away from zero;
- saturating f32 conversion clamps finite/infinite bounds but still rejects NaN;
- explicit conversion back to `f32`;
- exact checked add/sub and clearly named saturating variants;
- checked mul/div computed in `i64`, rounded to nearest with halfway cases away from zero, then
  range-checked; zero division fails.

Do not implement `Add`, `Sub`, `Mul`, `Div`, or `Neg` for 1.0: those traits cannot expose checked
overflow/zero-division and would hide the policy. Callers choose named checked or saturating
methods. Comparison/order traits remain exact over the signed representation.

`Q16` is not Wyrd `Signal`. Counts remain `i32`. Wyrd domain adapters later convert Count and Level
through separate functions.

### Required numeric proof

- exact bit round trips;
- integer and fractional boundary values;
- positive and negative rounding symmetry;
- exact half-away-from-zero vectors for float, multiply, and divide;
- checked overflow/underflow;
- saturating limits;
- multiply/divide identities within documented precision;
- divide-by-zero rejection;
- comparison/order follows represented numeric value;
- representative f32 error bounds;
- layout assertion for `#[repr(transparent)]`;
- state/property tests against a wider integer reference calculation.

Game-math sizing follows binary-friendly representation where it affects real hot loops, but no
magic power-of-two capacity is exposed as a semantic guarantee.

## Public error policy

Expected absence uses `Option`: for example, a live entity without component `T`.
Contract violations use `Result`: stale entity, unregistered type, registration conflict, wrong
storage operation, capacity/overflow failure.

Errors contain the operation and relevant opaque ids/names. Public error enums are
`#[non_exhaustive]`; their source chains are available under `std` without making std mandatory.
No error path silently mutates half of a registry or storage.

## Tasks

- [x] **T201** Implement the opaque 32-bit slot/32-bit generation EntityId and occupancy-aware
  allocator.
- [x] **T202** Add allocator state-machine/property tests including forced generation exhaustion.
- [x] **T203** Implement private-field `ComponentOptions`, `StorageKind`, and opaque
  `ComponentId`.
- [x] **T204** Implement checked, transactional component registration.
- [x] **T205** Implement the private typed/erased sparse storage boundary without unsafe code.
- [x] **T206** Implement `WorldBuilder` and the final minimal World path needed to exercise sparse
  registration and lifecycle.
- [x] **T207** Add a downstream vertical-slice test using only approved public paths.
- [x] **T208** Implement conventional always-present `math::Q16`.
- [x] **T209** Add wide-reference numeric property tests and boundary cases.
- [x] **T210** Add initial sparse iteration and Q16 Divan benchmarks.
- [x] **T211** Confirm generated rustdoc exposes policy types, not allocator/registry/storage
  containers.

## Verification

```sh
cargo test --no-default-features
cargo test --features std
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo bench --bench storage
cargo bench --bench q16
```

Review tests must include mutation-on-error checks, not only returned error values.

## Risks and controls

| Risk | Control |
| --- | --- |
| Generation equality is mistaken for liveness | separate occupancy bit and forced freed-slot test |
| Component names silently alias layouts | exact idempotence tuple and transactional conflict errors |
| Storage leaks into public API | only policy/id/world facade is public |
| Raw id conversion bypasses validation | no unchecked public inverse; World validates every handle |
| Wyrd signal semantics contaminate Q16 | conventional math API plus domain-explicit later adapter |
| Horizontal layer cake hides integration bugs | final-type sparse-world vertical test in this phase |

## Exit criteria

- [x] Entity liveness invariants hold under randomized traces and overflow.
- [x] Registration conflicts are detected before any world runs.
- [x] Sparse lifecycle works end to end using only intended public paths.
- [x] `Q16` has conventional, documented, independently tested semantics.
- [x] No numeric feature exists.
- [x] No allocator, registry, or storage implementation type is public.
- [x] Phase 3 can add table/archetype moves without changing identity semantics.

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Phase 0 corrections](./PHASE_0_ANALYSIS.md#mandatory-corrections-discovered-during-research)
- [Rust type layout](https://doc.rust-lang.org/reference/type-layout.html)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
