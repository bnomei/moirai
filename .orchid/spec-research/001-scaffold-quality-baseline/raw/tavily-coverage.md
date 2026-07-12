# Tavily provenance

Query: cargo llvm-cov MSRV Rust version requirement no_std coverage. Primary result: https://github.com/taiki-e/cargo-llvm-cov . Search result alone did not establish a supported MSRV/no_std matrix; that remains a local spike.

Published-package metadata checked locally on 2026-07-12: `cargo info cargo-llvm-cov` reported version 0.8.7 with `rust-version: 1.87`; `cargo info rstest@0.24.0` reported `rust-version: 1.70.0`. This establishes the toolchain split required by Phase 1.
