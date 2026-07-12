# Design Decisions

- No `moirai-wyrd` or `moirai-anapao` member in this repository.
- Wyrd owns `WyrdDriver<P, B>` and `WyrdBinding`.
- Default driver step is `begin_frame → sample → loom → apply`; any phase error leaves the tick
  unchanged and sticky-faults driver plus App because sampling also has mutable binding/World access.
- SettleTick advances only after completed Apply.
- Wyrd RuntimeState is versioned, topology/layout-bound, numeric-tagged, complete, and atomically
  restored; outbox is excluded.
- WyrdDriverState stores RuntimeState plus last-completed and next SettleTick. Fresh and post-Apply
  cross-field relations validate atomically; mid-step/faulted state is not restorable.
- Sea of Grass deletion requires all 16 behavior cases plus uninterrupted/save-rebind-restore
  continuation as source-audited in `docs/wyrd-parity.md`.
- Moirai testkit uses exact `S: Eq` host snapshots and scalar metrics at post-flush/pre-clear.
- Anapao bridge builds supported reports and reuses its existing evaluators/artifacts; it does not
  fake ScenarioSpec or NodeSnapshot.
- Host cutovers use path dependencies, coherent domain groups, dual traces, and explicit deletion
  gates.
