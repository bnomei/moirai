# Moirai architecture

This document is the architectural contract for the first public Moirai API. The phase documents
describe delivery order; this document owns the cross-phase shape that must remain coherent while
those phases are implemented.

## Verdict

Moirai should remain one published crate through 1.0. It should be internally deep but publicly
small: private storage/runtime machinery, stable semantic namespaces, curated crate-root exports,
and a deliberately small prelude.

The central runtime type is `App`, not `World`. `App` owns `World` and `Schedule` as sibling fields.
This avoids the unsafe self-aliasing used by pd-asteroids when a `World` owns the `Schedule` that is
currently executing it. `World` owns ECS data; `Schedule` owns executable systems; `App` owns the
outer lifecycle.

## Design principles

1. **One crate, many private modules.** File boundaries are implementation boundaries, not reasons
   to publish more crates.
2. **Public paths describe concepts.** `moirai::query` and `moirai::schedule` are stable namespaces;
   `query::iter::table` is not.
3. **The root is the happy path.** Common types are importable from `moirai::{App, World, System}`.
4. **The prelude is smaller than the root.** It contains system-authoring essentials, not every
   public type or optional integration.
5. **Configuration is fallible; execution is prepared.** Schedule cycles, missing resources,
   unknown event roles, and invalid feature combinations fail before the first update.
6. **Core is numeric-agnostic.** One world may contain `f32`, grid `i32`, and `Q16` components at the
   same time.
7. **Adapters point inward.** Wyrd and Anapao may depend on stable Moirai seams; storage and
   scheduling internals never depend on host games or test tooling.
8. **No unsafe code in Moirai.** Platform FFI and clocks remain host responsibilities.
9. **Single-threaded by contract.** Parallel scheduling and `Send + Sync` App guarantees are
   outside 1.0; downstream batch runners may impose stronger bounds on their own factories.

## Conceptual lifecycle

The three sibling libraries should feel related without sharing an artificial base trait:

| Library | Author/configure | Validate/compile | Execute |
| --- | --- | --- | --- |
| Moirai | `AppBuilder`, components, resources, systems | `AppBuilder::build` | `App::update`, `App::render` |
| Wyrd | `Weave` / `Recipe` | `Runtime::bind` / `Recipe::bind` | sample → `loom` → apply |
| Anapao | `ScenarioSpec`, `RunConfig` | `Simulator::compile` | deterministic run/batch |

Moirai must not create a common trait merely to make this table symmetrical. The useful shared
contract is behavioral: setup resolves names and validates topology; hot execution uses prepared
state and deterministic order.

The top-level names remain complementary:

```rust
use moirai::{App, World};
use wyrd::Runtime as WyrdRuntime;
use anapao::Simulator as ScenarioSimulator;
```

`App` is intentionally a thin World/Schedule execution owner, not a rendering/assets/plugins game
framework. Moirai avoids the names `Runtime` and `Simulator` so all three libraries remain clear in
one module. Their preludes contain no cross-library adapter names.

## Crate and module layout

```text
src/
  lib.rs                  # crate docs, attributes, root exports only
  prelude.rs              # curated system-authoring imports
  app.rs                  # App, AppBuilder, AppError/AppFault/BuildError, outer lifecycle
  operation.rs            # shared StageOperation enum; no Schedule state
  entity/
    mod.rs                # EntityId public facade
    allocator.rs          # private generational allocator
  component/
    mod.rs                # ComponentId, ComponentOptions, StorageKind
    registry.rs           # private TypeId/name registry and table factories
  storage/
    mod.rs                # private storage coordination
    erased.rs             # type-erased storage boundary
    sparse.rs             # SparseSet implementation
    table.rs              # typed table columns
    archetype.rs          # signatures, rows, moves, locations
  command/
    mod.rs                # borrowed Commands facade
    queue.rs              # private CommandOp and reusable buffers
  resource/
    mod.rs                # private typed resource store
  event/
    mod.rs                # EventId, EventReader, EventOptions, EventRetention
    registry.rs           # private typed/named channels
    queue.rs              # retention and multi-reader cursors
    component.rs          # component lifecycle channels
  world/
    mod.rs                # World facade and state ownership
    builder.rs            # WorldBuilder schema/data construction
    access.rs             # typed get/has/get_mut
    bundle.rs             # public Bundle/BundleWriter/DynamicBundle facade
    spawn.rs              # entity/component mutation
    flush.rs              # command application and archetype migration
    resources.rs          # public World resource methods
    events.rs             # public World event methods
    query/                 # private query implementation
      spec.rs
      filter.rs
      query1.rs
      query2.rs
      ids.rs
      cache.rs
      result_cache.rs
  query.rs                # stable query facade/re-exports
  schedule/
    mod.rs                # stable schedule facade
    stage.rs              # Stage, built-in stage constants
    system.rs             # System, SystemId, SystemSet
    condition.rs          # run conditions and generic state helpers
    builder.rs            # authoring graph and validation
    compiled.rs           # dense stages/order and runtime state
    runner.rs             # safe execution against &mut World
    error.rs              # non-exhaustive diagnostics
  time.rs                 # WorldTick, ChangeTick, FixedConfig, FixedStep
  state.rs                # generic State<S>, never a game-owned enum
  diagnostics.rs          # observer contracts and allocation-free aggregation
  math/
    mod.rs
    q16.rs                # real fixed-point Q16 newtype
  testkit/                # feature = testkit; deterministic replay primitives
    mod.rs
```

