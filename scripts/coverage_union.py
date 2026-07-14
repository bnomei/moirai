#!/usr/bin/env python3
"""Build the production source-line gate from a merged cargo-llvm-cov JSON export."""

from __future__ import annotations

import argparse
import bisect
import hashlib
import json
import os
import re
import subprocess
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


CFG_ATTRIBUTE = re.compile(r"#\s*\[\s*cfg\s*\(")
EXTERNAL_MOD = re.compile(
    r"^(?:(?:pub(?:\s*\([^)]*\))?)\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;",
    re.DOTALL,
)
ANY_EXTERNAL_MOD = re.compile(
    r"(?m)^\s*(?:(?:pub(?:\s*\([^)]*\))?)\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"
)
REQUIRED_FLAVORS = {"no-default", "std", "testkit", "all-features"}
WHOLE_FILE_EXCLUSIONS = {
    "src/bench_internals.rs": "benchmark-only implementation enabled by the bench-internals feature",
}
WHOLE_DIRECTORY_EXCLUSIONS = {
    "src/examples": "documentation-only lessons validated separately through stable doctests",
}


@dataclass(frozen=True)
class ItemSpan:
    start: int
    end: int
    external_module: str | None = None


CfgExpr = tuple[str, object]


class _CfgParser:
    """Small fail-closed parser for Rust cfg predicate expressions."""

    def __init__(self, source: str) -> None:
        self.source = source
        self.index = 0

    def _skip_space(self) -> None:
        while self.index < len(self.source):
            if self.source[self.index].isspace():
                self.index += 1
            elif self.source.startswith("//", self.index):
                newline = self.source.find("\n", self.index + 2)
                self.index = len(self.source) if newline < 0 else newline + 1
            elif self.source.startswith("/*", self.index):
                depth = 1
                self.index += 2
                while self.index < len(self.source) and depth:
                    if self.source.startswith("/*", self.index):
                        depth += 1
                        self.index += 2
                    elif self.source.startswith("*/", self.index):
                        depth -= 1
                        self.index += 2
                    else:
                        self.index += 1
                if depth:
                    raise ValueError("unterminated block comment in cfg predicate")
            else:
                return

    def _identifier(self) -> str:
        self._skip_space()
        match = re.match(r"[A-Za-z_][A-Za-z0-9_]*", self.source[self.index :])
        if not match:
            raise ValueError(f"expected identifier in cfg predicate at offset {self.index}")
        self.index += match.end()
        return match.group(0)

    def _string(self) -> str:
        self._skip_space()
        raw = re.match(r"r(?P<hashes>#{0,255})\"", self.source[self.index :])
        if raw:
            terminator = '"' + raw.group("hashes")
            body_start = self.index + raw.end()
            end = self.source.find(terminator, body_start)
            if end < 0:
                raise ValueError("unterminated raw string in cfg predicate")
            value = self.source[body_start:end]
            self.index = end + len(terminator)
            return value
        if self.index >= len(self.source) or self.source[self.index] != '"':
            raise ValueError(f"expected string in cfg predicate at offset {self.index}")
        start = self.index
        self.index += 1
        escaped = False
        while self.index < len(self.source):
            char = self.source[self.index]
            self.index += 1
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                try:
                    return json.loads(self.source[start : self.index])
                except json.JSONDecodeError as error:
                    raise ValueError("unsupported escape in cfg predicate string") from error
        raise ValueError("unterminated string in cfg predicate")

    def _take(self, expected: str) -> bool:
        self._skip_space()
        if self.source.startswith(expected, self.index):
            self.index += len(expected)
            return True
        return False

    def expression(self) -> CfgExpr:
        name = self._identifier()
        if self._take("="):
            return ("atom", f"{name}={self._string()!r}")
        if not self._take("("):
            return ("atom", name)
        if name not in {"all", "any", "not"}:
            raise ValueError(f"unsupported cfg predicate operator: {name}")
        children: list[CfgExpr] = []
        self._skip_space()
        if not self._take(")"):
            while True:
                children.append(self.expression())
                if self._take(")"):
                    break
                if not self._take(","):
                    raise ValueError("expected ',' or ')' in cfg predicate")
                self._skip_space()
                if self._take(")"):
                    break
        if name == "not" and len(children) != 1:
            raise ValueError("cfg(not(...)) requires exactly one predicate")
        return (name, tuple(children))

    def parse(self) -> CfgExpr:
        expression = self.expression()
        self._skip_space()
        if self.index != len(self.source):
            raise ValueError(f"unexpected cfg predicate input at offset {self.index}")
        return expression


