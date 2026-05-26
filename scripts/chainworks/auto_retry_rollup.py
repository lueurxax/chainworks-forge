#!/usr/bin/env python3
"""P076 observe-only auto-retry ledger rollup.

Reads auto-retry-observation.v1 JSONL records and writes a deterministic
grouped issue table keyed by blocker_signature_id. The script never dispatches
retry/recovery side effects.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def meta_root() -> Path:
    configured = os.environ.get("CHAINWORKS_META_ROOT")
    if configured:
        return Path(configured)
    return Path.cwd() / ".chainworks"


def default_paths() -> dict[str, Path]:
    root = meta_root()
    automation = root / "automation"
    return {
        "ledger": automation / "auto-retry-observations.jsonl",
        "catalog": automation / "auto-retry-known-issues.json",
        "markdown": automation / "auto-retry-known-issues.md",
        "rollup": automation / "auto-retry-rollup.json",
    }


def read_jsonl(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    if not path.exists():
        diagnostics.append(
            diagnostic("no_observation_history", "info", f"ledger not found: {path}")
        )
        return records, diagnostics

    lines = path.read_text(encoding="utf-8").splitlines()
    raw = path.read_bytes()
    if raw and not raw.endswith(b"\n"):
        diagnostics.append(
            diagnostic(
                "partial_trailing_record",
                "warning",
                "ledger does not end with newline; last line is ignored",
                str(path),
            )
        )
        lines = lines[:-1]

    for idx, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            diagnostics.append(
                diagnostic(
                    "ledger_parse_failed",
                    "error",
                    f"line {idx}: {exc}",
                    str(path),
                )
            )
            continue
        if value.get("schema_version") != "auto-retry-observation.v1":
            diagnostics.append(
                diagnostic(
                    "unsupported_observation_version",
                    "warning",
                    f"line {idx}: {value.get('schema_version')!r}",
                    str(path),
                )
            )
            continue
        records.append(value)
    return records, diagnostics


def build_rollup(records: list[dict[str, Any]], diagnostics: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[str, dict[str, Any]] = {}
    for record in records:
        observation_id = record.get("observation_id")
        observed_at = record.get("observed_at")
        for blocked in record.get("blocked_runs") or []:
            signature = blocked.get("blocker_signature_id")
            if not signature:
                diagnostics.append(
                    diagnostic(
                        "missing_blocker_signature_id",
                        "warning",
                        f"observation {observation_id} has blocked run without signature",
                    )
                )
                continue
            row = grouped.setdefault(
                signature,
                {
                    "blocker_signature_id": signature,
                    "blocker_class": blocked.get("blocker_class", "unknown"),
                    "first_seen_at": observed_at,
                    "last_seen_at": observed_at,
                    "observation_count": 0,
                    "affected_run_ids": set(),
                    "last_stage_id": blocked.get("stage_id"),
                    "last_observation_id": observation_id,
                    "last_policy_decision": blocked.get("policy_decision"),
                    "last_retry_result": blocked.get("retry_result"),
                    "last_evidence_report_id": blocked.get("evidence_report_id"),
                    "proposed_owner_lane": owner_lane(blocked),
                    "current_status": "observed",
                },
            )
            row["observation_count"] += 1
            row["last_seen_at"] = observed_at
            row["last_stage_id"] = blocked.get("stage_id")
            row["last_observation_id"] = observation_id
            row["last_policy_decision"] = blocked.get("policy_decision")
            row["last_retry_result"] = blocked.get("retry_result")
            row["last_evidence_report_id"] = blocked.get("evidence_report_id")
            if blocked.get("run_id"):
                row["affected_run_ids"].add(blocked["run_id"])

    issues = []
    for row in grouped.values():
        row = dict(row)
        row["affected_run_ids"] = sorted(row["affected_run_ids"])
        issues.append(row)
    issues.sort(key=lambda item: (-item["observation_count"], item["blocker_signature_id"]))

    return {
        "schema_version": "auto-retry-rollup.v1",
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "issue_count": len(issues),
        "issues": issues,
        "diagnostics": diagnostics,
    }


def owner_lane(blocked: dict[str, Any]) -> str:
    blocker_class = blocked.get("blocker_class")
    if blocker_class == "human_gate":
        return "human_operator"
    if blocker_class == "substantive_output_contract":
        return "output_contract"
    if blocker_class in {"stale_execution_truth", "projection_divergence"}:
        return "control_plane_recovery"
    if blocker_class == "provider_or_session_failure":
        return "provider_runtime"
    return "unknown"


def diagnostic(code: str, severity: str, message: str, path: str | None = None) -> dict[str, Any]:
    return {
        "code": code,
        "severity": severity,
        "message": message,
        "path": path,
        "run_id": None,
        "blocker_signature_id": None,
        "observation_id": None,
    }


def write_markdown(path: Path, rollup: dict[str, Any]) -> None:
    lines = [
        "# Auto-Retry Known Issues",
        "",
        f"Generated at: `{rollup['generated_at']}`",
        "",
        "| Signature | Class | Count | Runs | Last decision | Last retry | Owner |",
        "|---|---:|---:|---:|---|---|---|",
    ]
    for issue in rollup["issues"]:
        runs = ", ".join(issue["affected_run_ids"])
        lines.append(
            "| {sig} | {cls} | {count} | {runs} | {decision} | {retry} | {owner} |".format(
                sig=issue["blocker_signature_id"],
                cls=issue["blocker_class"],
                count=issue["observation_count"],
                runs=runs,
                decision=issue.get("last_policy_decision"),
                retry=issue.get("last_retry_result"),
                owner=issue.get("proposed_owner_lane"),
            )
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--catalog", type=Path)
    parser.add_argument("--markdown", type=Path)
    parser.add_argument("--write-markdown", action="store_true")
    args = parser.parse_args()

    paths = default_paths()
    ledger = args.ledger or paths["ledger"]
    output = args.output or paths["rollup"]
    catalog = args.catalog or paths["catalog"]
    markdown = args.markdown or paths["markdown"]

    records, diagnostics = read_jsonl(ledger)
    rollup = build_rollup(records, diagnostics)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(rollup, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    catalog.write_text(
        json.dumps(
            {
                "schema_version": "auto-retry-known-issues.v1",
                "generated_at": rollup["generated_at"],
                "path_resolution": {
                    "known_issue_catalog_path": str(catalog),
                    "generated_markdown_catalog_path": str(markdown),
                },
                "issues": rollup["issues"],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    if args.write_markdown:
        write_markdown(markdown, rollup)
    print(json.dumps({"rollup_path": str(output), "issue_count": rollup["issue_count"]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