`storage`, `resource`, and command-operation internals stay private. Consumers need storage policy,
not access to `SparseSet`, `Archetype`, `ComponentRegistry`, `EntityAllocator`, `Resources`,
`Events`, or `CommandOp`. Keeping those types private is what lets one crate evolve internally.

## Dependency direction

```text
entity ───────────────┐
component ────────────┼→ storage ─┐
math                  │           │
event ────────────────┤           ├→ World ← query implementation
resource ─────────────┤           │
command ──────────────┘           │
                                  ├→ Schedule
World + Schedule ─────────────────┴→ App
App public seams ───────────────────→ testkit ← downstream Wyrd/Anapao adapters
```

The forbidden edge is `World → Schedule`. `World` must not store a schedule, system registry, or
profiler clock. `Schedule` may execute against `&mut World`; `App` may mutably borrow both fields.
The leaf `operation.rs` enum may be imported by event, schedule, and app; that classification carries
no executable Schedule reference or registry.

## Public facade

### Crate-root exports

The root should export the common, stable vocabulary:

```rust
pub use app::{App, AppBuilder, AppError, AppFault, BuildError};
pub use command::Commands;
pub use component::{ComponentId, ComponentOptions, StorageKind};
pub use entity::EntityId;
pub use event::{EventId, EventOptions, EventReader, EventRetention};
pub use query::{
    ExactIdPolicy, PreparedQuery1, PreparedQuery2, Query1, Query2, QueryCommands, QueryCursor,
    QueryEffects, QueryError, QueryPolicy, QuerySpec, QueryWindow,
};
pub use operation::StageOperation;
pub use schedule::{
    stage, Condition, ConditionError, FlushMode, Schedule, ScheduleBuilder, ScheduleError, StageId,
    System, SystemId, SystemInitContext, SystemSet,
};
pub use revision::{Revision, RevisionExhausted, RevisionKey};
pub use state::State;
pub use time::{ChangeTick, FixedConfig, FixedStep, WorldTick};
pub use world::{Bundle, DenseEntityScratch, DynamicBundle, EntityScratchError, World, WorldBuilder};
```

Low-level registries, raw command variants, storage containers, adapter types, and diagnostics do
not belong at the root.

### Prelude

`moirai::prelude::*` should contain only the names commonly needed to install and write systems:

```text
App, AppBuilder, World, WorldTick,
EntityId, DenseEntityScratch, EntityScratchError, Commands, Bundle, DynamicBundle,
ComponentOptions, StorageKind,
System, SystemSet, FlushMode,
ExactIdPolicy, PreparedQuery1, PreparedQuery2, QueryCursor, QueryError, QueryPolicy, QuerySpec,
QueryWindow, Revision, RevisionKey, StageOperation, State, StateError
```

Do not export adapter names, `Q16`, schedule handles, event readers, or every root name from the
prelude. The selected query/state errors are system-authoring results; explicit imports keep the
remaining collisions and optional-feature drift visible.

### Stable namespaces

- `moirai::component`: component ids and registration policy.
- `moirai::event`: event configuration and readers.
- `moirai::query`: query specifications, iterators, and caches.
- `moirai::schedule`: stages, systems, validation, and execution diagnostics.
- `moirai::world`: World errors, bundles, and advanced data-lifecycle policy.
- `moirai::math`: numeric helpers such as `Q16`.
- `moirai::diagnostics`: opt-in observers and profiler aggregation.
- `moirai::testkit`: an additive feature-owned surface, first published with its real replay
  vocabulary in Phase 6; never re-exported by the root or prelude.

