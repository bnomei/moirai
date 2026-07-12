# Phase 3 — World data and structural lifecycle

**Status:** complete · 2026-07-12
**Depends on:** [Phase 2](./PHASE_2_CORE_STORAGE.md)
**Research:** [packet 003](./.orchid/spec-research/003-world-schedule-contract/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)

## Goal

Complete `World` as the owner of ECS data—entities, component storage, resources, events, command
buffers, ticks, and lifecycle state—without giving it a schedule or system registry.

This phase fixes exactly when structural mutation is visible and makes archetype moves, deferred
commands, resource scopes, event retention, and component lifecycle events safe and deterministic.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R301 | WHEN World is built IT SHALL own ECS data but no Schedule/system registry/platform clock. |
| R302 | WHEN idle entity/component topology mutates THE CHANGE SHALL be immediate. |
| R303 | WHEN running topology mutation is requested DIRECT WORLD METHODS SHALL reject; Update SHALL use Commands and Render SHALL reject Commands. |
| R304 | WHEN a command batch is invalid NO structural operation in that batch SHALL commit. |
| R305 | WHEN an archetype move commits RETAINED values/ticks and every swapped location SHALL remain correct. |
| R306 | WHEN a host Bundle writes values IT SHALL use checked BundleWriter operations without storage access. |
| R307 | WHEN a required resource is locked ITS value MAY be replaced but removal SHALL return ResourceInUse. |
| R308 | WHEN registered optional resources/events change LATER SYSTEMS SHALL observe their immediate values/revisions. |
| R309 | WHEN EventReader values live/die WEAK CURSOR TRACKING SHALL preserve compaction and release dropped claims. |
| R310 | WHEN event retention drops unread history THE READER SHALL report exact lag rather than ambiguous absence. |
| R311 | WHEN public flush/discard is called during execution IT SHALL reject; only compiled Update boundaries may flush. |
| R312 | WHEN frame events register THEIR Update/Render owner SHALL govern clearing, including prequeued external input. |
| R313 | WHEN ChangeTick/event sequence/allocator generation exhaust EACH SHALL follow its distinct non-wrapping policy. |
| R314 | WHEN Phase 3 exits table/sparse/tag, commands, resources, events, and ChangeTick SHALL be state-model verified without unsafe code. |

## World ownership

Conceptually, `World` owns:

```text
World
├── entity allocator (Free / Reserved / Live / Retired)
├── frozen component registry
├── sparse stores
├── archetype signatures, table columns, and entity locations
├── typed resource store
├── typed and dynamic event channels
├── reusable structural command queue
├── WorldTick + ChangeTick
└── lifecycle guard (Idle / Running)
```

It does **not** own:

- `Schedule` or executable systems;
- stage/order graphs or fixed-step accumulation;
- platform clocks, Playdate FFI, or logging;
- host `GameState` or schedule presets;
- Wyrd/Anapao adapters.

The component schema is finalized by `WorldBuilder::build`, directly or inside
`AppBuilder::build`. Runtime APIs cannot register a new typed layout behind an existing component
id.

## Entity lifecycle and deferred spawn

Phase 2's allocator is extended to distinguish reserved from live:

- `Free`: available for reservation;
- `Reserved`: id returned by `Commands::spawn` but not query-visible;
- `Live`: committed at flush and present in storage;
- `Retired`: generation cannot advance safely.

`Commands::spawn(bundle)` reserves an id immediately so later commands in the same batch can target
it. `World::is_alive` remains false until the batch commits. World access to a reserved id returns a
specific pending/not-live error; command APIs may accept it after checking reservation ownership.

If structural-batch validation fails, every reservation created by that batch is released with the
correct generation transition. A reserved id can never alias an existing or future entity.

Idle `World::spawn` commits immediately and returns a live `EntityId`.

## Storage and archetype model

Sparse components remain in private sparse stores. Table components are grouped by an internal
archetype signature; tags contribute to signature/membership without allocating a data column.

Private data structures include:

- canonical sorted archetype signatures;
- component-id-to-column maps;
- type-erased table columns;
- row-to-entity and entity-to-location indices;
- reusable move plans/buffers.

Adding or removing table/tag components moves an entity exactly once:

