# Current State

- Wyrd's Bevy adapter separates systems because Bevy queries are system parameters; Moirai can
  safely use an atomic driver with World/resource scope.
- Wyrd Runtime privately stores held senses, prior inputs/decrements, counters, flags, timers,
  delays, OnStart, RNG, tick, and ephemeral outbox state, but exposes no complete restore seam.
- Sea of Grass saves latch/delay/step wiring continuation and orders settle/apply before portal
  travel.
- The sixteen Sea of Grass tests prove validation/immediate behavior but not save continuation.
- Anapao's public Simulator accepts a CompiledScenario, while arbitrary Moirai World snapshots are
  richer than Anapao NodeId-to-f64 NodeSnapshot.
- pd-asteroids owns game state/stages/platform profiling that must remain downstream during its ECS
  cutover.
- Phase 6 owns the neutral testkit; `PHASE_7_INTEGRATIONS.md` records exact adapter ownership,
  behavior/restore tests, its use in the canonical three-library proof, and deletion gates.