Every root and namespace path needs a public-API integration test. README snippets must also compile
as doctests or exact snippet tests, following Anapao's public API and README drift tests.

## App, World, and Schedule

```rust
pub struct App {
    world: World,
    schedule: Schedule,
}

impl App {
    pub fn builder() -> AppBuilder;
    pub fn from_parts(world: World, schedule: Schedule) -> Result<Self, BuildError>;
    pub fn world(&self) -> &World;
    pub fn world_mut(&mut self) -> &mut World;
    pub fn schedule(&self) -> &Schedule;
    pub fn set_system_enabled(
        &mut self,
        system: &SystemId,
        enabled: bool,
    ) -> Result<(), ScheduleError>;
    pub fn update(&mut self, delta_seconds: f32) -> Result<(), AppError>;
    pub fn update_with<R>(
        &mut self,
        delta_seconds: f32,
        observe: impl FnOnce(&World) -> R,
    ) -> Result<R, AppError>;
    pub fn render(&mut self, delta_seconds: f32) -> Result<(), AppError>;
    pub fn render_with<R>(
        &mut self,
        delta_seconds: f32,
        observe: impl FnOnce(&World) -> R,
    ) -> Result<R, AppError>;
}
```

`AppBuilder` owns configuration. It registers components/resources/events, collects systems, and
builds a validated dense schedule. `build()` returns a typed `BuildError`; it never leaves an
invalid partially runnable app.

`AppError` is shared by update/render because both enforce delta, pending-command, poison, and
terminal-fault preconditions. `AppFault` is the retained terminal execution record; `BuildError` is
construction-only. Calling Render must not produce an update-specific error name.

The advanced composition path is `WorldBuilder::build → ScheduleBuilder::build(&mut World) →
App::from_parts`. Building Schedule against World validates schema/resource/event contracts and
locks required resource types against removal. `Schedule::run` is crate-private: only App owns
outer tick, fixed-step, fault, final-flush, observation, and event-clear semantics. This makes
Schedule/World independently inspectable and constructible without publishing two competing
lifecycle APIs.

Schedule build creates one opaque `Rc<ExecutionLease>`. Schedule owns it; World stores only Weak
lease/required-resource locks. A live lease prevents compiling a second schedule for that World.
Dropping an unattached Schedule expires/prunes the locks, while `App::from_parts` requires the live
matching lease. This avoids a World → Schedule reference and avoids permanent locks after a
discarded advanced build.

`System` replaces the misleading `SystemMeta` name. Its constructor requires the run closure, so a
system cannot reach runtime without a body. Infallible and fallible constructors normalize the
migration-friendly `FnMut(&mut World, f32)` body into a crate-owned result because `App`—not
`World`—owns the schedule. Public fields become private builder state. Labels and event/resource
names are authoring-time values; `ScheduleBuilder::build` resolves them to dense ids and a cached
topological order. Cycles are a build error, not a runtime panic.

`StageId`, `SystemId`, and mutable schedule-control handles retain a private
`Rc<ScheduleOwner>` and slot/generation, rejecting cross-Schedule use. A stage handle is obtained
only with `Schedule::stage_id(label)` and resolved with checked `Schedule::stage_label(&id)`; its raw
dense index is not public. Hot order arrays contain only private dense indices.

Every stage has an immutable `StageOperation::{Update, Render}` assigned when the stage is created.
The two-variant enum is physically defined in dependency-neutral `operation.rs`, then re-exported
from the crate root and `schedule`; event storage therefore does not gain a `World → Schedule`
dependency merely to classify lifetime. Built-in Startup, FixedUpdate, and Update stages belong to
Update; the built-in Render stage belongs to Render. Custom-stage creation requires the operation,
and ordering edges may connect only stages in the same operation. `App::update` and `App::render`
execute their respective operation-local DAGs. A Sea of Grass `RenderExtract` stage that constructs
a shell snapshot is Update-owned; a Moirai system that performs actual presentation is Render-owned.

Update is the only structurally mutating operation. It performs its final structural flush, calls
its read-only `update_with` observer, then clears every Update-owned frame channel. Render systems
may mutate existing component values, resources, and declared events, but cannot obtain structural
Commands; Render performs no structural flush, calls its observer after all Render systems, then
clears every Render-owned frame channel. A matching operation clears all events currently queued on
its frame channels—including external-source input queued immediately before the call—not merely
events emitted by its systems. It never clears the other operation's frame channels.
Events sent between two matching calls belong to the next call and preserve send order; if that
operation is never invoked, they remain queued until retention/capacity or channel exhaustion
returns an explicit error.

