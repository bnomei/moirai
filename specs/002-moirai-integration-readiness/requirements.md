# Requirements — Moirai integration readiness

R001: WHEN a component is inserted, removed, deferred, or retained through an archetype migration
  THE WORLD SHALL transfer its owned value and change ticks without requiring `Clone`, changing
  identity, double-dropping, or leaking it.
R002: WHEN structural commands are deferred THEY SHALL consume owned values, support atomic bundle
  insertion on an existing entity, expose query-scoped insert/remove/bundle operations, and roll
  back a rejected enqueue without partially mutating the batch.
R003: WHEN a frame event has been sent before any reader is active THE CHANNEL SHALL retain every
  event until its owning operation clears the frame, while independent readers receive cloned
  payloads under one explicit `E: Clone + 'static` broadcast contract.
R004: WHEN systems declare typed event roles THE BUILDER SHALL validate registration, operation
  ownership, external-source policy, producer existence, and producer-before-consumer reachability,
  and execution SHALL reject undeclared event access from systems and query effects.
R005: WHEN a host authors a schedule IT SHALL support world predicates, configurable Update-stage
  flushes, builder-order-independent fixed-stage configuration, and deterministic system/set
  ordering while rejecting cross-stage set order and invalid Render structural flushes.
R006: WHEN an application is built IT SHALL seed resources and state before schedule validation,
  expose the retained terminal fault, make same-state requests idempotent, and report conflicting
  state transitions with a dedicated error.
R007: WHEN a consumer needs entity-only or runtime-ID queries THE WORLD SHALL provide checked
  entity enumeration, immutable entity views, dynamic required/excluded/tag/added/changed filters,
  exact IDs, caches, cursors, partial-iteration semantics, owner validation, and cursor forking.
R008: WHEN a system needs ephemeral entity-keyed scratch state IT SHALL use an owner-bound
  generational `EntityScratch<V>` that validates owner, generation, and liveness without exposing
  or persisting packed entity identity.
R009: WHEN Moirai publishes or claims an integration contract ITS facade, testkit extensions,
  parity ledger, generated evidence, README, roadmap, rustdoc, changelog, and public-path tests
  SHALL agree with real executable behavior.
R010: WHEN this spec is implemented THE CRATE SHALL remain one dependency-pure Rust 1.75
  `no_std + alloc` crate with `#![forbid(unsafe_code)]`, private packed entity identity, no Bevy
  compatibility facade, no proc macro, and no downstream Wyrd or game dependency.
R011: WHEN a task requests completion A fresh Sol/high validator SHALL independently review the
  worker report, diff, semantics, public compatibility, and focused/full validation evidence before
  Orchid may stage or commit it.
R012: WHEN validation returns `needs_fix` THE ORCHESTRATOR SHALL run a bounded repair-and-revalidate
  loop and SHALL stop after three repair rounds, the same root defect twice, a blocked verdict, an
  attribution conflict, or a newly required architecture decision outside this spec.