def _cfg_atoms(expression: CfgExpr) -> set[str]:
    kind, value = expression
    if kind == "atom":
        return {str(value)}
    atoms: set[str] = set()
    for child in value:
        atoms.update(_cfg_atoms(child))
    return atoms


def _eval_cfg(expression: CfgExpr, values: dict[str, bool]) -> bool:
    kind, value = expression
    if kind == "atom":
        return values[str(value)]
    children = tuple(_eval_cfg(child, values) for child in value)
    if kind == "all":
        return all(children)
    if kind == "any":
        return any(children)
    if kind == "not":
        return not children[0]
    raise ValueError(f"unsupported cfg expression kind: {kind}")


def _cfg_possible_without_test(source: str) -> bool:
    expression = _CfgParser(source).parse()
    atoms = sorted(_cfg_atoms(expression).difference({"test"}))
    if len(atoms) > 16:
        raise ValueError("cfg predicate has too many independent atoms to audit")
    for mask in range(1 << len(atoms)):
        values = {"test": False}
        values.update({atom: bool(mask & (1 << index)) for index, atom in enumerate(atoms)})
        if _eval_cfg(expression, values):
            return True
    return False


def _run(*args: str, cwd: Path) -> str:
    return subprocess.run(
        args,
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    ).stdout.strip()


def _mask_rust(source: str) -> str:
    """Mask comments and literals while retaining byte positions and newlines."""
    chars = list(source)
    length = len(source)

    def blank(start: int, end: int) -> None:
        for index in range(start, end):
            if chars[index] != "\n":
                chars[index] = " "

    index = 0
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            end = length if end < 0 else end
            blank(index, end)
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            blank(index, end)
            index = end
            continue

        raw = re.match(r"(?:br|cr|r)(?P<hashes>#{0,255})\"", source[index:])
        if raw:
            terminator = '"' + raw.group("hashes")
            body_start = index + raw.end()
            found = source.find(terminator, body_start)
            end = length if found < 0 else found + len(terminator)
            blank(index, end)
            index = end
            continue

        prefix = 1 if source.startswith(('b"', 'c"'), index) else 0
        if source[index + prefix : index + prefix + 1] == '"':
            end = index + prefix + 1
            escaped = False
            while end < length:
                char = source[end]
                end += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
            blank(index, end)
            index = end
            continue

        char_prefix = 1 if source.startswith("b'", index) else 0
        if source[index + char_prefix : index + char_prefix + 1] == "'":
            # A lifetime has no closing quote. Only mask a short, valid-looking char literal.
            end = index + char_prefix + 1
            if end < length and source[end] == "\\":
                end += 2
                if end < length and source[end - 1] == "u" and source[end] == "{":
                    close = source.find("}", end + 1)
                    end = length if close < 0 else close + 1
            else:
                end += 1
            if end < length and source[end] == "'":
                end += 1
                blank(index, end)
                index = end
                continue
        index += 1
    return "".join(chars)


def _matching(masked: str, start: int, opening: str, closing: str) -> int | None:
    depth = 0
    for index in range(start, len(masked)):
        char = masked[index]
        if char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index
    return None


def _skip_outer_attributes(masked: str, start: int) -> int:
    index = start
    while True:
        while index < len(masked) and masked[index].isspace():
            index += 1
        if index >= len(masked) or masked[index] != "#":
            return index
        bracket = index + 1
        while bracket < len(masked) and masked[bracket].isspace():
            bracket += 1
        if bracket >= len(masked) or masked[bracket] != "[":
            return index
        end = _matching(masked, bracket, "[", "]")
        if end is None:
            return index
        index = end + 1