An idle command queue is never adopted implicitly by App. `App::from_parts` returns
`BuildError::PendingCommands`; later `update` and `render` calls return
`AppError::PendingIdleCommands` before a tick or system runs. The host must call idle-only
`World::flush` or `World::discard_commands`; discard releases Reserved ids safely. A system/flush
failure discards only the current uncommitted command batch, leaves already committed mutations
visible, and puts that App into a terminal explicit fault state rather than silently continuing a
partial frame. Expected domain failures are handled inside systems; rebuilding/restoring is the
recovery boundary for a failed execution.

`ScheduleBuilder::standard()` installs an Update DAG `Startup → FixedUpdate → Update` plus a
separate Render operation containing Render. Startup runs once on the first update; FixedUpdate
performs due substeps before Update; Render is called separately.
Fixed accumulation uses `core::time::Duration`, defaults to at most eight substeps, drops whole
excess steps while preserving the fractional remainder, and reports the dropped count. Hosts may
configure another cap/order explicitly. Fixed update is disabled until AppBuilder receives a
positive step; a build containing FixedUpdate systems without that configuration fails, and
`FixedConfig` has no `Default`. Schedule owns the private accumulator.
`world.fixed_step() -> Option<FixedStep>` exposes the current index/delta only while a fixed system
runs (first index 0). `Condition::fixed_step_mod` supports validated power-of-two cadence phases.
The pd-asteroids eight-stage order and sea-of-grass stage graph are host
presets in their respective install functions, not Moirai API.

The f32 update/render convenience validates finite, non-negative, Duration-representable input
before any tick or World mutation; zero delta remains valid.

`State` becomes `State<S> { current, previous, pending, changed_at }` with private fields.
`request(next)` is idempotent for the current/same pending value and rejects a conflicting pending
request; an explicitly installed transition system applies it at a host-selected boundary. It
retains one previous value, not a navigation/pause stack. The
`GameState { Menu, Game, Pause, Inventory }` enum and push/pop policy are pd-asteroids domain code.
`Condition::in_state(value)` and `Condition::state_changed::<S>()` are generic conveniences over
the resource. Attach them to a system with `System::run_if`, or to a registered set with
`AppBuilder::set_run_if`.
The base contract is `S: Eq + 'static` and does not require Clone. `state::apply::<S>(label, stage)`
constructs the generic transition system; AppBuilder never chooses its stage implicitly.

## Component, resource, and event conveniences

- `AppBuilder::world_builder().register_component::<T>(options)` uses `type_name::<T>()` as the
  default diagnostic name and returns `ComponentId`.
- An explicitly named/untyped tag API remains available for authored data and compatibility.
- `ComponentOptions` has private fields with `sparse()`, `table()`, and `tag()` constructors.
- Typed `tag()` registration accepts only zero-sized, non-dropping marker types; non-ZST or
  `needs_drop` types are rejected rather than silently discarding a value. Authored untyped tags
  carry a name and no Rust value.
- Registration is checked: an exact type/name/layout repeat is idempotent; name, type, or layout
  conflicts return a contextual error.
- `ComponentId` cannot be fabricated with a public unchecked constructor. Raw conversion, if
  needed for diagnostics, is explicitly named and does not imply cross-world stability.
- `ComponentId`, `EventId`, prepared queries, and cursors retain a private cloned `WorldOwner`
  token and reject cross-World use. `EventReader` and `QueryCursor` carry mutable progress,
  deliberately do not implement Clone, and require an explicit owner-validated `fork` for an
  independent cursor. Prepared queries are affine and keep resolved execution state local to one
  system.
- `EntityId` is an opaque 16-byte Copy handle with a private `u32` World owner token and packed
  `u32` slot/`u32` generation (initial generation 1). It has no public raw bit conversion.
  Cross-World use is rejected even when slot/generation happen to coincide, and same-World stale
  handles are rejected after despawn. Persistence/network protocols use host ids, never EntityId
  layout.
- Resources are type-keyed. Multiple logical instances use host newtypes; the unused pd named
  resource surface is not ported.
- A resource type declared required by compiled Schedule is locked in World. Idle replacement is
  allowed and advances its revision; removal returns `ResourceInUse`. Optional resources use
  explicit conditions and remain removable.
