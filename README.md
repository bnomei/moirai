# moirai

A standalone, single-threaded `no_std + alloc` ECS library — extracted from the production runtime
in [pd-asteroids](https://github.com/bnomei/pd-asteroids) and designed to work alongside
[wyrd](https://github.com/bnomei/wyrd) (signal-graph behavior) and
[anapao](https://github.com/bnomei/anapao) (deterministic verification).

**Status:** the core integration-readiness contract is implemented and under final quality review.
Downstream Wyrd adapters and game cutovers remain separate work; no downstream migration or
performance result is claimed here. See [ROADMAP.md](./ROADMAP.md).

## Quickstart

```rust
use moirai::prelude::*;
use moirai::stage;

#[derive(Debug, PartialEq)]
struct Counter(u32);

let mut builder = AppBuilder::new();
builder.insert_resource(Counter(0));
builder
    .add_system(System::new("increment", stage::UPDATE, |world, _dt| {
        world
            .resource_mut::<Counter>()
            .expect("registered resource")
            .expect("seeded resource")
            .0 += 1;
    }))
    .expect("valid system");

let mut app = builder.build().expect("valid app");
app.update(1.0 / 60.0).expect("update");
assert_eq!(app.world().resource::<Counter>().unwrap(), Some(&Counter(1)));
```

Events use an explicit typed broadcast contract (`E: Clone + 'static`), schedules validate typed
producer/consumer roles, and runtime stage handles are obtained through `Schedule::stage_id` and
resolved through checked `Schedule::stage_label`.

## Examples

The canonical learning path is the ordered [`moirai::examples`](https://docs.rs/moirai/latest/moirai/examples/index.html)
Rustdoc hierarchy. It starts with world and application foundations, then progresses through
scheduling, queries, constrained host data, and deterministic replay. Every lesson is a runnable
stable-Rust doctest using the public API.

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
| [PHASE_5_QUERIES.md](./PHASE_5_QUERIES.md) | Prepared queries and execution policies |
| [PHASE_6_QUALITY.md](./PHASE_6_QUALITY.md) | Classified parity, coverage, API stability, and Divan |
| [PHASE_7_INTEGRATIONS.md](./PHASE_7_INTEGRATIONS.md) | Downstream Wyrd/Anapao adapters and host migrations |

## License

MIT — see [LICENSE](./LICENSE).