def cfg_test_only_item_spans(source: str) -> list[ItemSpan]:
    """Return items whose cfg predicate cannot be true when `test` is false."""
    masked = _mask_rust(source)
    spans: list[ItemSpan] = []
    for match in CFG_ATTRIBUTE.finditer(masked):
        open_paren = match.end() - 1
        close_paren = _matching(masked, open_paren, "(", ")")
        if close_paren is None:
            raise ValueError("unbalanced cfg predicate")
        bracket = close_paren + 1
        while bracket < len(masked) and masked[bracket].isspace():
            bracket += 1
        if bracket >= len(masked) or masked[bracket] != "]":
            raise ValueError("cfg predicate is not followed by a closing attribute bracket")
        predicate = source[open_paren + 1 : close_paren]
        if _cfg_possible_without_test(predicate):
            continue
        item_start = _skip_outer_attributes(masked, bracket + 1)
        parens = 0
        brackets = 0
        item_end: int | None = None
        for index in range(item_start, len(masked)):
            char = masked[index]
            if char == "(":
                parens += 1
            elif char == ")":
                parens = max(0, parens - 1)
            elif char == "[":
                brackets += 1
            elif char == "]":
                brackets = max(0, brackets - 1)
            elif not parens and not brackets and char == "{":
                close = _matching(masked, index, "{", "}")
                item_end = len(masked) if close is None else close + 1
                break
            elif not parens and not brackets and char == ";":
                item_end = index + 1
                break
        if item_end is None:
            raise ValueError("unbalanced test-only cfg item")
        item_text = masked[item_start:item_end]
        module_match = EXTERNAL_MOD.match(item_text)
        spans.append(
            ItemSpan(
                match.start(),
                item_end,
                module_match.group(1) if module_match else None,
            )
        )
    return spans


def _module_candidates(parent: Path, module: str) -> tuple[Path, Path]:
    base = parent.parent if parent.name in {"lib.rs", "main.rs", "mod.rs"} else parent.parent / parent.stem
    return base / f"{module}.rs", base / module / "mod.rs"


def _is_within(path: Path, directory: Path) -> bool:
    try:
        path.relative_to(directory)
    except ValueError:
        return False
    return True


def _excluded_source_directories(repo: Path) -> dict[Path, str]:
    exclusions: dict[Path, str] = {}
    for relative, reason in WHOLE_DIRECTORY_EXCLUSIONS.items():
        directory = (repo / relative).resolve()
        if not directory.is_dir():
            raise ValueError(f"configured coverage exclusion directory is missing: {relative}")
        if not any(directory.rglob("*.rs")):
            raise ValueError(f"configured coverage exclusion directory has no Rust files: {relative}")
        exclusions[directory] = reason
    return exclusions


def _reported_path(filename: str, repo: Path) -> Path:
    path = Path(filename)
    if not path.is_absolute():
        path = repo / path
    return path.resolve()


def _excluded_report_directory(
    filename: str, repo: Path, excluded_directories: dict[Path, str]
) -> Path | None:
    path = _reported_path(filename, repo)
    return next(
        (directory for directory in excluded_directories if _is_within(path, directory)),
        None,
    )


def external_test_modules(src_root: Path, excluded_directories: set[Path]) -> set[Path]:
    """Resolve test-only cfg external modules, including their external descendants."""
    excluded: set[Path] = set()
    queue: list[Path] = []
    for source_path in sorted(src_root.rglob("*.rs")):
        resolved_source = source_path.resolve()
        if any(_is_within(resolved_source, directory) for directory in excluded_directories):
            continue
        source = source_path.read_text(encoding="utf-8")
        for span in cfg_test_only_item_spans(source):
            if not span.external_module:
                continue
            candidates = _module_candidates(source_path, span.external_module)
            resolved = next((path for path in candidates if path.is_file()), None)
            if resolved is None:
                joined = " or ".join(str(path) for path in candidates)
                raise ValueError(
                    f"cannot resolve test-only cfg module {span.external_module}: {joined}"
                )
            queue.append(resolved.resolve())

    while queue:
        source_path = queue.pop()
        if source_path in excluded:
            continue
        excluded.add(source_path)
        masked = _mask_rust(source_path.read_text(encoding="utf-8"))
        for match in ANY_EXTERNAL_MOD.finditer(masked):
            candidates = _module_candidates(source_path, match.group(1))
            resolved = next((path for path in candidates if path.is_file()), None)
            if resolved is not None:
                queue.append(resolved.resolve())
    return excluded


