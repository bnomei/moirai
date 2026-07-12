# Research Questions

## Resolved

- `World` cannot own `Schedule` while systems receive `&mut World` and Moirai forbids unsafe code.
- `App` owns sibling World/Schedule and is the root lifecycle convenience.
- Idle structural mutation is immediate; Update systems use deferred `Commands`, while Render is
  topology-read-only and rejects Commands.
- Deferred spawn reserves an id; a full command batch preflights before commitment.
- Cycles and registration/dependency conflicts are build errors.
- Standard Moirai stages are generic; host graphs remain downstream.
- The replay observer is Update-only and runs after final flush and before Update-frame clearing;
  Render observation is separately post-system/pre-clear.
- Expected system/flush failures produce an explicit App fault, not silent partial continuation.
- World, Fixed, Wyrd Settle, and replay Step ticks are independent domains.
- The generic standard schedule is Startup once, up to eight due FixedUpdate steps, then Update;
  excess whole fixed steps are dropped with diagnostics while the fractional remainder survives.
- FixedUpdate is disabled until a positive Duration is configured; systems in that stage otherwise
  cause a build error.
- Untyped system pipes and per-system interval buffering are rejected for 1.0.
- Generic State retains current/previous/pending and rejects conflicting pending requests; host
  navigation stacks stay downstream.

## Resolved diagnostics seam

- One `Observer::observe(DiagnosticEvent<'_>)` method receives a non-exhaustive event enum.
- It has no World/Schedule mutation access and samples host clocks downstream.
- App owns an optional boxed observer; absence performs no allocation or platform call.
