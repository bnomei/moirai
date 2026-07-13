# Q16 checked multiply retains a cross-crate call boundary

Priority: medium
Confidence: medium

Hotspot:

The Q16 benchmark invokes public `Q16::checked_mul` from a separate benchmark crate 64 times. The implementation has no inline hint, so the constant right-hand operand cannot currently be propagated through that call boundary in the observed release artifact.

Evidence:

- `src/math/q16.rs:95-98` implements `checked_mul` through a private rounding helper and has no `#[inline]` attribute.
- `cargo rustc --bench q16 --release -- --emit=asm` on `aarch64-apple-darwin` emits `bl ...Q16::checked_mul` inside the 64-iteration benchmark loop even though the right-hand value is the constant raw bit pattern `32768`.
- `cargo rustc --release --lib -- --emit=asm` shows that the AArch64 `checked_mul` body already replaces division by 65536 with multiply/add/shift/select instructions and contains no `sdiv`.
- `cargo rustc --release --lib --target x86_64-apple-darwin -- --emit=asm` likewise emits multiply, adjustment, masks, and shifts for `checked_mul`, with no `idiv`. A hand-written fixed-shift rewrite therefore has no demonstrated codegen advantage on either inspected target.

Candidate and mechanism:

Experiment with ordinary `#[inline]` on `Q16::checked_mul` only. Cross-crate inlining may expose constant operands, remove `Result` tagging/branches at proven-success call sites, and let the caller optimize a surrounding loop. Do not use `#[inline(always)]`, unsafe indexing, intrinsics, or a manual rounding rewrite without separate evidence.

Expected scope (not promised speedup):

Potential benefit is limited to hot cross-crate call sites that LLVM chooses to inline, especially constant-scale operations. Dynamic operands, cold callers, and LTO builds may see no runtime improvement.

Semantic and operational risks:

Inlining duplicates the nontrivial overflow and half-away rounding path, increasing `.text`, compile time, and instruction-cache pressure. It can optimize the artificial constant-half benchmark without helping production-shaped dynamic arithmetic. Codegen differs by architecture and rustc version.

Benchmark plan:

1. First land the operand matrix described in `q16-fixed-point-chain-collapse.md` and save the unannotated baseline.
2. Compare only `#[inline]` versus status quo on constant and dynamic multiply cases, then confirm a downstream/component workload.
3. Inspect caller assembly for call removal and retained overflow/error behavior on AArch64, x86_64, and every supported constrained target.
4. Record final linked `.text`/artifact bytes and clean build/link time, not only the `.rlib` archive size.
5. Reject the hint if its win is confined to constant-half inputs, if dynamic cases regress, or if code-size growth is disproportionate to the end-to-end effect.

Losing/crossover case:

With fat/ThinLTO the call may already be inlined; with dynamic operands the body may remain equally expensive; with many call sites code duplication may cost more than a predictable call. Tiny constrained instruction caches are the most important losing case.

Result:

Deferred at the target-evidence gate. The repaired corpus establishes a host baseline, but this repository has neither a shipping host that links Moirai nor access to a physical Playdate measurement in this run. An ordinary `#[inline]` experiment could therefore prove only host microbenchmark code generation, not the constrained-target result required for retention.

Decision and fallback:

Leave `Q16::checked_mul` unchanged and reject the manual shift rewrite. Reopen the ordinary `#[inline]` candidate only when the same revision can be measured in a representative linked host and on physical Playdate hardware, with dynamic operand results, output checks, linked size, and build time recorded. Host-only wins must be reverted.