def _excluded_lines(source: str) -> tuple[set[int], int]:
    starts = [0]
    starts.extend(index + 1 for index, char in enumerate(source) if char == "\n")
    lines: set[int] = set()
    spans = cfg_test_only_item_spans(source)
    for span in spans:
        first = bisect.bisect_right(starts, span.start)
        last = bisect.bisect_right(starts, max(span.start, span.end - 1))
        lines.update(range(first, last + 1))
    return lines, len(spans)


def _source_path(filename: str, repo: Path, src_root: Path) -> Path | None:
    path = Path(filename)
    if not path.is_absolute():
        path = repo / path
    try:
        path = path.resolve()
        path.relative_to(src_root)
    except (OSError, ValueError):
        return None
    return path if path.suffix == ".rs" and path.is_file() else None


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _validate_llvm_json(
    path: Path, repo: Path, excluded_directories: dict[Path, str]
) -> None:
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict) or document.get("type") != "llvm.coverage.json.export":
        raise ValueError(f"unsupported LLVM coverage JSON type: {path}")
    version = document.get("version")
    if not isinstance(version, str) or not re.fullmatch(r"3\.\d+\.\d+", version):
        raise ValueError(f"unsupported LLVM coverage JSON version {version!r}: {path}")
    datasets = document.get("data")
    if not isinstance(datasets, list) or not datasets:
        raise ValueError(f"LLVM coverage JSON has no data sets: {path}")
    for dataset in datasets:
        if not isinstance(dataset, dict):
            raise ValueError(f"invalid LLVM coverage JSON data set: {path}")
        files = dataset.get("files")
        if not isinstance(files, list):
            raise ValueError(f"LLVM coverage JSON data set has no files list: {path}")
        for source_file in files:
            if not isinstance(source_file, dict) or not isinstance(
                source_file.get("filename"), str
            ):
                raise ValueError(f"invalid LLVM coverage JSON file record: {path}")
            filename = source_file["filename"]
            directory = _excluded_report_directory(filename, repo, excluded_directories)
            if directory is not None:
                relative = directory.relative_to(repo).as_posix()
                raise ValueError(
                    f"LLVM coverage JSON contains excluded source directory {relative}: {filename}"
                )
        if not isinstance(dataset.get("functions"), list):
            raise ValueError(f"LLVM coverage JSON data set has no functions list: {path}")
        if not isinstance(dataset.get("totals"), dict):
            raise ValueError(f"LLVM coverage JSON data set has no totals object: {path}")


def _source_input_evidence(repo: Path) -> dict[str, object]:
    result = subprocess.run(
        (
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ),
        cwd=repo,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    raw_paths = sorted(set(path for path in result.stdout.split(b"\0") if path))
    if not raw_paths:
        raise ValueError("git-visible source-input set is empty")

    records: list[dict[str, str]] = []
    for raw_path in raw_paths:
        relative = Path(os.fsdecode(raw_path))
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"unsafe git-visible source-input path: {relative}")
        path = repo / relative
        try:
            path.lstat()
        except FileNotFoundError:
            # `git ls-files --cached` intentionally includes tracked deletions. Keep
            # those deletions in the dirty-tree fingerprint so a coverage run can
            # validate a legitimate API removal without silently hashing HEAD's copy.
            records.append({"path": relative.as_posix(), "state": "deleted"})
            continue
        if not path.is_file():
            raise ValueError(f"git-visible source input is not a readable file: {relative}")
        records.append(
            {"path": relative.as_posix(), "state": "present", "sha256": _sha256(path)}
        )

    aggregate = hashlib.sha256()
    for raw_path, record in zip(raw_paths, records, strict=True):
        aggregate.update(raw_path)
        aggregate.update(b"\0")
        aggregate.update(record["state"].encode("ascii"))
        aggregate.update(b"\0")
        aggregate.update(record.get("sha256", "").encode("ascii"))
        aggregate.update(b"\n")
    return {"sha256": aggregate.hexdigest(), "files": records}


def _whole_file_exclusions(repo: Path) -> dict[Path, str]:
    exclusions: dict[Path, str] = {}
    for relative, reason in WHOLE_FILE_EXCLUSIONS.items():
        path = (repo / relative).resolve()
        if not path.is_file():
            raise ValueError(f"configured whole-file coverage exclusion is missing: {relative}")
        exclusions[path] = reason
    return exclusions


