# Tavily Rust API research provenance

Date: 2026-07-12

The primary Pro research validated private implementation modules plus curated re-exports, small
preludes, Cargo feature unification/additivity, non-exhaustive public enums, and explicit MSRV/CI
matrices. A second independent Pro pass validated external Schedule ownership, opaque generational
handles, checked builder-to-runtime boundaries, and always-available fixed-point newtypes.

Primary references retained after source-quality filtering:

- Rust visibility and privacy: https://doc.rust-lang.org/reference/visibility-and-privacy.html
- Rustdoc re-exports: https://doc.rust-lang.org/rustdoc/write-documentation/re-exports.html
- Cargo features: https://doc.rust-lang.org/cargo/reference/features.html
- Cargo weak dependency features: https://rust-lang.github.io/rfcs/3143-cargo-weak-namespaced-features.html
- Non-exhaustive API: https://rust-lang.github.io/rfcs/2008-non-exhaustive.html
- Rust API naming: https://rust-lang.github.io/api-guidelines/naming.html
- Rust type layout (`repr(transparent)`): https://doc.rust-lang.org/reference/type-layout.html
- Fixed-point precedent: https://docs.rs/fixed/latest/fixed
- Bevy Schedule external-World precedent: https://docs.rs/bevy/latest/bevy/ecs/schedule/struct.Schedule.html

Research request ids reported by the independent passes:

- `80b49e0d-e753-47fc-bedd-a7e934d4fd7f`
- `a9a6b425-3e15-490a-ba58-a31b5dc15e29`

Inference boundary: Rust specifies visibility, re-export, feature, layout, and non-exhaustive
semantics. The exact Moirai facade membership, App ownership, module tree, and adapter ownership are
repository-grounded architectural decisions, not language mandates.
