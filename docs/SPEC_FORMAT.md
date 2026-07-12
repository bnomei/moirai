# moirai phase spec format

Each `PHASE_*.md` is a **delegation-ready spec**, not a surface checklist.

## Sections

| Section | Purpose |
| --- | --- |
| **Metadata** | Status, depends on, consumers, estimated scope |
| **Requirements** | `R###` — WHEN/SHALL acceptance criteria |
| **Design** | Architecture, invariants, API contracts, data structures |
| **Tasks** | `T###` — ordered, verifiable work units |
| **Verification** | Exact `cargo` / script commands that prove done |
| **Risks** | Known failure modes + mitigations |
| **References** | Source repos, research, sibling patterns |

## Requirement IDs

- Phase N requirements: `R{N}xx` (e.g. Phase 2 → `R201`)
- Tasks: `T{N}xx`

## Sign-off

A phase is **done** only when every `R###` has a matching verification command green in CI or local gate documented in that phase.