- World owns a checked monotonic `ChangeTick` independent from WorldTick. Component/resource
  add/change metadata records it, so filters and conditions detect mutations between fixed
  substeps sharing one WorldTick. Mutable access conservatively advances it when granted;
  structural flushes stamp committed additions/removals. `World::resource_scope_ref` preserves
  ticks for immutable access; `World::resource_scope_mut` advances the changed tick exactly once
  when the resource is present. Both mark one resource type as scoped while a callback receives
  that value and the rest of World; same-type access is rejected rather than aliased.
- Typed events are the default: `add_event::<E>`, `send::<E>`, `event_reader::<E>`,
  `System::emits::<E>`, and `System::consumes::<E>`, all under one explicit
  `E: Clone + 'static` broadcast contract. No named/dynamic public event API is published.
- `EventOptions::frame(StageOperation)` assigns every frame-retained channel to exactly one App
  operation. Its orthogonal `external_source()` builder flag declares input that may have no
  producer system. Otherwise a consumed event requires a compiled producer/order relation. During
  Schedule execution, undeclared or wrong-operation send/read attempts return an access error; host
  code outside execution may use registered channels directly. Persistent/manual/bounded channels
  may cross operations and retain their normal reader-defined lifetime.
- Event sends are never silently dropped because a runtime gate was disabled. Explicit
  `EventReader<E>` values own independent cursor state and cloned payloads; frame queues retain all
  broadcasts until their owning operation clears them, even before a reader exists. There is no
  anonymous/shared default reader. Readers can start at oldest retained or “from now,” report lag
  after retention loss, and may be explicitly forked at their current cursor. Checked absolute
  sequence numbers never wrap or reset on compaction.
- Component lifecycle events use component ids and typed helpers internally; public callers should
  not construct magic `OnAdd:<name>` strings.

## Commands and queries

`moirai::world::Bundle` is a safe downstream extension trait: its single consuming method writes
typed values through `BundleWriter`, whose only public operation is checked typed insertion.
Moirai supplies tuple implementations through arity 16 and `DynamicBundle` for conditional or
authored cases. Custom host bundles therefore need neither a derive crate nor access to storage
internals.

`Bundle` is curated at the crate root and in the prelude. `BundleWriter` intentionally remains at
`moirai::world::BundleWriter`, keeping the advanced authoring mechanism out of the happy-path
facade.

`CommandOp` is private. Public borrowed `Commands` exposes only valid operations over World-owned
reusable buffers.
The existing `Spawn`-is-a-no-op detail and `RunSystem` command do not become public semantics.
Setup/editor entity/component topology mutations on `World` are immediate. During schedule
execution, those immediate structural methods return `WorldError::StructuralMutationDuringRun`;
Update systems use `world.commands()` for deferred topology mutation, while Render systems receive
`WorldError::StructuralCommandsDuringRender`. Registered resource values and events have their
separately documented immediate sequential semantics. This makes `FlushMode` observable without
context-dependent entity/component behavior.
Deferred spawn reserves an id that is not live/query-visible until the whole structural batch
preflights and commits. A rejected batch applies no structural operation and releases reservations
without reviving stale ids.
Public `World::flush` and `World::discard_commands` are idle-only. A failed flush or explicit
discard releases every Reserved id in the queue without making it live. Only crate-private Update
execution can flush while the World run guard is active; Render rejects structural command access
with `WorldError::StructuralCommandsDuringRender` before anything is queued.

`FlushMode` has `Final` and `Stage`; individual Update systems may request `flush_after`. The custom
ScheduleBuilder defaults Update stages to Final, while the standard Update operation defaults to
Stage (including after each fixed substep). A structural flush policy attached to a Render stage is
a build error rather than an ignored option.

`QuerySpec` has private fields and builder methods. This lets Moirai add filters without a breaking
public-field change and prevents invalid combinations. User mistakes return `QueryError`; unknown
components and unsupported policy/filter combinations do not panic.

`QuerySpec::added::<T>` and `changed::<T>` compare component `ChangeTick` metadata over the frozen
half-open window `(since, captured_now]`, using `QueryWindow::Since` or an owner/spec-scoped
`QueryCursor` created from start or now. A lazy Query1/Query2 traversal advances its cursor to
`captured_now` only when iteration reaches exhaustion; dropping it early leaves the cursor
unchanged. Closure traversal advances only after complete success. `QueryPolicy::Membership`
materializes structural membership and applies temporal filters during traversal;
`QueryPolicy::DeltaMembership` maintains membership from per-entity structural mutation logs;
`QueryPolicy::Result` rejects moving added/changed windows because a materialized result cannot
represent them honestly.

