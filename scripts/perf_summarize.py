#!/usr/bin/env python3
"""Descriptively summarize captures produced by perf_capture.py.

Example:
    uv run python scripts/perf_summarize.py target/perf-results
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import statistics
from collections import defaultdict
from collections.abc import Iterable, Sequence
from typing import Any


LABEL_PATTERN = re.compile(r"^(?P<group>.+)-(?P<phase>baseline|candidate)-(?P<run>\d+)$")
TIME_PATTERN = re.compile(r"^(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ps|ns|us|µs|ms|s)$")
ROW_PATTERN = re.compile(
    r"^[\s├╰│─]*(?P<name>.*?)\s{2,}(?P<fastest>\d+(?:\.\d+)?\s*(?:ps|ns|us|µs|ms|s))\s*$"
)
UNIT_TO_NS = {
    "ps": 0.001,
    "ns": 1.0,
    "us": 1_000.0,
    "µs": 1_000.0,
    "ms": 1_000_000.0,
    "s": 1_000_000_000.0,
}


def time_ns(value: str) -> float:
    match = TIME_PATTERN.match(value.strip())
    if match is None:
        raise ValueError(f"unsupported duration: {value!r}")
    return float(match.group("value")) * UNIT_TO_NS[match.group("unit")]


def parse_stdout(path: pathlib.Path) -> dict[str, float]:
    medians: dict[str, float] = {}
    parent: str | None = None
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
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


def load_one(metadata: dict[str, Any], key: str) -> str:
    values = metadata.get(key)
    if not isinstance(values, list) or not values:
        return "missing"
    value = values[0]
    return f"{value:.3f}" if isinstance(value, (int, float)) else "missing"


def optional_value(value: Any) -> str:
    return "missing" if value is None else str(value)


def median_absolute_deviation(values: Sequence[float]) -> float:
    median = statistics.median(values)
    return statistics.median(abs(value - median) for value in values)


def print_section(name: str, header: str, rows: Iterable[str]) -> None:
    print(f"[{name}]")
    print(header)
    for row in rows:
        print(row)


def percentage_delta(baseline: float, candidate: float) -> str:
    if baseline == 0:
        return "undefined"
    return f"{(candidate / baseline - 1.0) * 100.0:+.2f}"


def parse_arguments(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_dir", nargs="?", type=pathlib.Path)
    parser.add_argument("--baseline-dir", type=pathlib.Path)
    parser.add_argument("--candidate-dir", type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.capture_dir is not None and (args.baseline_dir or args.candidate_dir):
        parser.error("use capture_dir or the paired --baseline-dir/--candidate-dir options")
    if args.capture_dir is None and not (args.baseline_dir and args.candidate_dir):
        parser.error("provide capture_dir or both --baseline-dir and --candidate-dir")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_arguments(argv)
    sources = (
        [(args.capture_dir, None)]
        if args.capture_dir is not None
        else [(args.baseline_dir, "baseline"), (args.candidate_dir, "candidate")]
    )
    samples: dict[tuple[str, str, str, str, str], list[float]] = defaultdict(list)
    paired_samples: dict[
        tuple[str, str, str, int, str], list[tuple[str, str, float]]
    ] = defaultdict(list)
    inventory: list[dict[str, Any]] = []

    for capture_dir, selected_phase in sources:
        assert capture_dir is not None
        for metadata_path in sorted(capture_dir.glob("*/metadata.json")):
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            match = LABEL_PATTERN.match(str(metadata.get("label", "")))
            if match is not None and selected_phase is not None and match.group("phase") != selected_phase:
                continue
            stdout_path = metadata_path.with_name("stdout.log")
            parsed = parse_stdout(stdout_path) if stdout_path.exists() else {}
            cohort_value = metadata.get("cohort")
            cohort = (
                cohort_value
                if isinstance(cohort_value, str) and cohort_value
                else "uncohorted"
            )
            entry = {
                "capture": metadata_path.parent.name,
                "cohort": cohort,
                "label": metadata.get("label"),
                "group": match.group("group") if match else None,
                "phase": match.group("phase") if match else None,
                "run": int(match.group("run")) if match else None,
                "status": metadata.get("status"),
                "exit_code": metadata.get("exit_code"),
                "duration_seconds": metadata.get("duration_seconds"),
                "load_start": load_one(metadata, "load_average_at_start"),
                "load_end": load_one(metadata, "load_average_at_end"),
                "cases": len(parsed),
                "error": metadata.get("error"),
            }
            inventory.append(entry)
            if match is None:
                continue
            for case, median_ns in parsed.items():
                group = match.group("group")
                phase = match.group("phase")
                run = int(match.group("run"))
                status = str(metadata.get("status", "missing"))
                samples[(cohort, group, case, phase, status)].append(median_ns)
                paired_samples[(cohort, group, case, run, phase)].append(
                    (metadata_path.parent.name, status, median_ns)
                )

    inventory_rows = [
        "\t".join(
            optional_value(entry[key])
            for key in (
                "capture",
                "cohort",
                "label",
                "group",
                "phase",
                "run",
                "status",
                "exit_code",
                "duration_seconds",
                "load_start",
                "load_end",
                "cases",
            )
        )
        for entry in inventory
    ]
    print_section(
        "capture_inventory",
        "capture\tcohort\tlabel\tgroup\tphase\trun\tstatus\texit_code\tduration_seconds\tload_one_start\tload_one_end\tparsed_cases",
        inventory_rows,
    )

    failures = [entry for entry in inventory if entry["status"] != "passed"]
    print()
    print_section(
        "capture_failures",
        "capture\tlabel\tstatus\texit_code\terror",
        (
            "\t".join(
                optional_value(entry[key])
                for key in ("capture", "label", "status", "exit_code", "error")
            )
            for entry in failures
        ),
    )

    print()
    print_section(
        "statistics",
        "cohort\tgroup\tcase\tphase\tstatus\tsamples\tmedian_ns\tmad_ns\tmin_ns\tmax_ns",
        (
            f"{cohort}\t{group}\t{case}\t{phase}\t{status}\t{len(values)}\t{statistics.median(values):.3f}\t"
            f"{median_absolute_deviation(values):.3f}\t{min(values):.3f}\t{max(values):.3f}"
            for (cohort, group, case, phase, status), values in sorted(samples.items())
            if values
        ),
    )

    pairs: list[
        tuple[str, str, str, int, int, str, str, str, str, float, float]
    ] = []
    pair_keys = {
        (cohort, group, case, run)
        for cohort, group, case, run, _phase in paired_samples
    }
    for cohort, group, case, run in sorted(pair_keys):
        baselines = paired_samples.get((cohort, group, case, run, "baseline"), [])
        candidates = paired_samples.get((cohort, group, case, run, "candidate"), [])
        for pair_index, (baseline, candidate) in enumerate(
            zip(baselines, candidates, strict=False), start=1
        ):
            baseline_capture, baseline_status, baseline_ns = baseline
            candidate_capture, candidate_status, candidate_ns = candidate
            pairs.append(
                (
                    cohort,
                    group,
                    case,
                    run,
                    pair_index,
                    baseline_capture,
                    baseline_status,
                    candidate_capture,
                    candidate_status,
                    baseline_ns,
                    candidate_ns,
                )
            )

    print()
    print_section(
        "paired_deltas",
        "cohort\tgroup\tcase\trun\tpair\tbaseline_capture\tbaseline_status\tcandidate_capture\tcandidate_status\tbaseline_ns\tcandidate_ns\tdelta_ns\tdelta_pct",
        (
            f"{cohort}\t{group}\t{case}\t{run}\t{pair_index}\t{baseline_capture}\t{baseline_status}\t"
            f"{candidate_capture}\t{candidate_status}\t{baseline:.3f}\t{candidate:.3f}\t"
            f"{candidate - baseline:+.3f}\t{percentage_delta(baseline, candidate)}"
            for (
                cohort,
                group,
                case,
                run,
                pair_index,
                baseline_capture,
                baseline_status,
                candidate_capture,
                candidate_status,
                baseline,
                candidate,
            ) in pairs
        ),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