def _parse_lcov(
    path: Path,
    repo: Path,
    src_root: Path,
    exclusions: dict[Path, set[int]],
    external: set[Path],
    excluded_directories: dict[Path, str],
) -> dict[tuple[Path, int], bool]:
    lines: dict[tuple[Path, int], bool] = {}
    current: Path | None = None
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        if raw_line.startswith("SF:"):
            filename = raw_line[3:]
            directory = _excluded_report_directory(filename, repo, excluded_directories)
            if directory is not None:
                relative = directory.relative_to(repo).as_posix()
                raise ValueError(
                    f"LCOV contains excluded source directory {relative}: {filename}"
                )
            current = _source_path(filename, repo, src_root)
            if current in external:
                current = None
        elif raw_line.startswith("DA:") and current is not None:
            fields = raw_line[3:].split(",", 2)
            if len(fields) < 2:
                raise ValueError(f"invalid DA record in {path}: {raw_line}")
            line = int(fields[0])
            count = int(fields[1])
            if line in exclusions[current]:
                continue
            key = (current, line)
            lines[key] = lines.get(key, False) or count > 0
        elif raw_line == "end_of_record":
            current = None
    return lines


def _line_range(source: str, span: ItemSpan) -> tuple[int, int]:
    starts = [0]
    starts.extend(index + 1 for index, char in enumerate(source) if char == "\n")
    return (
        bisect.bisect_right(starts, span.start),
        bisect.bisect_right(starts, max(span.start, span.end - 1)),
    )


def _source_audit(
    repo: Path, src_root: Path
) -> tuple[
    dict[Path, set[int]],
    set[Path],
    dict[Path, str],
    dict[Path, str],
    list[dict[str, object]],
]:
    exclusions: dict[Path, set[int]] = {}
    items: list[dict[str, object]] = []
    whole_directories = _excluded_source_directories(repo)
    for source_path in sorted(src_root.rglob("*.rs")):
        resolved = source_path.resolve()
        if any(_is_within(resolved, directory) for directory in whole_directories):
            continue
        source = source_path.read_text(encoding="utf-8")
        spans = cfg_test_only_item_spans(source)
        lines, _count = _excluded_lines(source)
        exclusions[resolved] = lines
        for span in spans:
            start_line, end_line = _line_range(source, span)
            items.append(
                {
                    "path": resolved.relative_to(repo).as_posix(),
                    "start_line": start_line,
                    "end_line": end_line,
                    "external_module": span.external_module,
                }
            )
    return (
        exclusions,
        external_test_modules(src_root, set(whole_directories)),
        _whole_file_exclusions(repo),
        whole_directories,
        items,
    )


def _totals(lines: dict[tuple[Path, int], bool]) -> dict[str, float | int]:
    executable = len(lines)
    covered = sum(lines.values())
    return {
        "executable": executable,
        "covered": covered,
        "missed": executable - covered,
        "percent": 100.0 if executable == 0 else covered * 100.0 / executable,
    }


