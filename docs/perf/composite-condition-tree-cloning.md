# Evaluate composite schedule conditions without cloning their trees

Priority: high

Confidence: high; allocation removal and latency are measured
Target metric: schedule-update allocator calls and system-condition evaluation latency

Hotspot:

Every `And` or `Or` node clones each child `ConditionKind` before recursively evaluating it. The same cloning pattern is repeated while advancing system and set cursors. Because composite nodes own `Box<ConditionKind>`, cloning a non-leaf subtree allocates.

Evidence:

- [`src/schedule/condition.rs:107`](../../src/schedule/condition.rs#L107) constructs `Condition(left.as_ref().clone())` and the same for `right` during every system evaluation.
- [`src/schedule/condition.rs:145`](../../src/schedule/condition.rs#L145) repeats the clones for set-condition evaluation.
- [`src/schedule/condition.rs:183`](../../src/schedule/condition.rs#L183) and [`src/schedule/condition.rs:217`](../../src/schedule/condition.rs#L217) repeat them again for cursor advancement.
- [`tests/allocation.rs:358`](../../tests/allocation.rs#L358) proves only a leaf `Condition::always()` path is allocation-free; it does not cover `and`/`or`.
- Composite conditions are public and exercised functionally at [`tests/schedule.rs:1539`](../../tests/schedule.rs#L1539) and [`tests/schedule.rs:1820`](../../tests/schedule.rs#L1820), but those tests do not count allocations.
- The leaf-only release allocation test passed on this checkout; that result does not cover the cloning mechanism.

Candidate and mechanism:

Add private recursive helpers on `&ConditionKind` for evaluate and cursor advancement, and recurse directly through `left.as_ref()` and `right.as_ref()`. Keep the public owned `Condition` builder unchanged. This removes per-pass ownership reconstruction while preserving short-circuit behavior.

Expected scope (not promised speedup):

This removes heap allocation and deep tree/box clone work proportional to composite-tree size every time a gated system or set is evaluated and, when it runs, advanced. The effect scales with tree depth, gated systems, stages, and fixed substeps. Leaf conditions are already the likely loss/neutral case because they never take these branches.

Semantic and operational risks:

- `And` and `Or` must preserve left-to-right short-circuit evaluation exactly, especially for user predicates with interior-mutability side effects.
- Cursor advancement currently visits both children regardless of evaluation short-circuit; that behavior must remain unchanged unless separately specified.
- Deep user-built trees remain recursive and could overflow the stack; flattening is a separate design with ordering and code-size tradeoffs.
- Avoid `unsafe`; borrowing is sufficient.

Benchmark plan:

Add release allocation tests for balanced and skewed composite conditions at depths `[1, 4, 16, 64]`, for both system and set gates, and assert zero allocator calls after construction. Add Divan cases sweeping tree depth, systems per set, all-true, first-false, and last-false distributions; include fixed-update substeps. Compare direct borrowed recursion with the current clone path and optionally a flattened postfix representation. The disproof case is a leaf-only schedule or shallow tree where helper indirection/code growth regresses latency despite identical allocation counts.

Result:

Accepted. Evaluation and cursor advancement now recurse over borrowed `ConditionKind` nodes, preserving left-to-right short-circuit evaluation and unconditional two-child cursor advancement. Five paired host captures at 100 samples x 100 iterations reduced median update latency from 48.09-51.00 ns to 41-42 ns for the stable depth-1 pairs, 425.6-479.7 ns to 47.69-56.40 ns at depth 4, 7.112-7.833 us to 79.78-95.19 ns at depth 16, and 148.1-155.3 us to 857.9-920.2 ns at depth 64. One depth-1 candidate run was a 59.32 ns outlier; the scaled cases and removed clone mechanism remain decisive.

Decision and fallback:

Retain borrowed recursion. Functional condition tests pass and a release allocation contract now covers a constructed composite tree after warmup. Leaf-only scheduling remains the neutral case; no flattening or unsafe representation is introduced.