Public execution begins with `World::prepare_query1` or `World::prepare_query2`; ad-hoc query and
cache-handle entry points remain crate-private differential controls. Mutable traversal is
closure-scoped (`for_each_mut`, `for_each_mut_mut`, and `for_each_mut_read`). Distinct storage/column indices are sorted and
borrowed with `split_at_mut`; mixed sparse/table access splits World storage fields; erased values
are then safely downcast. The mandatory sparse/sparse, table/table, sparse/table, and tag-filter
matrix is covered, while same-type aliasing is rejected. Moirai does not promise a general mutable
iterator, derive macro, or generic query DSL: the former would push type-erased aliasing toward
unsafe code and a proc macro would require a second crate.

Closure traversal may opt into `query::QueryEffects`, a restricted view over disjoint command/event
fields. It can queue structural Commands and send declared events without exposing World/resources
beside live component references. Its command view is available only in Update operation context;
Render may still use its declared event view. Resource-dependent traversal uses
`World::resource_scope_ref` or `World::resource_scope_mut`.

World-owned query entries use the private `WorldOwner` token allocated by a checked `AtomicU32`
counter. Prepared queries retain the owner token, so cross-World use is rejected without exposing
public raw keys.

### Exhaustion taxonomy

Checked counters do not share one overly broad failure policy:

- App computes a capped fixed-step plan locally and preflights the next WorldTick plus every due
  FixedStep before committing clocks/accumulator or running a system. Exhaustion terminally faults
  App; neither counter wraps.
- A failed ChangeTick allocation applies no mutation and marks World mutation-poisoned. All later
  World mutation and App execution reject persistently even if a system caught the original error;
  read-only inspection remains available. The runner checks this poison after every system and
  flush boundary.
- Entity generation exhaustion retires only that allocator slot. Query-cache slot generation
  exhaustion likewise retires only that slot; a new slot may be allocated and App is not poisoned.
- Event sequence exhaustion closes only the affected channel to future sends. Existing retained
  events remain readable; no sequence wraps. It faults App only if a scheduled system propagates
  that send error as its fatal result.

## Numeric contract

Moirai has no crate-wide numeric mode. Remove `numeric-f32` and `numeric-q16`.

Wyrd's `Signal` is domain-tagged by graph ports: on the i32 path `from_count(3)` is raw `3`, while
`from_level(0.5)` is Q16 bits and `mul` performs a Q16 shift. Copying that API as a general scalar
would make ordinary count multiplication produce surprising results.

`moirai::math::Q16` is instead a real, always-available fixed-point newtype with:

- private `i32` bits and `FRAC_BITS = 16`;
- `from_bits` / `to_bits` for exact boundaries;
- checked integer scaling and explicit f32 conversion; f32 conversion multiplies by `2^16` and
  rounds to nearest with halfway cases away from zero;
- checked division (division by zero is not silently `ZERO`);
- integer-intermediate checked multiply/divide using the same nearest/half-away rule;
- exact checked add/sub plus explicitly named saturating variants;
- no `Add/Sub/Mul/Div` operator impl whose overflow policy would be hidden.

Counts remain integers. Grid coordinates remain integers. Continuous game values may remain `f32`.
Wyrd adapter conversions are domain-explicit (`level_to_signal`, `count_to_signal`) and live in the
Wyrd-owned Moirai adapter.

## Features and MSRV

```toml
[features]
default = []
std = []
testkit = []
```

Core always assumes `alloc`; an empty `alloc` feature is unnecessary. Moirai features remain
additive. The unavoidable Wyrd numeric choice stays in the Wyrd-owned adapter crate, where it can be
tested without making Moirai's own `--all-features` build contradictory.

The core library MSRV remains Rust 1.75. Wyrd core also supports 1.75; Anapao uses Rust 1.85. Keeping
both outside Moirai preserves one MSRV. Coverage tooling runs on current stable and is outside the
library MSRV contract.

Moirai 1.0 is a single-threaded runtime. Component/resource/event values require `'static` but are
not globally forced to be `Send + Sync`, and App/World/Schedule do not promise those auto traits.
An Anapao batch adapter may require a stronger `Send` factory and construct/run each App wholly on
one worker.

Do not add `serde`, parallel execution, macros, reflection, or a profiler feature until a concrete
consumer requires that public behavior.

## Unsafe and diagnostics

Moirai uses `#![forbid(unsafe_code)]`.

