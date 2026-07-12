# Current State

- pd-asteroids' allocator checks generation but does not independently prove occupancy; a
  freed-but-not-reallocated current-generation slot can be misclassified if raw construction is
  available.
- Component registration can silently reuse a name/type without proving storage/layout options
  agree.
- Source sparse storage supplies useful dense iteration and swap-remove behavior, but the container
  should not be public in Moirai.
- Wyrd's i32 signal path stores Count as raw integer bits and Level as Q16 bits while multiplication
  performs fixed-point scaling. That is a graph-domain contract, not a universal Q16 type.
- Cargo feature unification makes global mutually exclusive numeric modes unsuitable for an ECS
  that naturally stores multiple numeric component types.
- `PHASE_2_CORE_STORAGE.md` now records the corrected state models, transaction rules, numeric
  operations, tests, and vertical slice.
