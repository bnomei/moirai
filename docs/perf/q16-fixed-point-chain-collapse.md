# The Q16 chain spends most iterations at one raw bit

Priority: high
Confidence: high

Hotspot:

`benches/q16.rs:3-12` is the only Q16 performance case and is presented as the Q16 family baseline, but its dependent multiply-by-one-half chain quickly stops varying.

Evidence:

- The seed is 1.25 (`81920` raw bits), every right-hand operand is 0.5 (`32768` raw bits), and every result is rounded halfway away from zero.
- Following the repository's `checked_mul` and `round_half_away_i64` semantics (`src/math/q16.rs:95-98`, `180-202`), the positive raw-bit sequence reaches `1` after 17 multiplies. Multiplying raw bit `1` by 0.5 rounds back to `1`, so the remaining 47 of 64 iterations stay at that fixed point.
- The optimized benchmark assembly produced by `cargo rustc --bench q16 --release -- --emit=asm` constant-folds both float conversions, loads `32768` as the right-hand operand, and calls the same out-of-line `checked_mul` 64 times.
- The case does not cover negative values, exact integers, varying operands, overflow/underflow, division, conversion, or a production-shaped batch. It therefore cannot support a general Q16 performance conclusion.

Candidate and mechanism:

Keep the halving chain only as a specifically named dependent-chain diagnostic, and add an argument/corpus matrix of independent dynamic raw-bit operand pairs. Separate checked multiply, checked division, saturating paths, and f32 conversion so each mechanism is attributable. Include positive/negative values, exact and halfway rounding, near-boundary success, and overflow/error cases.

Expected scope (not promised speedup):

The new cases will expose operand-sensitive latency and code generation. They may show that the current chain is representative only of repeated constant scaling; they do not imply that any Q16 implementation change will be faster.

Semantic and operational risks:

Microbenchmarks can overemphasize arithmetic that is insignificant in a real ECS frame. Error-heavy corpora can misrepresent production branch rates. Independent operations remove the dependency chain and may measure throughput rather than latency.

Benchmark plan:

1. Retain the current dependency chain as a latency control and add dynamic independent-pair throughput cases using `black_box` on inputs and results.
2. Use several batch sizes large enough to exceed the 41 ns timer precision, and report operation counts.
3. Split success, rounding-boundary, overflow/underflow, and division-by-zero cases so their expected frequencies are explicit.
4. Confirm exact results against the existing wider-integer reference/property tests before comparing implementations.
5. Promote a candidate only if it wins its intended operand distribution and a component or downstream frame workload; reject it if it loses on dynamic operands or changes any rounding/error result.

Losing/crossover case:

The current constant-half chain is useful when a downstream game repeatedly applies a fixed damping factor and latency dependencies prevent instruction-level parallelism. Independent batches are more representative of vector-like throughput and may favor a different implementation.

Result:

Accepted as a benchmark correction. `benches/q16.rs` now keeps the old workload under the explicit `q16_mul_chain_constant_half_control` name and adds black-boxed dynamic corpora for checked multiply/divide, saturating multiply/divide, and float conversion. Five host captures with `--timer os --sample-count 100 --sample-size 100` put the constant-half control at 175.2-175.6 ns, dynamic checked multiply at 16.44-18.12 ns, and dynamic checked divide at 18.10-18.12 ns. These are distinct workload metrics, not before/after speedups.

Decision and fallback:

Retain the expanded corpus and stop treating the constant-half chain as the Q16-family baseline. Any future arithmetic candidate must pass the dynamic success/error/rounding cases and a downstream workload; the fixed-chain control remains useful only for dependent constant scaling.
