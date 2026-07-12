# Implementation Shape

Split the work into two specs/tasks with an explicit handoff:

## World data

```text
storage/{table,archetype}
world/{bundle,access,spawn,flush,resources,events}
command/{mod,queue}
resource/store
event/{registry,queue,component}
```

Prove Reserved/Live transitions, all storage moves, batch preflight, typed resource change ticks,
resource scope, event readers/retention, and the run guard.

## Safe execution

```text
schedule/{stage,system,condition,builder,compiled,runner,error}
operation
app
state
time
diagnostics
```

Prove build validation, operation-owned stages/frame events, deterministic order, conditions,
Update-only flush modes, Render structural rejection, fixed accumulator, Startup once,
update/render tick behavior, pending-command rejection, stable observation, failure cleanup/fault
state, and no raw pointer/unsafe code.

Tests/benches land with each owner; Phase 6 only closes the accumulated matrix.
