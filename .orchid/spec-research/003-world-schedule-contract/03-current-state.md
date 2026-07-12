# Current State

- pd-asteroids stores Schedule inside World and uses raw pointers during update; safe callbacks can
  reach mutable schedule state, creating an unsafe public path.
- Its scheduler also raw-pointers mutable system storage while using cached order.
- Several World structural methods always defer even outside systems, conflicting with the
  previously stated “deferred during systems” contract.
- Runtime order-cycle panic, game-specific `GameState`, magic stage policy, and Playdate profiler
  FFI are not engine-neutral behavior.
- Source events are cleared before an external caller can observe the final event-bearing World.
- Sea of Grass requires apply-before-portal ordering but that graph is host policy.
- `PHASE_3_WORLD_LIFECYCLE.md` and `PHASE_4_SCHEDULE.md` now define the corrected split and error
  boundaries.

