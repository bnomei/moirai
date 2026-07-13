#!/usr/bin/env python3
"""Capture a benchmark command and reproducibility metadata under target/.

Example:
    uv run python scripts/perf_capture.py \
        --label q16-baseline-a1 \
        --power-note "AC power; automatic power mode" \
        -- cargo bench --bench q16 -- --timer os --sample-count 100 --sample-size 100
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import pathlib
import platform
import shlex
import subprocess
import sys
import time
from typing import Any, Sequence


RELEVANT_ENVIRONMENT_KEYS = {
    "CARGO_BUILD_JOBS",
    "CARGO_BUILD_TARGET",
    "CARGO_INCREMENTAL",
    "MACOSX_DEPLOYMENT_TARGET",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
}
RELEVANT_ENVIRONMENT_PREFIXES = ("CARGO_PROFILE_", "CARGO_TARGET_")


def run_metadata_command(command: Sequence[str], cwd: pathlib.Path) -> str | None:
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return None
    if result.returncode != 0:
        return None
    return result.stdout.rstrip()


def cpu_description() -> str:
    if sys.platform == "darwin":
        value = run_metadata_command(["sysctl", "-n", "machdep.cpu.brand_string"], pathlib.Path.cwd())
        if value:
            return value
    return platform.processor() or platform.machine() or "unknown"


def command_value(command: Sequence[str], option: str) -> str | None:
    for index, argument in enumerate(command):
        if argument == option and index + 1 < len(command):
            return command[index + 1]
        prefix = f"{option}="
        if argument.startswith(prefix):
            return argument[len(prefix) :]
    return None


def selected_features(command: Sequence[str]) -> dict[str, Any]:
    features = command_value(command, "--features") or command_value(command, "-F")
    return {
        "features": features.replace(",", " ").split() if features else [],
        "all_features": "--all-features" in command,
        "no_default_features": "--no-default-features" in command,
    }


def git_metadata(cwd: pathlib.Path) -> dict[str, Any]:
    status = run_metadata_command(["git", "status", "--short"], cwd)
    return {
        "commit": run_metadata_command(["git", "rev-parse", "HEAD"], cwd),
        "dirty": bool(status),
        "status_short": status.splitlines() if status else [],
    }


def capture_working_tree(cwd: pathlib.Path, capture_dir: pathlib.Path) -> dict[str, Any]:
    patch = subprocess.run(
        ["git", "diff", "--binary", "HEAD", "--", "."],
        cwd=cwd,
        check=False,
        capture_output=True,
    )
    patch_bytes = patch.stdout if patch.returncode == 0 else b""
    (capture_dir / "working-tree.patch").write_bytes(patch_bytes)

    untracked_result = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"],
        cwd=cwd,
        check=False,
        capture_output=True,
    )
    untracked: list[dict[str, Any]] = []
    if untracked_result.returncode == 0:
        for raw_path in untracked_result.stdout.split(b"\0"):
            if not raw_path:
                continue
            relative = raw_path.decode("utf-8", errors="surrogateescape")
            path = cwd / relative
            if path.is_file():
                contents = path.read_bytes()
                untracked.append(
                    {
                        "path": relative,
                        "bytes": len(contents),
                        "sha256": hashlib.sha256(contents).hexdigest(),
                    }
                )

    identity = hashlib.sha256()
    identity.update(patch_bytes)
    for entry in sorted(untracked, key=lambda item: item["path"]):
        identity.update(entry["path"].encode("utf-8", errors="surrogateescape"))
        identity.update(entry["sha256"].encode("ascii"))
    return {
        "working_tree_sha256": identity.hexdigest(),
        "tracked_patch_sha256": hashlib.sha256(patch_bytes).hexdigest(),
        "tracked_patch_bytes": len(patch_bytes),
        "untracked_files": untracked,
    }


def toolchain_metadata(cwd: pathlib.Path) -> dict[str, Any]:
    verbose = run_metadata_command(["rustc", "-vV"], cwd)
    fields: dict[str, str] = {}
    if verbose:
        for line in verbose.splitlines():
            key, separator, value = line.partition(": ")
            if separator:
                fields[key] = value
    return {
        "rustc_verbose": verbose,
        "rustc_release": fields.get("release"),
        "host_target": fields.get("host"),
        "llvm_version": fields.get("LLVM version"),
        "cargo_version": run_metadata_command(["cargo", "--version"], cwd),
    }


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label", required=True, help="Filesystem-safe capture label")
    parser.add_argument(
        "--power-note",
        default="not recorded",
        help="Power source/mode and other thermal context",
    )
    parser.add_argument(
        "--output-dir",
        type=pathlib.Path,
        default=pathlib.Path("target/perf-results"),
        help="Ignored directory for capture artifacts",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required after --")
    if not args.label or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_." for character in args.label):
        parser.error("--label may contain only letters, digits, dash, underscore, and dot")
    return args


def write_json(path: pathlib.Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_arguments()
    cwd = pathlib.Path.cwd().resolve()
    captured_at = dt.datetime.now(dt.timezone.utc)
    timestamp = captured_at.strftime("%Y%m%dT%H%M%SZ")
    capture_dir = (cwd / args.output_dir / f"{timestamp}-{args.label}").resolve()
    capture_dir.mkdir(parents=True, exist_ok=False)

    environment = {
        key: value
        for key, value in sorted(os.environ.items())
        if key in RELEVANT_ENVIRONMENT_KEYS or key.startswith(RELEVANT_ENVIRONMENT_PREFIXES)
    }
    metadata: dict[str, Any] = {
        "schema_version": 1,
        "label": args.label,
        "captured_at_utc": captured_at.isoformat(),
        "working_directory": str(cwd),
        "command_argv": args.command,
        "command_display": shlex.join(args.command),
        "benchmark_options": {
            "target": command_value(args.command, "--target"),
            "timer": command_value(args.command, "--timer"),
            "sample_count": command_value(args.command, "--sample-count"),
            "sample_size": command_value(args.command, "--sample-size"),
        },
        "features": selected_features(args.command),
        "environment": environment,
        "git": git_metadata(cwd),
        "toolchain": toolchain_metadata(cwd),
        "machine": {
            "os": platform.platform(),
            "system": platform.system(),
            "release": platform.release(),
            "architecture": platform.machine(),
            "cpu": cpu_description(),
            "logical_cpu_count": os.cpu_count(),
        },
        "power_note": args.power_note,
        "status": "running",
    }
    metadata["git"].update(capture_working_tree(cwd, capture_dir))
    metadata_path = capture_dir / "metadata.json"
    write_json(metadata_path, metadata)

    started = time.monotonic()
    try:
        result = subprocess.run(
            args.command,
            cwd=cwd,
            check=False,
            capture_output=True,
        )
    except OSError as error:
        result = None
        metadata["status"] = "launch-error"
        metadata["error"] = str(error)
        exit_code = 127
    else:
        (capture_dir / "stdout.log").write_bytes(result.stdout)
        (capture_dir / "stderr.log").write_bytes(result.stderr)
        sys.stdout.buffer.write(result.stdout)
        sys.stdout.buffer.flush()
        sys.stderr.buffer.write(result.stderr)
        sys.stderr.buffer.flush()
        exit_code = result.returncode
        metadata["status"] = "passed" if exit_code == 0 else "failed"

    metadata["exit_code"] = exit_code
    metadata["duration_seconds"] = time.monotonic() - started
    metadata["completed_at_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    write_json(metadata_path, metadata)
    print(f"performance capture: {capture_dir}", file=sys.stderr)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
