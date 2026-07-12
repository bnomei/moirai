# Requirements — Moirai single-crate scaffold and quality baseline

R001: WHEN Moirai builds without default features THE library SHALL use Rust 1.75, `#![no_std]`,
  unconditional `alloc`, and `#![forbid(unsafe_code)]`.
R002: WHEN `std`, `testkit`, or all features build THE feature set SHALL be additive and coherent,
  with exactly `default = []`, `std = []`, and `testkit = []` at this boundary.
R003: WHEN Phase 1 rustdoc is generated THE crate SHALL expose no accidental semantic namespace,
  root re-export, prelude, or extension trait; each SHALL first publish with its owning real surface.
R004: WHEN Phase 1 documentation/tests describe future public API THEY SHALL use the frozen final
  path contract without declaring a constructor, enum variant, trait method, panic, no-op, or pretend
  runtime behavior before its owning phase implements the invariant.
R005: WHEN downstream boundary tests compile THEY SHALL prove README truthfulness and representative
  implementation-private paths fail without a fake App/System authoring example.
R006: WHEN CI runs IT SHALL separate current-stable format/lint/test/warnings-denied-doc/benchmark
  checks from the Rust 1.75 library-only no-default-features check.
R007: WHEN dependencies and rustdoc are inspected THE core SHALL have no Wyrd, Anapao, Bevy,
  Playdate, serde, or proc-macro dependency and no implementation-container namespace.
