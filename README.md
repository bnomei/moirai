# moirai

A standalone, single-threaded `no_std + alloc` ECS library — extracted from the production runtime
in [pd-asteroids](https://github.com/bnomei/pd-asteroids) and designed to work alongside
[wyrd](https://github.com/bnomei/wyrd) (signal-graph behavior) and
[anapao](https://github.com/bnomei/anapao) (deterministic verification).

**Status:** Phase 1 scaffold landed — private module tree, feature contract, boundary tests, and CI.
See [ROADMAP.md](./ROADMAP.md).

## Planning docs

The cross-phase public/module contract lives in [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).
Each `PHASE_*.md` is a **delegation-ready spec** (requirements, design, tasks, verification). See
[docs/SPEC_FORMAT.md](./docs/SPEC_FORMAT.md).

| Doc | Purpose |
| --- | --- |
| [ROADMAP.md](./ROADMAP.md) | Vision, ecosystem map, decisions, phase index |
| [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) | Single-crate module, facade, lifecycle, and interop contract |
| [docs/parity.md](./docs/parity.md) | All 151 source tests classified preserve/adapt/reject |
| [docs/wyrd-parity.md](./docs/wyrd-parity.md) | Source-audited 16-case Wyrd migration and restore-continuation gate |
| [PHASE_0_ANALYSIS.md](./PHASE_0_ANALYSIS.md) | Inventory, locked decisions, and sign-off gate |
| [PHASE_1_SCAFFOLD.md](./PHASE_1_SCAFFOLD.md) | Crate facade, visibility, CI, and features |
| [PHASE_2_CORE_STORAGE.md](./PHASE_2_CORE_STORAGE.md) | Checked identity, registration, storage, and Q16 |
| [PHASE_3_WORLD_LIFECYCLE.md](./PHASE_3_WORLD_LIFECYCLE.md) | World, commands, resources, events |
| [PHASE_4_SCHEDULE.md](./PHASE_4_SCHEDULE.md) | Safe App/Schedule validation and execution |
| [PHASE_5_QUERIES.md](./PHASE_5_QUERIES.md) | Queries & dual cache system |
| [PHASE_6_QUALITY.md](./PHASE_6_QUALITY.md) | Classified parity, coverage, API stability, and Divan |
| [PHASE_7_INTEGRATIONS.md](./PHASE_7_INTEGRATIONS.md) | Downstream Wyrd/Anapao adapters and host migrations |

## License

MIT — see [LICENSE](./LICENSE).
