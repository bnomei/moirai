# Intake

Prepare the implementation handoff for Moirai's permanent single-crate envelope: Rust 1.75,
unconditional `no_std + alloc`, additive features, semantic public modules, curated root/prelude,
downstream-style API tests, CI, and the benchmark/coverage scaffolding used by later phases.

In scope: Phase 1 only. The package must make later internals private from their first commit.

Out of scope: ECS behavior, public placeholder/no-op implementations, crate splitting, proc macros,
numeric selection, Wyrd/Anapao dependencies, and performance claims.

Human acceptance of Phase 0 D1–D16 remains the execution gate.

