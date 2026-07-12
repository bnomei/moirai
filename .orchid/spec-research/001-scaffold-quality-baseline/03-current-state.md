# Current State

- The repository is still planning-only; no `Cargo.toml` or `src/` implementation exists.
- `PHASE_1_SCAFFOLD.md` now freezes one dependency-pure package, additive features, private
  implementation children, root/prelude compile contracts, and separate MSRV/current-stable jobs.
- `docs/ARCHITECTURE.md` supplies the exact module tree and export lists.
- pd-asteroids publicly re-exports allocator, registry, sparse storage, commands, queues, and cache
  internals; Moirai treats those paths as source visibility accidents.
- Wyrd's private implementation/public facade split and Anapao's curated root, smaller prelude, and
  public API tests are the local precedents.
- Cargo features unify across a dependency graph, so mutually exclusive global numeric features
  would make Moirai's `--all-features` build incoherent.
- Coverage tooling may require a newer compiler than the library MSRV; it therefore cannot define
  the Rust 1.75 support contract.

