# Current-State Questions

1. Which pd-asteroids types are used by production game code, and which public exports are merely
   implementation leakage?
2. How do Wyrd and Anapao hide internals while providing root conveniences and semantic namespaces?
3. Who owns World data, executable systems, command buffers, ticks, and frame lifecycle today?
4. Which safe public calls can reach the source's unsafe schedule pointers or invalidate them?
5. Which features represent genuine optional capabilities, and which incorrectly select a global
   numeric universe for a numeric-agnostic ECS?
6. What does Wyrd persist or omit, and which clock advances its delay/state machinery?
7. Can Anapao's current Simulator execute an arbitrary external World or only CompiledScenario?
8. Which observation seam is needed after flush but before frame-event retirement?
9. Which tests are public behavior, characterization evidence, host-specific code, or undesirable
   quirks that should be adapted/rejected?
10. Which Rust visibility, re-export, Cargo-feature, non-exhaustive, and MSRV rules constrain the
    stable facade?
