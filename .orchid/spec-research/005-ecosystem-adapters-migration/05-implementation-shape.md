# Implementation Shape

Repository-ordered work:

1. **Wyrd:** add RuntimeState/fingerprint/atomic restore and continuation/corruption tests.
2. **Wyrd:** add the Moirai adapter, atomic driver, validated last/next tick envelope, domain
   conversions, and numeric matrices.
3. **Moirai prerequisite:** consume the already-green Phase 6 `testkit`; do not change its
   dependency-neutral contract for an adapter.
4. **Anapao:** expose supported report/batch construction if missing, then add the Moirai bridge.
5. **Sea of Grass:** port operation-owned host stages/resources/entities/queries in coherent groups;
   mount/restore Wyrd; pass `docs/wyrd-parity.md` behavior plus persistence gates; then remove
   wiring/Bevy ECS.
6. **pd-asteroids:** use path dep/host shim, reconcile classified parity and dual traces, then remove
   local ECS.
7. Run the canonical ten-step seeded door replay and step-five save/rebind/restore proof across all
   three libraries.

Each repository records its own commands/commits/artifacts. No single workspace command is accepted
as proof for the distributed migration.
