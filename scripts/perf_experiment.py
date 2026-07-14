#!/usr/bin/env python3
"""Run alternating baseline/candidate performance captures.

Example:
    uv run python scripts/perf_experiment.py \
        --group q16 \
        --baseline-command "cargo bench --bench q16 -- --timer os" \
        --candidate-command "cargo bench --bench q16_candidate -- --timer os"
"""

from __future__ import annotations

import argparse
import pathlib
import shlex
import subprocess
import sys
import uuid
from collections.abc import Sequence


def positive_integer(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return parsed


def parse_arguments(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--group", required=True, help="Label prefix for the experiment")
    parser.add_argument(
        "--cohort",
        help="Identity for this experiment invocation (default: a new random identity)",
    )
    parser.add_argument(
        "--cycles",
        type=positive_integer,
        default=7,
        help="Number of paired cycles to run (default: 7)",
    )
    parser.add_argument(
        "--baseline-command",
        required=True,
        help="Quoted baseline command, split with shell-like quoting but never run in a shell",
    )
    parser.add_argument(
        "--candidate-command",
        required=True,
        help="Quoted candidate command, split with shell-like quoting but never run in a shell",
    )
    parser.add_argument(
        "--power-note",
        default="not recorded",
        help="Power source/mode and other thermal context",
    )
    parser.add_argument(
        "--output-dir",
        type=pathlib.Path,
        default=pathlib.Path("target/perf-results"),
        help="Directory passed to perf_capture.py",
    )
    args = parser.parse_args(argv)
    allowed = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_."
    for option in ("group", "cohort"):
        value = getattr(args, option)
        if value is not None and (
            not value or any(character not in allowed for character in value)
        ):
            parser.error(
                f"--{option} may contain only letters, digits, dash, underscore, and dot"
            )
    if args.cohort is None:
        args.cohort = uuid.uuid4().hex
    for option in ("baseline_command", "candidate_command"):
        try:
            command = shlex.split(getattr(args, option))
        except ValueError as error:
            parser.error(f"--{option.replace('_', '-')} is invalid: {error}")
        if not command:
            parser.error(f"--{option.replace('_', '-')} must not be empty")
        setattr(args, option, command)
    return args


def capture_command(
    capture_script: pathlib.Path,
    cohort: str,
    label: str,
    command: Sequence[str],
    power_note: str,
    output_dir: pathlib.Path,
) -> list[str]:
    return [
        sys.executable,
        str(capture_script),
        "--cohort",
        cohort,
        "--label",
        label,
        "--power-note",
        power_note,
        "--output-dir",
        str(output_dir),
        "--",
        *command,
    ]


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_arguments(argv)
    capture_script = pathlib.Path(__file__).with_name("perf_capture.py").resolve()
    commands = {
        "baseline": args.baseline_command,
        "candidate": args.candidate_command,
    }
    failed = False

    for cycle in range(1, args.cycles + 1):
        order = ("baseline", "candidate") if cycle % 2 else ("candidate", "baseline")
        for phase in order:
            label = f"{args.group}-{phase}-{cycle}"
            capture_name = f"{args.cohort}/{label}"
            invocation = capture_command(
                capture_script,
                args.cohort,
                label,
                commands[phase],
                args.power_note,
                args.output_dir,
            )
            print(
                f"performance experiment: {capture_name}: "
                f"{shlex.join(commands[phase])}",
                file=sys.stderr,
            )
            try:
                result = subprocess.run(invocation, check=False)
            except OSError as error:
                print(
                    f"performance experiment: {capture_name}: launch failed: {error}",
                    file=sys.stderr,
                )
                failed = True
            else:
                if result.returncode != 0:
                    print(
                        f"performance experiment: {capture_name}: "
                        f"exited with {result.returncode}",
                        file=sys.stderr,
                    )
                    failed = True

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
