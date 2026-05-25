#!/usr/bin/env python3
"""Strict P081 boundary-policy canary validator.

The repository intentionally avoids adding a YAML runtime dependency for this
small, fixed-shape evidence file. This parser accepts only the documented
`boundary_policy_canaries_v1` subset and fails closed on duplicate keys,
unknown fields, malformed list items, missing matrix rows, and report drift.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import tempfile
from dataclasses import dataclass


TOP_LEVEL_KEYS = {"schema_version", "proposal_id", "matrix_id", "generated_from", "canaries"}
CANARY_KEYS = {"row_id", "test_id", "expected_decision"}
EXPECTED_DECISIONS = {"allow", "allow_redacted", "deny"}


@dataclass(frozen=True)
class Canary:
    row_id: str
    test_id: str
    expected_decision: str


def parse_scalar(line: str) -> tuple[str, str]:
    if ":" not in line:
        raise ValueError(f"expected key: value line, got {line!r}")
    key, value = line.split(":", 1)
    key = key.strip()
    value = value.strip()
    if not key:
        raise ValueError(f"expected non-empty key and value, got {line!r}")
    return key, value.strip('"').strip("'")


def parse_canaries(path: pathlib.Path) -> tuple[dict[str, str], list[Canary]]:
    top: dict[str, str] = {}
    canaries: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    seen_top: set[str] = set()

    for line_no, raw in enumerate(path.read_text().splitlines(), start=1):
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if raw.startswith("  - "):
            if current is not None:
                canaries.append(current)
            current = {}
            key, value = parse_scalar(raw[4:])
            if key not in CANARY_KEYS:
                raise ValueError(f"{path}:{line_no}: unknown canary field {key!r}")
            current[key] = value
            continue
        if raw.startswith("    "):
            if current is None:
                raise ValueError(f"{path}:{line_no}: canary field before list item")
            key, value = parse_scalar(raw[4:])
            if key not in CANARY_KEYS:
                raise ValueError(f"{path}:{line_no}: unknown canary field {key!r}")
            if key in current:
                raise ValueError(f"{path}:{line_no}: duplicate canary field {key!r}")
            current[key] = value
            continue
        if raw.startswith(" "):
            raise ValueError(f"{path}:{line_no}: unsupported indentation")

        if current is not None:
            canaries.append(current)
            current = None
        key, value = parse_scalar(raw)
        if key not in TOP_LEVEL_KEYS:
            raise ValueError(f"{path}:{line_no}: unknown top-level field {key!r}")
        if key in seen_top:
            raise ValueError(f"{path}:{line_no}: duplicate top-level field {key!r}")
        seen_top.add(key)
        if key == "canaries":
            if value not in ("", "[]"):
                raise ValueError(f"{path}:{line_no}: canaries must be a list, not scalar {value!r}")
        else:
            top[key] = value

    if current is not None:
        canaries.append(current)

    parsed: list[Canary] = []
    for index, item in enumerate(canaries, start=1):
        missing = CANARY_KEYS - item.keys()
        extra = item.keys() - CANARY_KEYS
        if missing:
            raise ValueError(f"{path}: canary #{index} missing fields {sorted(missing)}")
        if extra:
            raise ValueError(f"{path}: canary #{index} has unknown fields {sorted(extra)}")
        if item["expected_decision"] not in EXPECTED_DECISIONS:
            raise ValueError(
                f"{path}: canary #{index} has invalid expected_decision {item['expected_decision']!r}"
            )
        parsed.append(
            Canary(
                row_id=item["row_id"],
                test_id=item["test_id"],
                expected_decision=item["expected_decision"],
            )
        )

    return top, parsed


def validate(root: pathlib.Path, canary_path: pathlib.Path, matrix_path: pathlib.Path, report_path: pathlib.Path) -> None:
    top, canaries = parse_canaries(canary_path)
    if top.get("schema_version") != "boundary_policy_canaries_v1":
        raise ValueError("P081: canary schema_version mismatch")
    if top.get("proposal_id") != "proposal-081":
        raise ValueError("P081: canary proposal_id mismatch")

    matrix = json.loads(matrix_path.read_text())
    report = json.loads(report_path.read_text())
    if top.get("matrix_id") != matrix.get("matrix_id"):
        raise ValueError("P081: canary matrix_id does not match boundary matrix")
    if report.get("schema_version") != "boundary_policy_shadow_coverage_report_v1":
        raise ValueError("P081: shadow coverage report schema_version mismatch")
    if report.get("matrix_id") != matrix.get("matrix_id"):
        raise ValueError("P081: shadow coverage matrix_id mismatch")

    matrix_ids = {row["row_id"] for row in matrix.get("rows") or []}
    canary_ids = [canary.row_id for canary in canaries]
    duplicate_ids = sorted({row_id for row_id in canary_ids if canary_ids.count(row_id) > 1})
    if duplicate_ids:
        raise ValueError(f"P081: duplicate canary row ids: {duplicate_ids}")
    missing = sorted(matrix_ids - set(canary_ids))
    extra = sorted(set(canary_ids) - matrix_ids)
    if missing:
        raise ValueError(f"P081: canaries missing matrix rows: {missing}")
    if extra:
        raise ValueError(f"P081: canaries reference unknown matrix rows: {extra}")

    by_row = {canary.row_id: canary for canary in canaries}
    report_rows = report.get("rows") or []
    report_ids = {row.get("row_id") for row in report_rows}
    if matrix_ids - report_ids:
        raise ValueError(f"P081: shadow report missing matrix rows: {sorted(matrix_ids - report_ids)}")
    for row in report_rows:
        row_id = row.get("row_id")
        if row_id not in by_row:
            raise ValueError(f"P081: shadow report references non-canary row {row_id!r}")
        canary = by_row[row_id]
        if row.get("canary_covered") is not True:
            raise ValueError(f"P081: shadow row {row_id} must be canary_covered=true")
        if row.get("required_test_id") != canary.test_id:
            raise ValueError(
                f"P081: shadow row {row_id} required_test_id {row.get('required_test_id')!r} "
                f"does not match canary {canary.test_id!r}"
            )
        if int(row.get("shadow_disagreement_count") or 0) != 0:
            raise ValueError(f"P081: shadow row {row_id} has disagreement")

    if not any(canary.expected_decision == "allow_redacted" for canary in canaries):
        raise ValueError("P081: canaries must include allow_redacted observer proof")

    print(
        f"P081: structured canary validation passed for {len(canaries)} rows "
        f"against {canary_path.relative_to(root)}"
    )


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        matrix = root / "matrix.json"
        report = root / "report.json"
        canary = root / "canaries.yaml"
        matrix.write_text(json.dumps({"matrix_id": "m", "rows": [{"row_id": "a"}, {"row_id": "b"}]}))
        report.write_text(
            json.dumps(
                {
                    "schema_version": "boundary_policy_shadow_coverage_report_v1",
                    "matrix_id": "m",
                    "rows": [
                        {
                            "row_id": "a",
                            "canary_covered": True,
                            "shadow_disagreement_count": 0,
                            "required_test_id": "ta",
                        },
                        {
                            "row_id": "b",
                            "canary_covered": True,
                            "shadow_disagreement_count": 0,
                            "required_test_id": "tb",
                        },
                    ],
                }
            )
        )
        canary.write_text(
            "\n".join(
                [
                    "schema_version: boundary_policy_canaries_v1",
                    "proposal_id: proposal-081",
                    "matrix_id: m",
                    "generated_from: matrix.json",
                    "canaries:",
                    "  - row_id: a",
                    "    test_id: ta",
                    "    expected_decision: allow",
                    "  - row_id: b",
                    "    test_id: tb",
                    "    expected_decision: allow_redacted",
                ]
            )
        )
        validate(root, canary, matrix, report)
        bad = canary.read_text().replace("    test_id: tb", "    unknown: tb")
        canary.write_text(bad)
        try:
            validate(root, canary, matrix, report)
        except ValueError as exc:
            if "unknown canary field" not in str(exc):
                raise
        else:
            raise AssertionError("self-test expected unknown field failure")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    root = args.root
    validate(
        root,
        root / "docs/evidence/boundary-policy-shadow-coverage/boundary-policy-canaries.yaml",
        root / "docs/reference/boundary-first-api-auth-contract.json",
        root / "docs/evidence/boundary-policy-shadow-coverage/report.json",
    )


if __name__ == "__main__":
    main()