The pd-asteroids source uses unsafe pointers to execute an embedded schedule while borrowing its
world, to iterate mutable systems beside cached order arrays, and to call Playdate profiler FFI.
The `App { world, schedule }` split removes the first alias. Field splitting and precompiled order
arrays remove the schedule raw pointers. Platform clocks and logging belong in host observers.
RAII run guards clear World execution state and poison App if a panic unwinds, without catching the
panic or attempting rollback.

`diagnostics` exposes one downstream extension method:

```rust
pub trait Observer {
    fn observe(&mut self, event: DiagnosticEvent<'_>);
}
```

`DiagnosticEvent` is non-exhaustive and reports update/render, stage/system boundaries, flushes,
fixed-debt drops, and faults using opaque ids/context. It never lends mutable World or Schedule.
The callback is synchronous, so a host observer can sample its own platform clock on start/end
events. App owns the optional boxed observer; when absent, execution pays a predictable branch and
performs no diagnostic allocation or clock call.

The entity allocator also strengthens the source contract: it tracks Free/Reserved/Live/Retired
state separately from generation, rejects same-World stale/double free, retires a slot on
generation overflow, and never treats a freed-but-not-reallocated slot as alive. Raw entity
construction is not public.

## Wyrd integration

Wyrd integration is owned by Wyrd, alongside `wyrd-for-games-bevy`. It may be a
`wyrd-for-games-moirai` crate or a Wyrd feature/module, but it is not a Moirai workspace member and
Moirai does not depend on Wyrd.

The source-audited Sea of Grass replacement traces and persistence corruption matrix live in
[`wyrd-parity.md`](./wyrd-parity.md); Phase 7 must consume that evidence rather than infer behavior
from legacy test names.

The default public abstraction is one atomic driver step:

```text
begin_frame(SettleTick) → binding.sample → runtime.loom → binding.apply
```

The Wyrd-owned `WyrdDriver<P, B>` owns a bound `wyrd::Runtime`, resolved ports, the host binding, an independent
`SettleTick`, and a retained fault state. `WyrdBinding` is a real downstream extension trait because
game code must define how components/resources map to senses and effects. The scheduled driver uses
the appropriate immutable or mutable resource scope and is ordered as one system (for SoG: after
actions, before portal travel).
Separate Sample/Loom/Apply systems are an advanced escape hatch, not the default API.
SettleTick exhaustion is checked before beginning a step and never wraps.

Tick domains must never be conflated:

| Tick | Advances when | First executed value |
| --- | --- | ---: |
| `WorldTick` | each outer `App::update` | 1 (pd parity) |
| `FixedStep` | each fixed substep | 0 |
| `SettleTick` | one atomic Wyrd driver step completes Apply | 0 |
| Anapao `StepIndex` | replay driver performs a step | 0 |

A condition-skipped action does not call or advance the driver. Any sample/loom/apply error leaves
its next `SettleTick` unchanged and sticky-faults driver plus App because each phase may already have
mutated runtime, binding, or World. Multiple completed fixed settles advance it multiple times. It
must not be derived from `WorldTick`.

### Persistence prerequisite

Sea-of-grass cannot delete `WiringState` after behavior tests alone. Its save data persists latches,
edge history, delayed pulses, and the wiring step. Wyrd currently exposes no runtime snapshot/restore
contract.

Before D14 can complete, Wyrd needs versioned, topology-bound runtime state with atomic restore. It
must retain held senses, previous inputs/decrement state, counters, flags, timers, OnStart state,
delay buffers/heads, RNG state, and an optional begun-frame/runtime tick while excluding ephemeral
outbox data. The Wyrd adapter exposes
`WyrdDriverState { runtime_state, last_completed_tick: Option<SettleTick>, next_tick }`; hosts persist
their own binding/domain state.

Restore validates those fields together. Fresh means last-completed None, next tick 0, and no begun
runtime frame. After Apply at tick `n`, runtime frame tick and last-completed both equal `n` and next
equals `n.checked_add(1)`. Mixed-but-individually-valid values reject atomically before Runtime,
driver, or host mutation; faulted/mid-step state is not snapshot-capable.

Required continuation tests compare uninterrupted execution with save/rebind/restore across latch,
mid-delay, held-sense, timer, and RNG cases, and reject topology/numeric/cross-field tick mismatches.

## Anapao integration

Anapao's `Simulator` executes an Anapao `CompiledScenario`; it cannot directly execute an arbitrary
Moirai `World`. Moirai must not pretend otherwise.

