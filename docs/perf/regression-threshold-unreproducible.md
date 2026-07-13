# The 10% median gate has no demonstrated noise floor

Priority: high
Confidence: high

Hotspot:

`docs/perf.md:70-74` asks reviewers to investigate a median regression above about 10%, but the repository does not record repeated baseline distributions or an exact command that reproduces the saved sampling configuration.

Evidence:

- `docs/perf.md:15` records `cargo bench` with "Divan defaults", while the Q16 row at `docs/perf.md:25` says `100 samples x 100 iters`. Those are materially different experiment descriptions; the exact flags are absent.
- The saved Q16 median is 416.4 ns at commit `ab93dbb1d68796b2c5fbb9b5976f8e56834ad4f0` on Rust 1.96.0 / Apple M4.
- On 2026-07-13 at `dfd4177b293651536413377be783b3ac0c19bc9f`, five consecutive runs of `cargo bench --bench q16 -- --sample-count 100 --sample-size 100` produced medians of 181.4, 181.4, 181.6, 181.4, and 181.4 ns (41 ns timer precision).
- `git diff ab93dbb1d68796b2c5fbb9b5976f8e56834ad4f0..HEAD -- benches/q16.rs src/math/q16.rs Cargo.toml` shows no Q16 benchmark change and only a `#[cfg(test)]` Q16 helper plus an unrelated test-target declaration. This does not prove identical generated artifacts, but it leaves the 56% median difference unexplained by the measured workload's source.
- CI only runs `cargo bench --no-run` (`.github/workflows/ci.yml:125-137`), so it checks compilation but neither calibrates runner noise nor preserves performance distributions.

Candidate and mechanism:

Replace the single-number advisory gate with a capture protocol: record the exact Divan flags, commit and dirty state, feature set, target, rustc/LLVM, timer, power mode, and multiple independent repetitions. Compare an unchanged control immediately before and after a candidate, or alternate baseline and candidate when thermal drift is plausible. Derive the investigation threshold from observed control noise plus a minimum useful effect, not from an uncalibrated constant.

Expected scope (not promised speedup):

This improves regression classification. It should reduce false alarms and prevent large environment or harness shifts from being presented as code improvements.

Semantic and operational risks:

Repeated local runs take longer and still do not make an Apple M4 baseline portable to CI or constrained targets. Divan medians do not provide saved cross-revision confidence intervals by themselves. A wider evidence-based threshold can miss small but cumulative regressions.

Benchmark plan:

1. Re-capture the unchanged HEAD at least five times with explicit `--sample-count` and `--sample-size`, preserving raw output.
2. Run the same five-repetition control on a second day or after a normal thermal/load cycle to measure between-session drift.
3. Alternate baseline and one no-op/rebuild candidate to measure artifact/layout noise.
4. Set a per-family advisory threshold only above the larger demonstrated noise floor; require an end-to-end confirmation for borderline results.
5. Disprove the new protocol if it cannot reliably classify an intentionally injected slowdown larger than its threshold.

Losing/crossover case:

A compile-only CI smoke gate remains cheaper and more portable for every pull request. Timing gates belong on controlled hardware or scheduled runs; noisy shared runners should not enforce local nanosecond baselines.

Result:

Accepted as a reproducibility protocol. `scripts/perf_capture.py`, run through UV, records the exact command, label, UTC timestamp, commit, dirty state, rustc/Cargo/host metadata, exit status, and raw stdout/stderr beneath the ignored `target/perf-results/` directory. Five repeated Q16 foundation captures and paired runtime-candidate captures use explicit OS timer and 100 x 100 sampling.

Decision and fallback:

Replace the uncalibrated 10% rule for this work with paired five-run gates: latency-only changes need at least a 5% win in four of five pairs and no case worse than 3%; work-removal changes must eliminate the claimed work/allocation and keep latency within 3%. Keep compile-only benchmark CI as the portable fallback and retain raw local captures for audit rather than committing machine-specific output.
