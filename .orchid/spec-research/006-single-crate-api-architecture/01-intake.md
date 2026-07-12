# Intake

## Raw request

Think deeply about Moirai as a single-crate project: shape its internal submodules and top-level API
conveniences using Wyrd and Anapao as local precedents; define how all three cooperate; validate
Rust patterns externally; and use this pre-implementation window to correct every phase cheaply.

## Success signals

- One published Moirai crate has a concrete module tree and dependency direction.
- Root exports, semantic namespaces, and the prelude have explicit membership policies.
- The executable lifecycle does not reproduce pd-asteroids unsafe aliasing.
- Feature flags reflect Cargo feature unification and real domain choices.
- Wyrd tick/persistence and Anapao external-driver boundaries are truthful.
- Phases 0–7 can be revised without workers re-deciding architecture.

## Constraints

- `no_std + alloc`, Rust 1.75 core, Playdate-class consumer.
- One Moirai crate through 1.0; no proc-macro companion or microcrate split.
- Preserve proven behavior where it is desirable, but classify source quirks instead of canonizing
  them as public API.
- No implementation work in this research packet.

## Non-goals

- Designing game components, rendering, physics, persistence formats, or Anapao's internal engine.
- Making Moirai imitate Bevy APIs or providing derive macros.
