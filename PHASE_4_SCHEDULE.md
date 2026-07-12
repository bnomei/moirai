# Phase 4 — Compiled Schedule, App lifecycle, and safe execution

**Status:** complete
**Depends on:** [Phase 3](./PHASE_3_WORLD_LIFECYCLE.md)
**Research:** [packet 003](./.orchid/spec-research/003-world-schedule-contract/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)

## Goal

Implement `ScheduleBuilder → Schedule` and `AppBuilder → App` so systems execute deterministically
against `&mut World` without embedded-schedule aliasing, raw pointers, runtime topology panics, or
host-specific policy in core.

Phase 4 owns conditions, generic state, fixed steps, flush policy, fallible execution, stable
end-of-operation observation, and platform-neutral diagnostics.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R401 | WHEN advanced parts compose WorldBuilder/ScheduleBuilder/App::from_parts SHALL validate one matching ExecutionLease. |
| R402 | WHEN Schedule exists ITS run method SHALL remain crate-private and App SHALL own the only complete lifecycle. |
| R403 | WHEN schedule topology/contracts are invalid BUILD SHALL fail before first execution with contextual paths. |
| R404 | WHEN systems execute App SHALL safely split sibling Schedule/World without raw pointers or unsafe code. |
| R405 | WHEN any stage is created ITS immutable Update/Render operation SHALL determine which App call executes it. |
| R406 | WHEN FixedUpdate has systems POSITIVE FixedConfig SHALL be required; default cap/debt policy SHALL match this spec. |
| R407 | WHEN Update flush policy runs custom Final, standard Stage, and explicit system boundaries SHALL have trace-tested visibility. |
| R408 | WHEN State transitions apply generic current/previous/pending semantics SHALL hold without a host stack. |
| R409 | WHEN conditions compose THEY SHALL short-circuit left-to-right and set conditions SHALL run once per stage pass. |
| R410 | WHEN a system/flush fails THE CURRENT BATCH SHALL discard and App SHALL become terminally faulted. |
| R411 | WHEN a panic unwinds RAII SHALL clear World Running and poison App before propagation. |
| R412 | WHEN update/render succeeds OBSERVATION SHALL occur at the stable operation end and all matching frame events SHALL clear afterward. |
| R413 | WHEN event roles compile scheduled access/order/external-source/operation-boundary rules SHALL be enforced. |
| R414 | WHEN diagnostics are enabled ONLY the one-method neutral observer event seam SHALL run. |
| R415 | WHEN WorldTick/FixedStep exhaust App SHALL fault; World/cache/event counters SHALL follow their separate Phase 3/5 policies. |
| R416 | WHEN standard update runs ORDER SHALL be Startup-once, due FixedUpdate substeps, then Update. |
| R417 | WHEN App sees idle queued Commands IT SHALL reject before ticks/systems until World flush/discard resolves them. |

## Runtime ownership

```rust
pub struct App {
    world: World,
    schedule: Schedule,
    // lifecycle/fault bookkeeping
}
```

`App` is the top-level convenience; `WorldBuilder`, `World`, `ScheduleBuilder`, and `Schedule`
remain independently nameable advanced types. The only execution edge is:

```text
App ──mutably splits──→ pub(crate) Schedule::run(&mut World)
```

`World` contains no schedule and exposes no `schedule_mut`. A running system cannot invalidate the
system vector or recursively run the same schedule through safe World access.

`App` provides read-only `world()` and `schedule()`, mutable `world_mut()` only while not running,
and narrow runtime controls such as enabling a known `SystemId`. Required resource types are locked
against removal, so this mutable data access cannot invalidate compiled dependencies. General
topology mutation requires rebuilding through `AppBuilder`. If idle host code uses
`world_mut().commands()`, the host must explicitly flush or discard that batch before the next App
operation; App never chooses its visibility boundary implicitly.

`App::set_system_enabled(&SystemId, bool)` is the 1.0 runtime schedule control. It validates the
Rc owner/slot generation and changes only an enabled bit. There is no public system insertion,
removal, reorder, or immediate one-shot execution after build.

## Authoring and build boundary

