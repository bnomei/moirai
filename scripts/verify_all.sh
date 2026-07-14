#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "== fmt =="
cargo fmt --all -- --check

echo "== clippy =="
cargo clippy --all-targets --all-features -- -D warnings

echo "== tests =="
cargo test --no-default-features
cargo test --features std
cargo test --features testkit
cargo test --all-features
cargo test --release --features std allocation -- --test-threads=1

echo "== production coverage union =="
scripts/verify_coverage_union.sh

echo "== msrv =="
cargo +1.75 check --lib --no-default-features

echo "== docs =="
cargo test --doc --package moirai-for-games --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --package moirai-for-games --no-deps --all-features --locked

echo "== bench compile =="
cargo bench --no-run

echo "== package =="
cargo package --allow-dirty --locked

echo "Repository verification commands completed."
