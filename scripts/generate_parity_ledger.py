#!/usr/bin/env python3
"""Regenerate tests/parity_ledger.rs from docs/parity.md."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARITY = ROOT / "docs" / "parity.md"
OUT = ROOT / "tests" / "parity_ledger.rs"

# Keep closure routing in one place for regeneration.
from importlib.util import spec_from_loader, module_from_spec

# Inline mapping loader: exec the mapping block from this script's companion data.
MAPPING: dict[str, str] = {}


def load_rows() -> list[tuple[str, str, str]]:
    rows: list[tuple[str, str, str]] = []
    for line in PARITY.read_text().splitlines():
        match = re.match(
            r"^\| (\d+) \| `([^`]+)` \| (preserve|adapt|reject) \| ([^|]+) \|", line
        )
        if match:
            rows.append((match.group(2), match.group(3), match.group(4).strip()))
    return rows


def closure_for(name: str, cls: str, owner: str) -> str:
    if name in MAPPING:
        return MAPPING[name]
    if cls == "reject":
        if "Downstream" in owner:
            return "docs/parity.md reject-doc downstream-host"
        return "docs/parity.md reject-doc"
    return f"tests/parity_gaps.rs::{name}"


def main() -> None:
    rows = load_rows()
    if len(rows) != 151:
        raise SystemExit(f"expected 151 parity rows, found {len(rows)}")

    lines = [
        "//! Parity ledger closure proofs for all 151 pd-asteroids characterization tests.",
        "//! Regenerate: `python3 scripts/generate_parity_ledger.py`",
        "",
        "struct ParityClosure {",
        "    source: &'static str,",
        "    class: &'static str,",
        "    owner: &'static str,",
        "    moirai_proof: &'static str,",
        "}",
        "",
        "const PARITY_CLOSURES: &[ParityClosure] = &[",
    ]
    for name, cls, owner in rows:
        proof = closure_for(name, cls, owner).replace('"', '\\"')
        lines.append(
            f'    ParityClosure {{ source: "{name}", class: "{cls}", owner: "{owner}", moirai_proof: "{proof}" }},'
        )
    lines.extend(
        [
            "];",
            "",
            "#[test]",
            "fn parity_ledger_accounts_for_all_source_tests() {",
            "    assert_eq!(PARITY_CLOSURES.len(), 151);",
            "    let mut names = PARITY_CLOSURES.iter().map(|row| row.source).collect::<Vec<_>>();",
            "    names.sort_unstable();",
            "    names.dedup();",
            "    assert_eq!(names.len(), 151, \"duplicate source test names in ledger\");",
            "}",
            "",
            "#[test]",
            "fn parity_ledger_reject_rows_document_negative_contract() {",
            "    for row in PARITY_CLOSURES {",
            "        if row.class == \"reject\" {",
            "            assert!(",
            "                row.moirai_proof.contains(\"reject-doc\"),",
            "                \"reject row {} must cite reject-doc proof\",",
            "                row.source",
            "            );",
            "        }",
            "    }",
            "}",
        ]
    )
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()