`AppBuilder` owns all configuration:

- component, resource, and event registrations;
- initial entities/resources;
- schedule stages and systems;
- fixed-step configuration;
- diagnostics observer configuration if any.

`AppBuilder::build` first builds/finalizes World schema, then compiles Schedule against it. Failure
returns `BuildError` without a runnable partial App.

`ScheduleBuilder` may be used independently with an already prepared World. It has two
entrypoints:

- `ScheduleBuilder::new()` for a custom empty graph;
- `ScheduleBuilder::standard()` for an Update graph `Startup → FixedUpdate → Update` and a separate
  Render operation containing Render.

The pd-asteroids eight-stage graph and Sea of Grass action/portal order are downstream presets.
There is no `with_sea_of_grass_stages` or game state in Moirai.

The complete advanced construction path is:

```rust
let mut world = WorldBuilder::new() /* registrations/data */ .build()?;
let schedule = ScheduleBuilder::standard() /* systems */ .build(&mut world)?;
let app = App::from_parts(world, schedule)?;
```

`ScheduleBuilder::build(&mut World)` validates and locks required resources transactionally.
`App::from_parts` rejects mismatched World/Schedule owner tokens or dirty/running parts.
`Schedule::run` is not public; publishing it would create a second, incomplete lifecycle without
App's ticks, faults, Update flushes, observation, and event clearing.

Build also creates an opaque `Rc<ExecutionLease>` owned by Schedule. World stores only Weak
lease/resource-lock records. It rejects a second live compiled Schedule; dropping an unattached
Schedule expires and lazily prunes its locks. App::from_parts requires the matching live lease.
This is ownership validation, not a World-owned schedule registry.

### Stage labels

Built-in labels live under `moirai::schedule::stage`. `StageOperation` is the closed public
classification `Update | Render`, defined in a dependency-neutral module and re-exported at the root
and from `schedule`. Startup, FixedUpdate, and Update are Update-owned; Render is Render-owned.

Custom stages use an opaque, validated label with a stable diagnostic name and must be added with
their operation, conceptually `add_stage(label, StageOperation::Update)`. Raw dense stage indices
remain private. Duplicate labels are idempotent only when operation and ordering declaration agree.
Ordering edges are operation-local; a cross-operation edge is a build error. `App::update` traverses
only the Update DAG and `App::render` only the Render DAG. Registration order remains the final
deterministic tie-breaker within one operation.

`StageId`, `SystemId`, and other runtime schedule handles share a private `Rc<ScheduleOwner>` plus
slot/generation. They are Clone handles, resolve to dense private indices during build, and reject
cross-Schedule or stale use without a global counter.

Stage ordering is explicit and acyclic. A host extraction stage that creates a RenderSnapshot is
Update-owned because it finishes simulation state; an actual presentation system is Render-owned.

## System surface

`System` replaces `SystemMeta`. A system cannot exist without a body:

```rust
System::new("movement", stage::UPDATE, |world: &mut World, dt: f32| {
    // infallible common path
})

System::try_new("wyrd", host_stage, |world: &mut World, dt: f32| {
    // Result-returning advanced path
})
```

Fields are private. Builder methods configure:

- before/after `SystemId` or authoring label;
- membership in and ordering relative to `SystemSet`;
- run conditions;
- required resources;
- typed emitted/consumed events;
- flush mode;
- enabled state and diagnostic name.

The stored executable form normalizes both constructors to a crate-owned `SystemResult`. Fallible
error conversion allocates a diagnostic message only on failure, preserving no-allocation
steady-state execution.

The body remains conceptually `FnMut(&mut World, f32)` for migration. It cannot borrow Schedule
through World.

Moirai 1.0 deliberately omits the source's untyped system output pipes and per-system interval
buffer. No production host uses them, and interval carry/missed-run/multi-fire policy would create
a second time scheduler. Compose ordinary Rust functions inside one system; use FixedUpdate for
repeated simulation or a host-owned run condition/timer for throttling.

## Compile-time validation

`ScheduleBuilder::build` performs exact, contextual passes:

