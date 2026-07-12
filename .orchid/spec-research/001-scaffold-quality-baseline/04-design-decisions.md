# Design Decisions

- One package named `moirai`, edition 2021, Rust 1.75.
- `#![no_std]`, unconditional `extern crate alloc`, and `#![forbid(unsafe_code)]`.
- Features are only `std` and `testkit`, both additive and empty at the dependency edge until used;
  the public `testkit` namespace begins with its real Phase 6 replay vocabulary, not an empty Phase
  1 facade.
- Public modules describe concepts and first publish only with their owning real surface;
  allocator/storage/registry/queue/runner files remain private.
- Root conveniences and prelude are curated independently; prelude is intentionally smaller.
- Public structs use private fields/builders; growing errors are non-exhaustive.
- Phase 1 compile tests prove privacy and no premature API; owner phases add public import tests as
  their real types/traits arrive, and Phase 6 closes the complete matrix.
- Current stable owns fmt/clippy/tests/docs/coverage; Rust 1.75 owns a library-only no-default check.
- No second crate, derive macro, Wyrd/Anapao dependency, empty `alloc` feature, or numeric feature.

Architecture authority: packet 006 and `docs/ARCHITECTURE.md`.
