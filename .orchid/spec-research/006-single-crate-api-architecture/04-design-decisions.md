# Design Decisions

## Locked recommendations

1. **One Moirai crate through 1.0.** No later split review and no Moirai adapter crates.
2. **App owns World and Schedule as siblings.** World never owns/imports Schedule; systems retain
   migration-friendly mutable World access without unsafe aliasing.
3. **Private machinery, semantic public modules.** Storage, allocators, registries, command ops,
   raw queues, and query plans remain crate-private.
4. **Curated root plus small prelude.** Optional integrations never enter either.
5. **Checked setup boundary.** AppBuilder/ScheduleBuilder return contextual errors; cycles and
   missing contracts cannot reach runtime.
6. **No global numeric features.** Q16 is always available and conventional; f32/i32/Q16 coexist.
7. **Generic state.** Host game enums remain host-owned.
8. **Typed resources/events by default.** Named resources are not ported; named events remain only
   for authored/dynamic compatibility.
9. **Immediate setup, explicit Update deferral.** World structural mutation is immediate outside
   execution and direct methods reject during execution; Update systems use `world.commands()` and
   Render rejects Commands.
10. **No unsafe Moirai code.** Platform clocks/logging use downstream observer implementations.
11. **Adapters live with their semantic owner.** Wyrd owns its Moirai driver; Anapao owns its
    external-run/report bridge. Moirai supplies stable seams and an optional neutral testkit.
12. **Independent tick domains.** WorldTick, FixedStep, Wyrd SettleTick, and Anapao StepIndex are
    mapped explicitly, never equated.
13. **Wyrd persistence blocks SoG deletion.** Sixteen behavior cases are necessary but insufficient
    until versioned runtime snapshot/restore and continuation tests exist.
14. **The 151 tests are classified evidence.** Preserve desirable behavior, adapt host/API quirks,
    and reject unsound or misleading contracts.
15. **Single-threaded 1.0 contract.** Moirai does not force all user values/closures to be
    `Send + Sync`; downstream batch adapters may add stronger factory bounds.
16. **Owner-scoped configuration handles.** Component/event/query/schedule handles use private Rc
    owner tokens; compact Copy EntityId stays explicitly World-relative.
17. **Two clocks for data visibility.** WorldTick marks outer updates; checked ChangeTick marks
    component/resource mutations and query windows.
18. **One lifecycle entry.** WorldBuilder/ScheduleBuilder can construct parts, but only App executes
    Schedule; an Rc/Weak lease validates attachment and required-resource locks.
19. **Operation-owned stages and frame events.** Every custom stage and frame channel declares
    Update or Render. Operation-local DAGs define execution; matching App calls observe then clear
    all queued frame events, including prequeued external input.
20. **Update owns topology.** Update owns Commands/flush; Render can mutate existing values,
    resources, and declared events but is entity/component-topology-read-only.
21. **No implicit idle batch adoption.** App rejects pending idle Commands until explicit World
    flush/discard.
22. **Exact temporal progress.** Change queries use `(since, captured_now]` and commit cursor progress
    only after full exhaustion/success.
23. **Typed exhaustion outcomes.** App clocks fault App, ChangeTick poisons mutation, entity/cache
    slots retire locally, and event sequence closes only its channel.
24. **Cross-field Wyrd restore.** RuntimeState, last-completed tick, and next tick validate initial or
    post-Apply relationships atomically before host deletion.

## Rejected paths

- Multiple Moirai core/world/schedule/adapter crates: change coupling does not justify release and
  dependency overhead.
- World owning Schedule with `mem::take`: callbacks see placeholder state and unwind semantics are
  poor; it does not restore a coherent ownership model.
- Copying the Wyrd Bevy three-system adapter: Moirai can run one atomic resource-scoped driver and
  should not permit a gate/failure between loom and apply.
- Copying Wyrd Signal as Q16: count and level domains have incompatible representation semantics.
- Using Anapao ScenarioSpec as an ECS wrapper: Simulator cannot execute external subjects.
- Public derive/query macro: requires another crate and freezes syntax before the typed API settles.

## Remaining approval

The architecture is internally complete. Phase 0 human sign-off still decides whether these
recommendations become implementation locks.
