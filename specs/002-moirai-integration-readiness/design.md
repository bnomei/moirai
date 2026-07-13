# Design — Moirai integration readiness

## Objective

Make Moirai a trustworthy integration target for Sea of Grass and `pd-asteroids` before either
consumer removes its current ECS. This spec corrects core ownership and event semantics, completes
the neutral scheduling/query/host APIs proven necessary by both consumers, and makes parity evidence
truthful. It does not port either game or perform a performance audit.

## Frozen boundaries

Moirai remains one dependency-free Rust 2021 crate with Rust 1.75, unconditional `alloc`, additive
`std` and `testkit` features, and `#![forbid(unsafe_code)]`. `App` continues to own sibling `World`
and `Schedule` values. Storage stays private; packed entity IDs are neither exposed nor serialized.
Wyrd, Anapao, Bevy, Playdate, serde, proc macros, compatibility facades, and downstream policy do not
enter the crate.

The event model is typed broadcast: registered event payloads are `Clone + 'static`; readers own
their copied payloads; frame queues clear only at their registered operation boundary. Systems must
declare typed emission/consumption roles. Idle host access remains available, while access from a
running system or `QueryEffects` is checked against compiled metadata.

The query addition is intentionally smaller than Bevy's DSL. `QueryIds`/`QueryEntities` enumerate
matching live entities, and `EntityRef` gives checked immutable component access. Runtime
`ComponentId` selectors cover required, excluded, tag, added, and changed membership. Repeated
added filters and repeated changed filters use OR semantics; combining added and changed remains an
error. No `Optional<T>`, Query3+, arbitrary mutable tuples, name-based component identity, or raw
fingerprint constructors are added.

## Implementation sequence

The nine tasks are one serial dependency chain and map one-to-one to after-validation commits:

1. `fix(storage): move owned component values`
2. `fix(commands): consume deferred values and insert bundles`
3. `fix(events): preserve frame broadcasts`
4. `feat(schedule): validate typed event roles`
5. `feat(schedule): complete host authoring`
6. `feat(app): seed resources and harden state faults`
7. `feat(query): add entity and dynamic-id queries`
8. `feat(entity): add generational scratch storage`
9. `chore(api): make quality evidence truthful`

Each implementation worker receives only the narrow task context and allowlist. Orchid checks the
worker report and touched paths, then creates a fresh Sol/high validator from the canonical report.
No commit is permitted before that validator passes. A `needs_fix` verdict creates a repair packet:
use Terra/medium only for a bounded local correction whose contract is already fixed; retain
Sol/high when the repair touches an invariant, public API, lifecycle, or cross-module contract. A
new Sol/high validator reviews every repair. Stop rather than loop when R012's limit is reached.

## Data and failure semantics

Structural migration consumes erased values and transfers their stored change ticks; removal
returns the original value and drops it exactly once. Deferred bundle insertion is atomic from the
caller's perspective: a validation/enqueue failure leaves no partial command batch.

Schedule compilation expands explicit system-set relationships into deterministic system edges,
then validates stages, cycles, resources, and event roles. Cross-stage set-order declarations and
Render-owned structural flushes are build errors. Resource/state seeds are installed before this
validation; repeated seeds use last-call-wins and establish one initial change tick.

State requests for the current state are no-ops. Competing pending destinations produce
`StateError`; applying state declares its resource requirement. Once `App` terminally faults,
`fault()` returns the first retained fault.

`EntityScratch<V>` is World-owner-bound and validates a full `EntityId` against current liveness on
every entity-facing operation. It is transient system/local state, has no persistence conversion,
and never exposes a slot index.

## Verification and acceptance

Every slice follows red-green-refactor unless its task explicitly identifies quality-only existing
tests. Focused tests must prove the new invariant before the full repository gates run. Final
acceptance requires formatting, strict all-target/all-feature Clippy, the complete feature test
matrix, warnings-denied rustdoc, benchmark compilation without measurement, Rust 1.75
no-default-features library checking, allocation-regression tests, and deterministic parity-ledger
regeneration. Cargo uses the repository's normal `target/`; lock contention is handled by waiting.

| Requirement | Owning task |
| --- | --- |
| R001, R010–R012 | T001 |
| R002, R010–R012 | T002 |
| R003, R010–R012 | T003 |
| R004, R010–R012 | T004 |
| R005, R010–R012 | T005 |
| R006, R010–R012 | T006 |
| R007, R010–R012 | T007 |
| R008, R010–R012 | T008 |
| R009–R012 | T009 |