`moirai::testkit` owns neutral deterministic replay primitives: step indices, seeded factories,
exact host-defined snapshots, selected scalar metrics, and a replay report. Exact ECS equality stays
typed (`S: Eq`) instead of being forced into Anapao's `f64` node snapshots.

Its `ReplayDriver` returns one `StepRecord<S>` per checked StepIndex; a finite ReplayConfig owns the
seed/capture policy, and failure retains a partial report. The App helper captures through
`update_with` using host snapshot/metric closures. Testkit owns no RNG, serializer, reflection, or
sibling assertion type.

Raw exhaustion and schedule-inspection hooks are crate-internal test support. The public
`moirai::testkit` feature contains deterministic replay and evidence types, not counter mutation,
fault injection, or compiled-schedule inspection controls.

The Anapao-owned Moirai adapter is a bridge, not a second simulator. It maps selected scalar
metrics and diagnostic events from `moirai::testkit` into Anapao's public report/assertion/artifact types. It calls
`evaluate_run_expectations` or related public functions; it does not call `Simulator::compile`.
Anapao remains outside Moirai, std-only, and higher-MSRV.

A canonical ecosystem test should:

1. construct a seeded `App` and Wyrd driver;
2. execute ten action steps;
3. capture an exact `DoorSnapshot` after each final flush;
4. repeat with the same seed and compare snapshots;
5. bridge scalar `door.open` observations to Anapao expectations/artifacts;
6. repeat with save/restore at step five and compare steps six through ten.

## Public API and compatibility tests

- `tests/public_api.rs`: root exports, namespace paths, and prelude compile together.
- doctests: the ordered `moirai::examples::tier_*` learning path plus crate privacy checks; the
  `testkit` feature adds the deterministic replay lesson.
- `trybuild`: invalid sealed/internal extension points stay inaccessible where applicable.
- feature matrix: core no_std, core std, and testkit. Wyrd and Anapao adapters own their matrices.
- `cargo semver-checks`: begin once the first public baseline is published.
- public enums likely to grow are `#[non_exhaustive]`; public structs keep fields private.
- no public trait exists solely to mock internals. `Bundle`, `WyrdBinding`, and diagnostics
  observers are legitimate downstream implementation points.

## Do not refactor into

- multiple microcrates before 1.0;
- public storage/registry internals;
- a universal numeric `Signal` type;
- a Bevy-compatible derive/proc-macro layer;
- host-specific schedule presets in core;
- a shared Moirai/Wyrd/Anapao base trait;
- a fake Anapao `ScenarioSpec` that merely wraps an ECS app.

## Phase ownership and current state

- Phases 1–6 are implemented in this repository; the current integration-readiness work reconciles
  their facade and evidence before downstream harnesses begin.
- Phase 1 froze the crate facade, features, private module shells, and API compile tests.
- Phase 2 implemented entity/component/storage, the true `Q16` newtype, and a minimal sparse
  `WorldBuilder`/`World` slice needed to prove storage invariants.
- Phase 3 completed World data ownership—typed resources/events, commands, lifecycle guards, and
  safe resource scopes—atop the Phase 2 sparse-world foundation.
- Phase 4 implemented ScheduleBuilder, compiled Schedule, App, generic State, and observers without
  unsafe code.
- Phase 5 implemented private query machinery behind the stable query facade.
- Phase 6 implemented the neutral testkit and reconciled the Phase 0 classified characterization
  corpus—preserved, adapted, or intentionally rejected—and carries reproducible feature, public API,
  allocation-regression, and benchmark-build gates. This integration-readiness pass does not claim
  a new performance result.
- Phase 7 remains downstream work: it consumes the core testkit, coordinates Wyrd/Anapao adapters, and
  performs host migrations after Wyrd persistence is available.

## External validation

The module/facade and feature strategy was checked against the Rust visibility rules, rustdoc
re-export behavior, Cargo feature semantics, weak dependency features, and non-exhaustive API
guidance:

- https://doc.rust-lang.org/reference/visibility-and-privacy.html
- https://doc.rust-lang.org/rustdoc/write-documentation/re-exports.html
- https://doc.rust-lang.org/cargo/reference/features.html
- https://rust-lang.github.io/rfcs/3143-cargo-weak-namespaced-features.html
- https://rust-lang.github.io/rfcs/2008-non-exhaustive.html
- https://rust-lang.github.io/api-guidelines/naming.html

Private implementation modules plus curated re-exports and a small prelude are design inferences
built on those language guarantees. Cargo explicitly unifies features, which is why numeric
exclusivity is removed from core and isolated at the Wyrd dependency boundary.