1. resolve unique stage/system/set labels and immutable stage operations to dense ids;
2. validate referenced stages, systems, and sets exist and reject cross-operation stage edges;
3. expand set ordering to system edges;
4. reject self-edges and dependency cycles with a readable path;
5. validate declared typed resources/events against World registration, require an operation-local
   producer/order relation or explicit external source where applicable, reject any system access to
   a frame channel owned by the other operation, permit retained/manual/bounded channels to bridge
   operations, and stage required-resource locks transactionally;
6. validate run-condition/state dependencies;
7. reject structural flush placement on Render and validate Update flush/fixed-stage configuration;
8. compute and cache deterministic topological order per operation and stage;
9. allocate reusable runner/diagnostic buffers.

All configuration errors are `Result` values before execution. Runtime never discovers an order
cycle and panics.

## Conditions and generic state

Run conditions inspect `&World` before a body receives `&mut World`. Conditions cannot structurally
mutate the world. Built-ins include:

- always/never;
- resource exists;
- resource added/changed since the condition's last successful observation;
- typed event available for a declared reader;
- `in_state(value)`;
- `state_changed`;
- explicit combinators whose evaluation order is documented.

Combinators evaluate left-to-right with short-circuit semantics. A SystemSet condition is evaluated
once per stage pass (once for each FixedUpdate substep), then each enabled member evaluates its own
conditions in declaration order.
Per-system resource/state change cursors advance only after that system completes successfully; a
short-circuited or failed system does not consume the observed change window.

Moirai provides `State<S>`, not a domain enum. A state value contains current plus an optional
previous and requested-next value plus transition ChangeTick. `State::request(next)` is idempotent
for the same pending value and rejects a conflicting second request at the same boundary.
Transition application is an explicitly installed system/boundary so hosts control whether it
occurs before Update, after actions, or elsewhere. Conditions observe one stable current state for
their stage pass. There is no push/pop overlay stack; hosts model pause/navigation stacks as their
own resource.

`State<S>` requires `S: Eq + 'static`, not Clone. `state::apply::<S>(label, stage)` returns the
generic transition System; hosts order that helper explicitly. Applying moves current to previous,
pending to current, and stamps the transition ChangeTick.

## Flush policy

`FlushMode` is compiled per Update system/stage:

- `Final`: defer until the mandatory final flush of the Update operation;
- `Stage`: flush after every stage pass (therefore after each fixed substep);
- `System::flush_after()`: an explicit additional boundary after that system.

`ScheduleBuilder::new()` defaults Update stages to `Final` for source-compatible explicit control.
`ScheduleBuilder::standard()` defaults its Update operation to `Stage` so Startup output is visible
to Fixed/Update and one fixed substep's structure is visible to the next. Hosts may choose
explicitly. Conditions and later systems only observe structural commands after the relevant flush.
Component value mutations, registered resource values, and directly sent events follow their own
immediate rules.

Every successful `update` ends with a final structural flush. A flush error identifies the
originating system/command and enters the same fault path as a fallible system. Render is
entity/component-topology-read-only: `world.commands()` and `QueryEffects::commands()` reject in
Render context with `WorldError::StructuralCommandsDuringRender`, direct structural methods already
reject while Running, and Render performs no structural flush. Existing component values,
resources, and declared events may still change.

## App update lifecycle

`App::update(delta_seconds) -> Result<(), AppError>` rejects negative, NaN, infinite, and
Duration-out-of-range delta before changing ticks or World. Zero is valid. For a valid call:

1. reject nested execution, a prior terminal fault, a mutation-poisoned World, or
   `AppError::PendingIdleCommands`; pending commands are a recoverable precondition error and may be resolved
   with public idle `World::flush` or `World::discard_commands`;
2. compute the capped fixed-step/debt plan in locals and preflight the next WorldTick plus the entire
   required FixedStep range;
3. commit `WorldTick` (first update is 1) and the accumulator plan;
4. run Startup once, marking it complete only after success;
5. run zero or more FixedUpdate substeps;
6. run Update once;
7. perform the final command flush;
8. invoke the optional post-flush observation callback;
9. clear every currently queued Update-owned frame event—including external-source input queued
   before this call—and compact eligible event storage;