1. validate the entity, component id/type, and destination signature;
2. resolve or create the destination archetype;
3. preflight all source/destination columns and bundle values;
4. move retained values and insert replacements;
5. repair swap-removed source locations;
6. update the moved entity's location;
7. emit lifecycle events only after commitment.

A failed preflight leaves storage and locations unchanged. Application after successful preflight
is treated as an internal invariant path and is covered exhaustively without unsafe code.

Retained components carry their original added/changed ChangeTicks across an archetype move. A new
component records the structural batch tick as both added and changed; replacement retains added
and updates changed; remove-then-later-add creates a new added tick.

Required tests cover empty/table/tag/sparse/mixed bundles, repeated add/remove, replacement without
an unnecessary move, despawn, swap-remove repair, and multi-entity moves across every relevant
signature transition.

## Public bundle conveniences

`moirai::world::Bundle` is a public safe extension trait. Its consuming method receives a
`BundleWriter` whose only public operation is checked typed insertion. Moirai supplies tuple
implementations through arity 16. This enables the common path:

```rust
let ship = world.spawn((Position::default(), Velocity::default(), Player))?;
```

Host bundle structs may implement `Bundle` through `BundleWriter`; they never manipulate columns,
ids, or erased storage directly. This is the manual, one-crate replacement for a derive macro.

`DynamicBundle` is the conditional/authored path. It stores resolved component ids and owned
values through a safe erased boundary. Duplicate components, unregistered types, tag-with-value,
and missing table values fail before structural mutation.

No derive macro is introduced. Tuple bundles and `DynamicBundle` are the complete 1.0 convenience
surface.

## Immediate versus deferred mutation

The contract is state-based and observable:

| Context | entity/component `spawn/insert/remove/despawn` | `world.commands()` |
| --- | --- | --- |
| Idle setup/editor code | immediate `Result` | explicit deferred batch, manually flushed |
| Update-owned stage | `StructuralMutationDuringRun` | required deferred path |
| Render-owned stage | `StructuralMutationDuringRun` | `StructuralCommandsDuringRender` |

Component value mutation through an already-borrowed typed value is not structural and remains
immediate. Adding/removing a component, creating/despawning an entity, or changing table
membership is structural. Registered resource values and event sends follow the separate immediate
rules below.

`Commands<'_>` is a borrowed facade. The private queue records only validated, resolved
operations plus diagnostic origin. `CommandOp` is never public. It includes real spawn semantics;
the source's Spawn no-op and `RunSystem` command are rejected.

An idle host may resolve a deferred batch only through `World::flush` or
`World::discard_commands`. Discard clears the whole batch and releases every Reserved entity using
the allocator's ordinary stale-id-safe release/retirement rules. App never guesses whether an idle
batch was intended for the next Update: `App::from_parts`, `App::update`, and `App::render` reject
the dirty queue before execution (`BuildError::PendingCommands` at attachment,
`AppError::PendingIdleCommands` afterward) until the host flushes or discards it.

### Flush transaction

`World::flush`:

1. preflights the complete queued sequence against a shadow lifecycle/signature state;
2. detects conflicts such as double despawn or use after queued despawn;
3. resolves lifecycle-event effects and destination archetypes;
4. reserves one checked ChangeTick for the atomic batch and commits in command order;
5. clears/reuses buffers and returns a `FlushReport`.

Logical validation failure applies none of that structural batch and releases its reservations.
Already committed changes from earlier flushes are not rolled back. The error includes command
index and, when scheduled later, originating system id.

ChangeTick exhaustion is detected during preflight, so it cannot leave a half-stamped batch. The
batch is discarded, its reservations are released, and World becomes persistently
mutation-poisoned. Read-only inspection remains legal, but all future mutation and App execution
reject; this remains true even if a caller catches the first error.

Public `World::flush` and `World::discard_commands` are idle-only. During execution, a system cannot
defeat Schedule's `FlushMode` by flushing directly; Schedule uses a crate-private run capability to
invoke the same preflight/commit machinery at compiled Update boundaries. Render operation context
rejects `world.commands()` and `QueryEffects::commands()` with
`WorldError::StructuralCommandsDuringRender` before anything is queued.

