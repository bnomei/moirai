# Research Questions

## Resolved

- **Which crate topology?** One published `moirai` crate through 1.0.
- **Which platform envelope?** `#![no_std]` with unconditional `alloc` and no unsafe code.
- **Which features?** Exactly additive `default = []`, `std = []`, `testkit = []`.
- **Which paths are stable?** Semantic namespaces plus the exact root/prelude lists in
  `docs/ARCHITECTURE.md`.
- **How is MSRV tested?** Rust 1.75 checks the library only; coverage/docs tooling uses current
  stable.
- **How is visibility protected?** Downstream root/namespace/prelude tests, doctests, and
  representative compile-fail tests.

## Deferred to implementation spec

- Exact MSRV-compatible dev-dependency versions.
- Exact CI action revisions and coverage report merge commands.
- License/advisory tool selection.

These are tooling choices, not architecture reopeners.