10. leave World idle.

The standard schedule runs due FixedUpdate substeps before Update. Fixed accumulation uses
`core::time::Duration` after validating/converting the f32 outer delta. The default maximum is eight
substeps. When more are due, App runs eight, drops the excess whole steps, preserves the
sub-step remainder, and reports `FixedDebtDropped { steps }`; it never changes fixed delta
silently. Hosts may configure another non-zero cap, and pd-asteroids may select its historical cap
in its host preset.

Fixed update is disabled by default. `AppBuilder::fixed(FixedConfig::new(positive_duration)?)`
enables it; the config may override the non-zero substep cap and has no `Default`. Building any
FixedUpdate system without fixed configuration is a `BuildError`. Schedule owns the accumulator;
`world.fixed_step()` returns the current `FixedStep { index, delta }` only while that stage runs.

Each fixed substep increments `FixedStep` independently and passes the configured fixed delta to
its systems. World tick advances once for the outer update, not once per substep. WorldTick and
FixedStep use checked increment; the outer call preflights both before committing its clock/accumulator
plan, so exhaustion runs no system and terminally faults App without wrapping. ChangeTick,
allocator/cache generations, and event sequence numbers follow the deliberately distinct Phase 3/5
exhaustion taxonomy.

`App::render(delta_seconds)` validates delta and the same fault/poison/pending-command preconditions,
then runs every Render-owned stage without advancing `WorldTick` and without a structural flush. Its
`render_with` variant observes state after all Render systems, then clears every currently queued
Render-owned frame event, including external input queued immediately before Render. Update-owned
frame channels are untouched (and normally were cleared at the end of Update).

Frame channels declare their owner with `EventOptions::frame(StageOperation)`. A system may access
one only from that operation, with producer/consumer order compiled inside the operation. For data
that must survive or cross Update/Render, use retained/manual/bounded events or a host
`RenderSnapshot` resource. A retained cross-operation channel has retention semantics, not an
impossible cross-operation DAG edge.

### Stable observation boundary

```rust
pub fn update_with<R>(
    &mut self,
    delta_seconds: f32,
    observe: impl FnOnce(&World) -> R,
) -> Result<R, AppError>;
```

`render_with` has the same shape. The observer receives read-only stable end-of-operation World
state before that operation's frame events are cleared: post-flush for Update and post-system for
topology-read-only Render. Update observation is the foundation for `moirai::testkit` and the
Anapao-owned bridge. It is not a second execution hook and cannot enqueue commands or mutate the
captured state.

## Failure boundary

Execution is not falsely advertised as frame-transactional: component/resource mutations from
already completed systems and earlier flushes may be visible when a later system fails.

On a system or flush failure:

- the current uncommitted structural batch is discarded transactionally;
- all reserved entities in that batch are released safely;
- the World run guard is cleared;
- frame events are retained for diagnosis rather than cleared;
- `App` records a fault with stage, system, tick, and progress;
- subsequent update/render calls reject permanently for that App instance.

`AppError::PendingIdleCommands` is different: it is detected before execution, changes nothing, and does not
fault App. A ChangeTick allocation failure is more severe than an ordinary recoverable World error:
it mutation-poisons World, and the runner terminally faults App after the current system even if the
system caught the original result. Read-only inspection remains available.

This prevents accidental continuation from a partially executed frame. Inspection remains
available, but execution recovery means rebuilding or restoring a new App. Expected domain errors
must be represented in host resources/events rather than returned as fatal system errors. Moirai
never claims to rewind already committed World mutations.

Panics are outside the recoverable runtime contract and continue unwinding/aborting according to
the host profile. RAII execution guards still clear World's Running flag and poison App terminally
before unwinding leaves the call. Core remains memory-safe, but hosts use fallible systems for
expected failures.

## Safe runner implementation

The compiled schedule stores dense immutable order separately from mutable system closures. Safe
field splitting lets the runner iterate cached ids and borrow exactly one system closure at a time.
Because systems receive only World, they cannot mutate the schedule being traversed.

No raw pointer is needed for:

