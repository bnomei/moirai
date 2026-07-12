# Research Questions

## Resolved

- Moirai remains dependency-pure; adapters live with Wyrd and Anapao.
- Wyrd's default Moirai driver is one atomic sample/loom/apply system.
- Each driver owns an independent SettleTick; it is not WorldTick or FixedStep.
- Sea of Grass cannot delete wiring after behavior parity alone; Wyrd must snapshot/restore complete
  continuation state.
- Anapao Simulator runs only CompiledScenario and cannot be used as an arbitrary ECS backend.
- Moirai testkit owns exact host-defined snapshots and scalar metrics.
- Anapao owns conversion into its reports/assertions/events/artifacts.
- Host stages, component bindings, save schema, WorldMap policy, and platform adapters remain host
  code.

## External/upstream blockers

- Wyrd currently lacks a public complete RuntimeState snapshot/restore contract.
- Anapao may need a supported public RunReport/BatchReport builder and batch aggregation seam.
