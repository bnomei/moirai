# Make Specs Handoff: 001-scaffold-quality-baseline

## Status

- research_id: 001-scaffold-quality-baseline
- status: frozen
- intended_spec_slug: scaffold-quality-baseline
- shape_review: GREEN
- cheap_worker_ready: yes

## Objective

Create Moirai's permanent one-crate build, visibility, API-test, and CI envelope without exposing
temporary ECS internals.

## Requirements Seed

- R001: WHEN the crate builds without default features IT SHALL use `no_std + alloc` on Rust 1.75.
- R002: WHEN all features are enabled THE BUILD SHALL remain coherent and additive.
- R003: WHEN rustdoc is generated IT SHALL expose semantic modules, not implementation containers.
- R004: WHEN root/prelude paths drift DOWNSTREAM-STYLE COMPILE TESTS SHALL fail.
- R005: WHEN stable quality runs IT SHALL test core, std, testkit, docs, lint, and all features.
- R006: WHEN MSRV runs IT SHALL check the library independently from newer developer tools.

## Decisions

- One package through 1.0.
- Features exactly `default=[]`, `std=[]`, `testkit=[]`.
- Exact final public shape comes from `docs/ARCHITECTURE.md` and `PHASE_1_SCAFFOLD.md`. Phase 1
  publishes no empty namespace, root type, prelude, or extension trait; each owner adds its real
  surface and tests, while `testkit` begins in Phase 6.
- No proc macro, numeric feature, adapter dependency, unsafe code, or public empty implementation
  module.

## Suggested Task Slices

- T001: manifest, attributes, and feature contract.
- T002: semantic facade/private module layout and crate docs.
- T003: root, namespace, prelude, README, and visibility tests.
- T004: CI/toolchain matrix and documentation scaffolding.
- T005: package/rustdoc review.

## Validation

- Run every command in `PHASE_1_SCAFFOLD.md#verification`, including warnings-denied rustdoc and
  benchmark-harness compilation.
- Inspect generated rustdoc module index.
- Confirm `cargo check --all-features` has no exclusive feature error.
- Confirm Wyrd, Anapao, Bevy, Playdate, serde, and proc-macro dependencies are absent.

## Open

- Dev-dependency/action revisions selected during spec implementation with MSRV evidence.

## Worker Context Policy

Workers read `docs/ARCHITECTURE.md`, `PHASE_1_SCAFFOLD.md`, and this handoff. They do not infer
public visibility from the pd-asteroids source tree.