def analyze(args: argparse.Namespace) -> None:
    repo = args.repo.resolve()
    src_root = (repo / "src").resolve()
    source_inputs = _source_input_evidence(repo)
    if args.verified_source_digest != source_inputs["sha256"]:
        raise ValueError("verified source-input digest does not match analyzer inputs")
    exclusions, external, whole_files, whole_directories, cfg_items = _source_audit(
        repo, src_root
    )
    excluded_files = external.union(whole_files)

    flavor_lines: dict[str, dict[tuple[Path, int], bool]] = {}
    artifact_paths: dict[str, Path] = {}
    names = [name for name, _lcov_path, _json_path in args.flavor]
    if len(names) != len(set(names)):
        raise ValueError("duplicate coverage flavor")
    if set(names) != REQUIRED_FLAVORS:
        missing = sorted(REQUIRED_FLAVORS.difference(names))
        extra = sorted(set(names).difference(REQUIRED_FLAVORS))
        raise ValueError(f"coverage flavors must be exact; missing={missing}, extra={extra}")
    for name, lcov_path, json_path in args.flavor:
        lcov_path = Path(lcov_path)
        json_path = Path(json_path)
        # Full JSON is retained as independently reproducible evidence. Parse it now so a
        # truncated export can never be published beside an otherwise valid LCOV gate.
        _validate_llvm_json(json_path, repo, whole_directories)
        flavor_lines[name] = _parse_lcov(
            lcov_path,
            repo,
            src_root,
            exclusions,
            excluded_files,
            whole_directories,
        )
        if not flavor_lines[name]:
            raise ValueError(f"coverage flavor has an empty production LCOV map: {name}")
        artifact_paths[f"flavors/{name}.lcov"] = lcov_path
        artifact_paths[f"flavors/{name}.json"] = json_path

    executable: dict[tuple[Path, int], bool] = {}
    for lines in flavor_lines.values():
        for key, covered in lines.items():
            executable[key] = executable.get(key, False) or covered
    if not executable:
        raise ValueError("production source-line union denominator is empty")

    file_rows: list[dict[str, object]] = []
    missing_rows: list[tuple[str, int]] = []
    for source_path in sorted({path for path, _line in executable}):
        relative = source_path.relative_to(repo).as_posix()
        lines = sorted(line for path, line in executable if path == source_path)
        missing = [line for line in lines if not executable[(source_path, line)]]
        missing_rows.extend((relative, line) for line in missing)
        file_rows.append(
            {
                "path": relative,
                "executable": len(lines),
                "covered": len(lines) - len(missing),
                "missed": len(missing),
            }
        )

    totals = _totals(executable)
    summary = {
        "schema_version": 1,
        "gate": "production-source-line-union",
        "definition": "unique LCOV DA source lines under src/, covered when any independent feature flavor has a positive count",
        "totals": totals,
        "flavors": {name: _totals(lines) for name, lines in sorted(flavor_lines.items())},
        "exclusions": {
            "policy": "cfg items impossible with test=false are balanced independently; their external modules are excluded transitively; documentation-only lesson directories are excluded from instrumentation reports",
            "test_only_cfg_items": len(cfg_items),
            "external_module_files": sorted(path.relative_to(repo).as_posix() for path in external),
            "inline_source_lines": sum(len(lines) for lines in exclusions.values()),
            "whole_files": [
                {"path": path.relative_to(repo).as_posix(), "reason": reason}
                for path, reason in sorted(whole_files.items())
            ],
            "whole_directories": [
                {"path": path.relative_to(repo).as_posix(), "reason": reason}
                for path, reason in sorted(whole_directories.items())
            ],
        },
        "files": file_rows,
    }
    args.summary.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.missing.write_text(
        "".join(f"{path}:{line}\n" for path, line in sorted(missing_rows)),
        encoding="utf-8",
    )

    status = _run("git", "status", "--porcelain=v1", "--untracked-files=all", cwd=repo).splitlines()
    artifacts = {
        name: {"sha256": _sha256(path)} for name, path in sorted(artifact_paths.items())
    }
    artifacts.update(
        {
            "summary.json": {"sha256": _sha256(args.summary)},
            "missing-lines.txt": {"sha256": _sha256(args.missing)},
        }
    )
    if _source_input_evidence(repo)["sha256"] != source_inputs["sha256"]:
        raise ValueError("source inputs changed while coverage artifacts were analyzed")
    manifest = {
        "schema_version": 1,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "git": {
            "revision": _run("git", "rev-parse", "HEAD", cwd=repo),
            "dirty": bool(status),
            "status": status,
        },
        "versions": {
            "cargo": _run("cargo", "--version", cwd=repo),
            "cargo_llvm_cov": _run("cargo", "llvm-cov", "--version", cwd=repo),
            "rustc": _run("rustc", "-vV", cwd=repo),
        },
        "source_inputs": {
            **source_inputs,
            "verified_before_and_after_flavors": args.verified_source_digest,
        },
        "commands": args.command,
        "artifacts": artifacts,
        "gate": summary["totals"],
        "exclusions": summary["exclusions"],
        "test_only_cfg_audit": cfg_items,
    }
    args.manifest.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def self_test() -> None:
    sample = '''
fn production_before() {}
// #[cfg(test)] fn fake() {}
const TEXT: &str = "#[cfg(test)] fn fake() {}";
#[cfg(test)]
fn helper<T: Copy>() { let _ = "}"; }
fn production_between() {}
#[cfg(all(test, feature = "std"))]
#[allow(dead_code)]
mod tests { fn nested() { if true { } } }
#[cfg(any(test, feature = "testkit"))]
fn shipped_testkit_code() {}
#[cfg(not(test))]
fn non_test_code() {}
fn production_after() {}
'''
    spans = cfg_test_only_item_spans(sample)
    assert len(spans) == 2
    excluded, _ = _excluded_lines(sample)
    lines = sample.splitlines()
    assert all(not lines[line - 1].startswith("fn production") for line in excluded)
    assert all("shipped_testkit_code" not in lines[line - 1] for line in excluded)
    assert all("non_test_code" not in lines[line - 1] for line in excluded)
    assert not _cfg_possible_without_test("all(test, feature = \"std\")")
    assert _cfg_possible_without_test("any(test, feature = \"testkit\")")
    assert _cfg_possible_without_test("not(test)")

    with tempfile.TemporaryDirectory() as directory:
        src = Path(directory) / "src"
        (src / "thing" / "tests").mkdir(parents=True)
        (src / "thing.rs").write_text("#[cfg(test)]\nmod tests;\n", encoding="utf-8")
        (src / "thing" / "tests.rs").write_text("mod child;\n", encoding="utf-8")
        child = src / "thing" / "tests" / "child.rs"
        child.write_text("fn test_only() {}\n", encoding="utf-8")
        assert external_test_modules(src, set()) == {
            (src / "thing" / "tests.rs").resolve(),
            child.resolve(),
        }

        repo = Path(directory).resolve()
        (src / "lib.rs").write_text(sample, encoding="utf-8")
        benchmark_only = src / "bench_internals.rs"
        benchmark_only.write_text("fn benchmark_only() {}\n", encoding="utf-8")
        lesson_directory = src / "examples"
        lesson_directory.mkdir()
        lesson = lesson_directory / "a01_world.rs"
        lesson.write_text("//! Documentation-only lesson.\n", encoding="utf-8")
        exclusions, external, whole_files, whole_directories, _items = _source_audit(repo, src)
        assert whole_files == {
            benchmark_only.resolve(): "benchmark-only implementation enabled by the bench-internals feature"
        }
        assert whole_directories == {
            lesson_directory.resolve(): "documentation-only lessons validated separately through stable doctests"
        }
        production_before = next(
            index for index, line in enumerate(sample.splitlines(), 1) if line.startswith("fn production_before")
        )
        production_after = next(
            index for index, line in enumerate(sample.splitlines(), 1) if line.startswith("fn production_after")
        )
        helper = next(
            index for index, line in enumerate(sample.splitlines(), 1) if line.startswith("fn helper")
        )
        flavor_a = repo / "a.lcov"
        flavor_b = repo / "b.lcov"
        flavor_a.write_text(
            f"SF:{src / 'lib.rs'}\nDA:{production_before},1\nDA:{production_after},0\n"
            f"DA:{helper},1\nSF:{src / 'thing' / 'tests.rs'}\nDA:1,1\n"
            f"SF:{benchmark_only}\nDA:1,1\nend_of_record\n",
            encoding="utf-8",
        )
        flavor_b.write_text(
            f"SF:{src / 'lib.rs'}\nDA:{production_before},0\nDA:{production_after},2\nend_of_record\n",
            encoding="utf-8",
        )
        excluded_files = external.union(whole_files)
        first = _parse_lcov(
            flavor_a,
            repo,
            src.resolve(),
            exclusions,
            excluded_files,
            whole_directories,
        )
        second = _parse_lcov(
            flavor_b,
            repo,
            src.resolve(),
            exclusions,
            excluded_files,
            whole_directories,
        )
        union = dict(first)
        for key, covered in second.items():
            union[key] = union.get(key, False) or covered
        assert _totals(union) == {
            "executable": 2,
            "covered": 2,
            "missed": 0,
            "percent": 100.0,
        }
        valid_json = repo / "valid.json"
        valid_json.write_text(
            json.dumps(
                {
                    "type": "llvm.coverage.json.export",
                    "version": "3.1.0",
                    "data": [{"files": [], "functions": [], "totals": {}}],
                }
            ),
            encoding="utf-8",
        )
        _validate_llvm_json(valid_json, repo, whole_directories)
        invalid_json = repo / "invalid.json"
        invalid_json.write_text("{}", encoding="utf-8")
        try:
            _validate_llvm_json(invalid_json, repo, whole_directories)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid LLVM coverage JSON must fail closed")

        excluded_lcov = repo / "excluded.lcov"
        excluded_lcov.write_text(f"SF:{lesson}\nDA:1,1\nend_of_record\n", encoding="utf-8")
        try:
            _parse_lcov(
                excluded_lcov,
                repo,
                src.resolve(),
                exclusions,
                excluded_files,
                whole_directories,
            )
        except ValueError:
            pass
        else:
            raise AssertionError("LCOV lesson records must fail closed")

        excluded_json = repo / "excluded.json"
        excluded_json.write_text(
            json.dumps(
                {
                    "type": "llvm.coverage.json.export",
                    "version": "3.1.0",
                    "data": [
                        {
                            "files": [{"filename": str(lesson)}],
                            "functions": [],
                            "totals": {},
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        try:
            _validate_llvm_json(excluded_json, repo, whole_directories)
        except ValueError:
            pass
        else:
            raise AssertionError("LLVM JSON lesson records must fail closed")

        (repo / "Cargo.toml").write_text("[package]\nname='digest-test'\n", encoding="utf-8")
        scripts = repo / "scripts"
        scripts.mkdir()
        for name in ("coverage_union.py", "verify_coverage_union.sh", "verify_phase6.sh"):
            (scripts / name).write_text(name + "\n", encoding="utf-8")
        (repo / ".gitignore").write_text("/target/\n", encoding="utf-8")
        tests = repo / "tests"
        tests.mkdir()
        tracked_test = tests / "coverage_input.rs"
        tracked_test.write_text("fn tracked_input() {}\n", encoding="utf-8")
        subprocess.run(("git", "init", "-q"), cwd=repo, check=True)
        subprocess.run(
            (
                "git",
                "add",
                ".gitignore",
                "Cargo.toml",
                "scripts",
                "src",
                "tests/coverage_input.rs",
            ),
            cwd=repo,
            check=True,
        )

        first_digest = _source_input_evidence(repo)
        assert _source_input_evidence(repo) == first_digest
        tracked_test.write_text("fn tracked_input_changed() {}\n", encoding="utf-8")
        tracked_changed = _source_input_evidence(repo)
        assert tracked_changed["sha256"] != first_digest["sha256"]

        tracked_test.unlink()
        tracked_deleted = _source_input_evidence(repo)
        assert tracked_deleted["sha256"] != tracked_changed["sha256"]
        assert {
            "path": "tests/coverage_input.rs",
            "state": "deleted",
        } in tracked_deleted["files"]

        fixture = repo / "fixtures" / "runtime.txt"
        fixture.parent.mkdir()
        fixture.write_text("first\n", encoding="utf-8")
        untracked_first = _source_input_evidence(repo)
        fixture.write_text("second\n", encoding="utf-8")
        untracked_second = _source_input_evidence(repo)
        assert untracked_second["sha256"] != untracked_first["sha256"]

        ignored = repo / "target" / "coverage" / "artifact.json"
        ignored.parent.mkdir(parents=True)
        ignored.write_text("generated\n", encoding="utf-8")
        assert _source_input_evidence(repo) == untracked_second


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--audit-only", action="store_true")
    parser.add_argument("--source-digest", action="store_true")
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument(
        "--flavor",
        action="append",
        nargs=3,
        default=[],
        metavar=("NAME", "LCOV", "JSON"),
    )
    parser.add_argument("--summary", type=Path)
    parser.add_argument("--missing", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--verified-source-digest")
    parser.add_argument("--command", action="append", default=[])
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.self_test:
        self_test()
        return
    if args.audit_only:
        repo = args.repo.resolve()
        _exclusions, external, whole_files, whole_directories, items = _source_audit(
            repo, (repo / "src").resolve()
        )
        print(
            f"audited {len(items)} test-only cfg items, {len(external)} external module files, "
            f"{len(whole_files)} whole-file exclusions, and "
            f"{len(whole_directories)} whole-directory exclusions"
        )
        return
    if args.source_digest:
        print(_source_input_evidence(args.repo.resolve())["sha256"])
        return
    required = (args.summary, args.missing, args.manifest)
    if any(path is None for path in required):
        raise SystemExit("--summary, --missing, and --manifest are required")
    if not args.flavor:
        raise SystemExit("at least one --flavor NAME LCOV JSON is required")
    if not args.verified_source_digest:
        raise SystemExit("--verified-source-digest is required")
    analyze(args)


if __name__ == "__main__":
    main()
