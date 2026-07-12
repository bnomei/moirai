# Implementation Shape

Create:

```text
Cargo.toml
src/lib.rs
src/prelude.rs
src/{app,command,state,time}.rs
src/{component,event,math,query,schedule,world,diagnostics}/...
src/{entity,resource,storage}/...              private
tests/{public_api,prelude,readme}.rs
benches/                                      harness only
.github/workflows/ci.yml
docs/{coverage,perf}.md
```

Only final semantic facades with an actual owning surface are public. Phase 1 creates private
physical modules and crate documentation; semantic namespaces, root re-exports, prelude, and
`src/testkit` wait for their owning phases rather than becoming empty API. Do not add panic/no-op
placeholder methods to make examples look executable; each owning phase extends tests when behavior
exists.

CI slices:

1. fmt;
2. strict Clippy on all targets/features;
3. tests for core/std/testkit/all-features;
4. rustdoc with warnings denied;
5. Rust 1.75 library-only no-default check.

Coverage scripts may be scaffolded but cannot claim 100% until executable code exists. The Divan
harness compiles without publishing placeholder results.