- embedded World/Schedule self-borrowing;
- system-vector iteration;
- conditions;
- fixed-step execution;
- profiling.

The crate remains under `#![forbid(unsafe_code)]`.

## Diagnostics

Moirai exposes one platform-neutral downstream method:

```rust
pub trait Observer {
    fn observe(&mut self, event: DiagnosticEvent<'_>);
}
```

The non-exhaustive event reports update/render, stage/system start/finish, flush completion,
fixed-debt drop, and fault boundaries with opaque ids and World/Fixed ticks. The synchronous
observer may sample a host clock itself; Moirai never calls Playdate FFI or assumes `std::time`.
It receives no World/Schedule mutation access and cannot veto execution.

App owns an optional boxed observer configured by AppBuilder. With no observer, each emission site
is one predictable option branch with no allocation or clock call. A detailed observer owns and
preallocates its own trace buffer.

## Tasks

- [x] **T401** Implement StageOperation-owned opaque stage/system/set ids and private-field `System`
  builders.
- [x] **T402** Implement ExecutionLease, ScheduleBuilder resolution, graph validation, required
  locks, and deterministic order.
- [x] **T403** Implement safe condition and system runners with no raw pointers.
- [x] **T404** Implement generic `State<S>` and explicitly installed transition boundaries.
- [x] **T405** Implement Update-only FlushMode/final-flush semantics and Render structural rejection.
- [x] **T406** Implement FixedConfig/accumulator/substep/debt diagnostics and trace tests.
- [x] **T407** Implement `AppBuilder` finalization, `App::from_parts`, ownership, and accessors.
- [x] **T408** Implement update/render/observe, pending-command prechecks, and operation-owned frame
  event lifetime ordering.
- [x] **T409** Implement terminal fault and unwind-poison semantics.
- [x] **T410** Replace source runtime-cycle panics with build-error characterization tests.
- [x] **T411** Add the one-method diagnostic observer/event surface and absent-observer allocation
  proof.
- [x] **T412** Port/classify the schedule/state/fixed-step source test inventory.

## Verification

```sh
cargo test --no-default-features
cargo test --features std
cargo test --features testkit
cargo clippy --all-targets --all-features -- -D warnings
cargo bench --bench schedule
cargo doc --no-deps --all-features
```

Required trace tests cover deterministic ties, set expansion, cycle diagnostics, cross-operation
edge rejection, skipped conditions, every Update flush mode, Render structural rejection, Startup
once, fixed catch-up, pending idle commands, fault cleanup, operation-owned prequeued frame events,
cross-call accumulation/other-operation isolation, post-flush observation, and render tick behavior.

## Risks and controls

| Risk | Control |
| --- | --- |
| Embedded schedule recreates aliasing | App owns sibling fields; forbidden `World → Schedule` edge |
| Public fields freeze runtime bookkeeping | private builders and opaque ids |
| Cycle fails during play | complete build-time graph validation |
| Custom stage runs from the wrong App method | immutable StageOperation and local DAGs |
| Prequeued host input survives or clears at the wrong boundary | operation-owned frame channels |
| Fallible system silently continues partial frame | sticky fault boundary and explicit recovery |
| Observer sees unstable or already-cleared state | one tested end-of-operation/pre-clear hook |
| Generic schedule accumulates host policy | only Startup/FixedUpdate/Update/Render in standard preset |

## Exit criteria

- [x] App executes World through an external compiled Schedule using safe Rust only.
- [x] Every invalid topology/configuration fails before first update.
- [x] Update, render, fixed-step, flush, event-clear, and observer ordering are trace-tested.
- [x] Generic `State<S>` works without importing a host enum.
- [x] Errors leave World idle and App explicitly faulted.
- [x] No platform clock/FFI or host stage graph exists in Moirai.
- [x] Phase 5 queries can execute inside systems without changing schedule ownership.

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Bevy Schedule API](https://docs.rs/bevy/latest/bevy/ecs/schedule/struct.Schedule.html)
- [Rustonomicon: working with unsafe](https://doc.rust-lang.org/nomicon/working-with-unsafe.html)
