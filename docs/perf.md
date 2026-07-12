# Performance policy

Divan benchmarks compile in Phase 1 but publish no placeholder performance numbers. Each hot-path
owning phase adds representative release cases for both `f32` and `Q16` workloads in the same build.

Benchmark execution and device checks remain hypotheses until Phase 6 closes the performance gate.