Flush behavior is deterministic and allocation-free in steady state after representative capacity
warmup, subject to a documented maximum topology/bundle shape.

## Typed resources

Resources are keyed by Rust `TypeId`. Multiple logical instances use host newtypes. The unused
source named-resource API is not ported.

Resource types are registered/frozen by WorldBuilder, but an optional registered type may initially
have no value. The common value surface includes:

- immediate `insert_resource<R>` / `remove_resource<R>` in idle or sequential system code;
- `resource<R>` / `resource_mut<R>` with contextual absence errors;
- `contains_resource<R>`;
- `resource_scope<R>` for a callback that needs one resource and the rest of the world.

World owns a monotonic checked `ChangeTick` distinct from `WorldTick`. Each resource and component
value stores added/changed ticks. Conditions compare ChangeTick windows, so changes between fixed
substeps sharing a WorldTick remain visible. Mutable access conservatively advances the clock and
stamps the value before returning `&mut T`.

Every API that needs a new ChangeTick preflights it before exposing a mutable reference or applying
state. A failed allocation performs no requested mutation and sets the same terminal World poison
used by structural flush. The runner checks the poison after every system/boundary, so a system
cannot catch the error and let App continue with frozen change metadata.

When `ScheduleBuilder::build(&mut World)` resolves `System::requires_resource::<R>`, it
transactionally locks that resource type in World. Replacement remains legal; removal returns
`ResourceInUse` and names the requiring systems. Optional resource conditions do not lock a type.
This preserves the checked-build guarantee even though `App::world_mut` remains useful while idle.

Runtime insert/remove is safe and immediate because a system has exclusive World access and
resource borrows cannot outlive the call that produces them. Insertion of an unregistered resource
type is an error; hosts declare optional dynamic presence during setup. A later condition/system in
the same schedule observes the new value/revision immediately.

`resource_scope` temporarily marks the type as scoped and moves the value out of the store. During
the callback, attempts to access/replace the same type fail instead of aliasing. On normal return,
the original slot is restored and successful mutable scope access advances its revision. Moirai
guarantees memory safety, but—as with the rest of the
`no_std` runtime—does not promise recovery from an unwinding panic inside callbacks; fallible work
returns `Result` instead.

The scope API is specifically stable enough for a Wyrd-owned driver or host controller to mutate
its state while sampling/applying the World, without unsafe code.

## Events

Typed events are the default:

```rust
builder.add_event::<Damage>(EventOptions::frame(StageOperation::Update))?;
world.send(Damage { amount: 3 })?;
let reader = world.event_reader::<Damage>()?;
```

The private channel registry owns payload queues. Public `EventReader<E>` handles retain an owner
token, event id, and `Rc` cursor cell; each queue tracks only `Weak` cursor cells. Dropping a reader
therefore releases its compaction claim without a callback into World. Readers are not implicitly
cloned; `fork(&mut World)` registers an explicit independent cursor at the same sequence. Public policy
includes:

- frame retention;
- explicit/manual retention;
- bounded/count retention where required by source parity;
- deterministic send order;
- multiple readers with independent progress;
- explicit handling when a reader falls behind retained history.

Reader construction chooses `oldest_retained` or `from_now` explicitly. There is no magic shared
reader id. Falling behind returns `EventReadError::Lagged { dropped }` and advances to the oldest
remaining sequence under documented recovery semantics.

Channel head/next sequence numbers are checked `u64` values and do not reset when storage compacts.
Persistent compaction removes events consumed by every live Weak-tracked reader, and removes all
when no readers exist. Frame/bounded retention may advance past a live reader and therefore reports
lag on its next read. Sequence exhaustion atomically closes only that channel to future sends;
retained events remain readable, other channels and World remain usable, and no sequence wraps. A
scheduled send faults App only if its error is propagated as the system's fatal result.

Registered event sends always enqueue (or return a capacity/error result); there is no public
runtime enable/disable gate and no silent “disabled” drop. System emits/consumes declarations are
compiled into the current run-access descriptor. During a system, sending or reading a registered
but undeclared event returns `UndeclaredEventAccess`; outside schedule execution, host code may
send/read directly. The descriptor is temporary run context, not a World-owned system registry.

