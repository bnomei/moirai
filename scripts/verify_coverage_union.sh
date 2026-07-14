#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="$ROOT/target"
FINAL="$TARGET/coverage-union"
LOCK="$TARGET/.coverage-union.lock"
STAGE=""
PREVIOUS=""
SOURCE_DIGEST_BEFORE=""

mkdir -p "$TARGET"
if ! mkdir "$LOCK" 2>/dev/null; then
    echo "coverage union is already running (lock: $LOCK)" >&2
    exit 2
fi

cleanup() {
    if [[ -n "$STAGE" && -d "$STAGE" ]]; then
        rm -rf "$STAGE"
    fi
    if [[ -n "$PREVIOUS" && -d "$PREVIOUS" && ! -e "$FINAL" ]]; then
        mv "$PREVIOUS" "$FINAL"
    fi
    rmdir "$LOCK" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

cd "$ROOT"
STAGE="$(mktemp -d "$TARGET/.coverage-union.XXXXXX")"
mkdir -p "$STAGE/flavors"

echo "== coverage analyzer self-test =="
uv run --no-project python scripts/coverage_union.py --self-test
uv run --no-project python scripts/coverage_union.py --repo "$ROOT" --audit-only
SOURCE_DIGEST_BEFORE="$(
    uv run --no-project python scripts/coverage_union.py --repo "$ROOT" --source-digest
)"

echo "== coverage: no default features =="
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --no-default-features
cargo llvm-cov report --lcov --output-path "$STAGE/flavors/no-default.lcov"
cargo llvm-cov report --json --output-path "$STAGE/flavors/no-default.json"

echo "== coverage: std =="
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --features std
cargo llvm-cov report --lcov --output-path "$STAGE/flavors/std.lcov"
cargo llvm-cov report --json --output-path "$STAGE/flavors/std.json"

echo "== coverage: testkit =="
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --features testkit
cargo llvm-cov report --lcov --output-path "$STAGE/flavors/testkit.lcov"
cargo llvm-cov report --json --output-path "$STAGE/flavors/testkit.json"

echo "== coverage: all features =="
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --all-features
cargo llvm-cov report --lcov --output-path "$STAGE/flavors/all-features.lcov"
cargo llvm-cov report --json --output-path "$STAGE/flavors/all-features.json"

SOURCE_DIGEST_AFTER="$(
    uv run --no-project python scripts/coverage_union.py --repo "$ROOT" --source-digest
)"
if [[ "$SOURCE_DIGEST_BEFORE" != "$SOURCE_DIGEST_AFTER" ]]; then
    echo "source inputs changed while coverage flavors were collected" >&2
    exit 1
fi

uv run --no-project python scripts/coverage_union.py \
    --repo "$ROOT" \
    --verified-source-digest "$SOURCE_DIGEST_BEFORE" \
    --flavor no-default "$STAGE/flavors/no-default.lcov" "$STAGE/flavors/no-default.json" \
    --flavor std "$STAGE/flavors/std.lcov" "$STAGE/flavors/std.json" \
    --flavor testkit "$STAGE/flavors/testkit.lcov" "$STAGE/flavors/testkit.json" \
    --flavor all-features "$STAGE/flavors/all-features.lcov" "$STAGE/flavors/all-features.json" \
    --summary "$STAGE/summary.json" \
    --missing "$STAGE/missing-lines.txt" \
    --manifest "$STAGE/manifest.json" \
    --command "cargo llvm-cov clean --workspace" \
    --command "cargo llvm-cov --no-report --no-default-features" \
    --command "cargo llvm-cov report --lcov --output-path <staging>/flavors/no-default.lcov" \
    --command "cargo llvm-cov report --json --output-path <staging>/flavors/no-default.json" \
    --command "cargo llvm-cov clean --workspace" \
    --command "cargo llvm-cov --no-report --features std" \
    --command "cargo llvm-cov report --lcov --output-path <staging>/flavors/std.lcov" \
    --command "cargo llvm-cov report --json --output-path <staging>/flavors/std.json" \
    --command "cargo llvm-cov clean --workspace" \
    --command "cargo llvm-cov --no-report --features testkit" \
    --command "cargo llvm-cov report --lcov --output-path <staging>/flavors/testkit.lcov" \
    --command "cargo llvm-cov report --json --output-path <staging>/flavors/testkit.json" \
    --command "cargo llvm-cov clean --workspace" \
    --command "cargo llvm-cov --no-report --all-features" \
    --command "cargo llvm-cov report --lcov --output-path <staging>/flavors/all-features.lcov" \
    --command "cargo llvm-cov report --json --output-path <staging>/flavors/all-features.json"

# Publish the evidence and normalized artifacts as one directory only after every flavor succeeds.
# Keeping the previous directory until the rename completes avoids a stale/new mixed set.
if [[ -e "$FINAL" ]]; then
    PREVIOUS="$TARGET/.coverage-union.previous.$$"
    mv "$FINAL" "$PREVIOUS"
fi
mv "$STAGE" "$FINAL"
STAGE=""
if [[ -n "$PREVIOUS" ]]; then
    rm -rf "$PREVIOUS"
    PREVIOUS=""
fi

if [[ -s "$FINAL/missing-lines.txt" ]]; then
    echo "production source-line coverage misses:" >&2
    cat "$FINAL/missing-lines.txt" >&2
    exit 1
fi

echo "Production source-line union coverage is 100%."
echo "Artifacts: $FINAL"
