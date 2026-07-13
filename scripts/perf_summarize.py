#!/usr/bin/env python3
"""Summarize paired Divan captures produced by perf_capture.py.

Example:
    uv run python scripts/perf_summarize.py /private/tmp/encosy-perf-results
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import statistics
from collections import defaultdict


LABEL_PATTERN = re.compile(r"^(?P<group>.+)-(?P<phase>baseline|candidate)-(?P<run>\d+)$")
TIME_PATTERN = re.compile(r"^(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ps|ns|us|µs|ms|s)$")
ROW_PATTERN = re.compile(
    r"^[\s├╰│─]*(?P<name>.*?)\s{2,}(?P<fastest>\d+(?:\.\d+)?\s*(?:ps|ns|us|µs|ms|s))\s*$"
)
UNIT_TO_NS = {"ps": 0.001, "ns": 1.0, "us": 1_000.0, "µs": 1_000.0, "ms": 1_000_000.0, "s": 1_000_000_000.0}


def time_ns(value: str) -> float:
    match = TIME_PATTERN.match(value.strip())
    if match is None:
        raise ValueError(f"unsupported duration: {value!r}")
    return float(match.group("value")) * UNIT_TO_NS[match.group("unit")]


def parse_stdout(path: pathlib.Path) -> dict[str, float]:
    medians: dict[str, float] = {}
    parent: str | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if ("├─" in line or "╰─" in line) and re.search(
            r"\d+(?:\.\d+)?\s*(?:ps|ns|us|µs|ms|s)", line
        ) is None:
            parent = line.split("│", 1)[0].strip(" ├╰│─") or parent
            continue
        columns = re.split(r"\s+│\s+", line)
        if len(columns) < 3 or "median" in line and "samples" in line:
            continue
        median = columns[2].strip()
        left = columns[0]
        if not median:
            parent = left.strip(" ├╰│─") or parent
            continue
        row = ROW_PATTERN.match(left)
        if row is None:
            continue
        name = row.group("name").strip().lstrip("│├╰─ ")
        key = f"{parent}/{name}" if parent and name.isdigit() else name
        medians[key] = time_ns(median)
    return medians


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_dir", nargs="?", type=pathlib.Path)
    parser.add_argument("--baseline-dir", type=pathlib.Path)
    parser.add_argument("--candidate-dir", type=pathlib.Path)
    parser.add_argument("--min-win-pct", type=float, default=5.0)
    parser.add_argument("--max-regression-pct", type=float, default=3.0)
    parser.add_argument("--required-wins", type=int, default=4)
    args = parser.parse_args()
    if args.capture_dir is not None and (args.baseline_dir or args.candidate_dir):
        parser.error("use capture_dir or the paired --baseline-dir/--candidate-dir options")
    if args.capture_dir is None and not (args.baseline_dir and args.candidate_dir):
        parser.error("provide capture_dir or both --baseline-dir and --candidate-dir")
    sources = (
        [(args.capture_dir, None)]
        if args.capture_dir is not None
        else [(args.baseline_dir, "baseline"), (args.candidate_dir, "candidate")]
    )
    samples: dict[tuple[str, str, str], dict[int, float]] = defaultdict(dict)
    for capture_dir, selected_phase in sources:
        for metadata_path in sorted(capture_dir.glob("*/metadata.json")):
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            match = LABEL_PATTERN.match(metadata["label"])
            stdout_path = metadata_path.with_name("stdout.log")
            if (
                match is None
                or selected_phase is not None and match.group("phase") != selected_phase
                or metadata.get("status") != "passed"
                or not stdout_path.exists()
            ):
                continue
            for case, median_ns in parse_stdout(stdout_path).items():
                samples[(match.group("group"), case, match.group("phase"))][
                    int(match.group("run"))
                ] = median_ns

    groups = sorted({(group, case) for group, case, _phase in samples})
    print(
        "group\tcase\tbaseline_ns\tcandidate_ns\tdelta_pct\tpairs\twins\t"
        "worst_regression_pct\tgate"
    )
    for group, case in groups:
        baseline = samples.get((group, case, "baseline"), [])
        candidate = samples.get((group, case, "candidate"), [])
        if not baseline or not candidate:
            continue
        paired_runs = sorted(baseline.keys() & candidate.keys())
        if not paired_runs:
            continue
        baseline_median = statistics.median(baseline.values())
        candidate_median = statistics.median(candidate.values())
        delta = (candidate_median / baseline_median - 1.0) * 100.0
        pair_deltas = [
            (candidate[run] / baseline[run] - 1.0) * 100.0 for run in paired_runs
        ]
        wins = sum(delta <= -args.min_win_pct for delta in pair_deltas)
        worst_regression = max(pair_deltas)
        required_wins = min(args.required_wins, len(paired_runs))
        if required_wins == 0:
            gate_passed = delta <= args.max_regression_pct
        else:
            gate_passed = wins >= required_wins and worst_regression <= args.max_regression_pct
        gate = "pass" if gate_passed else "fail"
        print(
            f"{group}\t{case}\t{baseline_median:.3f}\t{candidate_median:.3f}\t{delta:+.2f}\t"
            f"{len(paired_runs)}\t{wins}\t{worst_regression:+.2f}\t{gate}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