`EventOptions::frame(StageOperation)` makes a frame channel's Update/Render owner mandatory.
`external_source()` is an orthogonal builder flag for a producer that is host input/test code rather
than a scheduled system. It is the explicit escape from “consumer must have a producer” validation.
Persistent/manual/bounded channels carry no clear-operation owner and may bridge operations.

Named/dynamic event channels remain an advanced authored-data facility in `moirai::event` and are
not in the root/prelude happy path. Named resources do not accompany them.

Component lifecycle channels use component ids internally and typed helpers publicly. Callers never
construct magic strings such as `OnAdd:<name>`.

Frame events are cleared only by their Phase 4 App operation lifecycle. Update clears all currently
queued Update-owned frame events after its final structural flush and `update_with` observation;
Render clears all currently queued Render-owned frame events after Render and `render_with`.
“Currently queued” includes external-source events sent immediately before the matching call. The
other operation's channels are untouched. `World` exposes clearing crate-privately; it does not
guess when a host operation ends. Component structural lifecycle channels are Update-owned because
Render cannot change topology.

Events sent between matching operations are input to the next matching operation and preserve send
order. If that App operation is never called, they remain queued; bounded capacity or sequence
closure reports an explicit send error rather than silently dropping or letting the other operation
clear them.

## World tick and run guard

`WorldTick` is stored with World for queries, diagnostics, and adapters. Only `App` advances it.
The first executed update observes tick 1. Render calls and manual setup mutation do not increment
it.

Crate-private `begin_run`/`end_run` transitions protect immediate structural APIs. The guard is
cleared on every normal/fallible Schedule exit. Nested execution against the same World is rejected.

## Tasks

- [x] **T301** Add Reserved allocator state and transactional reservation release.
- [x] **T302** Implement private table columns, archetype signatures, locations, and checked move
  plans.
- [x] **T303** Complete idle spawn/insert/remove/despawn across sparse/table/tag storage.
- [x] **T304** Implement tuple/custom `Bundle` support and final `DynamicBundle` validation.
- [x] **T305** Implement borrowed `Commands`, idle discard/pending detection, and whole-batch
  preflight/commit.
- [x] **T306** Implement typed resources, ChangeTick, locks, and the safe scoped-resource contract.
- [x] **T307** Implement typed event registration, operation-owned frame queues, weak-tracked
  readers, retention, closed-channel exhaustion, and lifecycle channels.
- [x] **T308** Add World run guard, crate-private tick advancement, and event-clear boundary.
- [x] **T309** Port/classify all non-query world/source tests owned by this phase.
- [x] **T310** Add state-machine tests spanning allocator, locations, commands, and lifecycle
  events.
- [x] **T311** Add steady-state allocation and archetype-move benchmarks with owning code.

## Verification

```sh
cargo test --no-default-features
cargo test --features std
cargo clippy --all-targets --all-features -- -D warnings
cargo bench --bench world_lifecycle
cargo doc --no-deps --all-features
```

Invariant tests must compare World state before and after rejected command batches.

## Risks and controls

| Risk | Control |
| --- | --- |
| Deferred spawn id becomes observable too early | Reserved state; queries and World access require Live |
| Archetype move partially commits | full preflight, then invariant-only commit |
| Commands fail after half a flush | shadow-state batch validation |
| Idle commands leak into the next App call | reject pending queue; require explicit flush/discard |
| Resource scope aliases the same type | scoped sentinel and same-type access rejection |
| Frame events clear before testing/adapter capture | clearing belongs to App after observer |
| World grows schedule knowledge | forbidden dependency test/review boundary |

## Exit criteria

- [x] World data lifecycle is complete without Schedule ownership.
- [x] Immediate/deferred mutation timing is externally testable and documented.
- [x] Sparse/table/tag moves preserve all locations and values under randomized traces.
- [x] Resource/event surfaces are typed by default and implementation containers remain private.
- [x] Command failures never leave a partially applied structural batch.
- [x] The World is ready for a safe external Schedule runner and the full query facade.

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Phase 2](./PHASE_2_CORE_STORAGE.md)
- [Phase 4](./PHASE_4_SCHEDULE.md)
