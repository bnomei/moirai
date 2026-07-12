# Design Decisions

- World owns data/registries/resources/events/commands/tick/run guard only.
- Schedule owns systems/conditions/order/fixed accumulator only.
- AppBuilder validates both; App owns both as siblings.
- WorldBuilder/ScheduleBuilder/App::from_parts is the advanced construction path; Schedule run is
  crate-private and an Rc/Weak ExecutionLease validates attachment.
- Table/archetype moves and structural command batches preflight before commit.
- Resources are typed; named resources are rejected. Typed events are default; dynamic named events
  remain advanced.
- Required resource types receive Weak lease locks; registered optional values remain immediately
  insertable/removable. EventReader uses Rc cursors/Weak queue tracking; runtime gates are rejected.
- Every stage declares StageOperation::Update or Render; App methods execute separate operation-local
  DAGs and cross-operation stage edges are invalid.
- Frame event channels declare the same operation owner. A successful operation clears all queued
  events on its channels after observation, including prequeued external-source input.
- `World::resource_scope` marks one resource type scoped and rejects same-type access.
- `System` has a required body, private fields, infallible/fallible constructors, and opaque ids.
- ScheduleBuilder compiles dense order and reports readable cycle paths before execution.
- `State<S>` is generic and transitions at an explicitly installed boundary.
- State has no push/pop stack, and conflicting pending requests return an error.
- The standard fixed accumulator uses Duration, runs before Update, defaults to eight substeps, and
  reports/drops excess whole-step debt while retaining the remainder.
- Fixed is disabled by default and required explicitly when its stage has systems.
- System pipes and interval buffers are omitted; use normal Rust composition, fixed steps, or host
  timers.
- Update performs final flush, read-only observation, then Update-frame clear. Custom Update stages
  default to final-only structural flush; standard Update stages flush each stage/fixed substep,
  with explicit per-system flush available.
- Render is topology-read-only, performs no structural flush, does not advance WorldTick, observes
  after its systems, then clears Render-frame channels.
- App rejects pending idle Commands before update/render until explicit World flush/discard.
- WorldTick/FixedStep exhaustion faults App; ChangeTick poison, entity/cache retirement, and
  channel-local sequence closure remain distinct policies.
- Fallible partial execution records a sticky App fault; it does not claim rollback of prior
  component/resource mutation.
- Safe field splitting replaces every source raw-pointer runner.
- Diagnostics use one synchronous `Observer::observe` method over a non-exhaustive event enum;
  platform clocks remain downstream.
