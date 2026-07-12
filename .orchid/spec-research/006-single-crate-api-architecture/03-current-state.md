# Current State

## Moirai planning state before this research

- The earlier Phase 1 draft exposed every implementation file as `pub mod` and selected mutually
  exclusive numeric features.
- The earlier Phase 3 draft placed Schedule and system-registry fields inside World.
- The earlier Phase 4 draft ported runtime cycle panic and Playdate profiler hooks.
- The earlier Phase 7 draft proposed extra Moirai adapter crates and equated World tick with Wyrd
  host time.
- The current phase documents replace those drafts and treat this packet plus
  `docs/ARCHITECTURE.md` as their cross-phase authority.

## Source ECS

- `../pd-asteroids/game-core/src/ecs/mod.rs` publicly re-exports allocator, registry, raw command
  operations, event queues, raw stores, sparse storage, schedule, World, and query caches.
- Production game modules consume `World`, component/entity ids, system descriptors, QuerySpec, and
  both query caches. They do not consume EntityAllocator, ComponentRegistry, SparseSet, Resources,
  EventRegistry, EventQueue, Events, or CommandOp directly.
- `../pd-asteroids/game-core/src/ecs/world.rs#World::update` raw-pointers the embedded Schedule while
  safe system callbacks can reach `World::schedule_mut`.
- `../pd-asteroids/game-core/src/ecs/schedule.rs#Schedule::run_stage` raw-pointers ordered/system
  vectors during callbacks. Safe callbacks can therefore reach overlapping mutable state or
  invalidate storage.
- `../pd-asteroids/game-core/src/ecs/profiler.rs` directly calls Playdate SDK FFI.
- `../pd-asteroids/game-core/src/ecs/state.rs` hard-codes Menu/Game/Pause/Inventory.
- Entity liveness checks generation but not occupancy, while raw ids are constructible.
- Component registration treats matching name or TypeId as idempotent without checking layout/type
  conflicts.
- World structural methods always queue; there is no source run-state switch despite phase prose
  saying deferral happens only during systems.

## Sibling facades

- `../wyrd/crates/wyrd-for-games/src/lib.rs` keeps authoring/foundation/runtime implementation
  modules private, publishes semantic `core`, `graph`, and `runtime` facades, and re-exports selected
  happy-path names at root.
- `../anpao/src/lib.rs` hides engine/plan/validation internals, roots common Simulator/report types,
  and publishes a smaller `prelude`.
- `../anpao/tests/public_api.rs` and README snippet tests protect both import paths and docs.

## Numeric and interop facts

- Wyrd i32 `Signal` is domain-dependent: counts are raw i32, levels are Q16 bits, and mul shifts as
  Q16 (`../wyrd/crates/wyrd-for-games/src/foundation/signal.rs`). It is not a general Q16 scalar.
- Wyrd Runtime keeps private senses, previous inputs, counters, flags, timers, delay rings, OnStart
  state, RNG, and tick but has no public snapshot/restore API
  (`../wyrd/crates/wyrd-for-games/src/runtime_impl/bind.rs#Runtime`).
- Sea of Grass persists wiring nodes, delayed pulses, and settle step
  (`../sea-of-grass/src/wiring.rs#WiringSave`).
- Anapao `Simulator` runs only `CompiledScenario`; its public assertion functions accept public
  reports, but its node snapshots are f64-only (`../anpao/src/simulator.rs`,
  `../anpao/src/assertions/mod.rs`, `../anpao/src/types/reports.rs`).

## External primary guidance

- Rust visibility permits private implementation modules with curated `pub use` facades.
- Cargo features unify and are expected to be additive; globally exclusive numeric features are
  fragile.
- Rustdoc treats re-exports as a first-class documentation surface.
- Non-exhaustive public enums preserve room for future variants.

Curated URLs and Tavily provenance are under `raw/`.
