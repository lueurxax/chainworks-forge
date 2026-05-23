#!/usr/bin/env python3
"""P076 observe-only auto-retry poll writer.

This script records one validated monitor observation under the P076 contract.
It is deliberately side-effect free: it never calls retry, recovery,
cancellation, approval, archive, continuation, or provider tools.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import auto_retry_rollup


POLICY_VERSION = "p076-observe-only"
OBSERVATION_VERSION = "auto-retry-observation.v1"
BUDGET_VERSION = "auto-retry-budget.v1"


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def utc_basic_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def meta_root() -> Path:
    configured = os.environ.get("CHAINWORKS_META_ROOT")
    if configured:
        return Path(configured)
    return Path.cwd() / ".chainworks"


def paths(root: Path) -> dict[str, Path]:
    automation = root / "automation"
    return {
        "automation": automation,
        "ledger": automation / "auto-retry-observations.jsonl",
        "budget": automation / "auto-retry-budget.json",
        "catalog": automation / "auto-retry-known-issues.json",
        "markdown": automation / "auto-retry-known-issues.md",
        "rollup": automation / "auto-retry-rollup.json",
        "lock": automation / "auto-retry.lock",
    }


def fsync_parent(path: Path) -> None:
    fd = os.open(str(path.parent), os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def durable_write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{uuid.uuid4().hex}.tmp")
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.write(content)
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, path)
    fsync_parent(path)


def append_jsonl(path: Path, record: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n"
    fd = os.open(str(path), os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    try:
        os.write(fd, encoded.encode("utf-8"))
        os.fsync(fd)
    finally:
        os.close(fd)
    fsync_parent(path)


def acquire_lock(path: Path, poll_deadline_seconds: int) -> dict[str, Any]:
    path.parent.mkdir(parents=True, exist_ok=True)
    token = uuid.uuid4().hex
    payload = {
        "hostname": socket.gethostname(),
        "boot_id_or_session_id": os.environ.get("CHAINWORKS_SESSION_ID", "local-session"),
        "pid": os.getpid(),
        "process_start_time": utc_now(),
        "command_identity": "scripts/chainworks/auto_retry_observe.py",
        "lock_token": token,
        "created_at": utc_now(),
        "expires_after_seconds": poll_deadline_seconds * 2,
    }
    try:
        fd = os.open(str(path), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    except FileExistsError:
        return {
            "acquired": False,
            "token": None,
            "payload": None,
            "skipped_reason": "skipped_lock_held",
        }
    with os.fdopen(fd, "w", encoding="utf-8") as fh:
        fh.write(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        fh.flush()
        os.fsync(fh.fileno())
    fsync_parent(path)
    return {"acquired": True, "token": token, "payload": payload, "skipped_reason": None}


def release_lock(path: Path, token: str | None) -> None:
    if not token or not path.exists():
        return
    try:
        current = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return
    if current.get("lock_token") != token:
        return
    path.unlink()
    fsync_parent(path)


def load_blocked_runs(path: Path | None) -> list[dict[str, Any]]:
    if path is None:
        return []
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, dict):
        payload = payload.get("blocked_runs", [])
    if not isinstance(payload, list):
        raise SystemExit("blocked run input must be an array or object with blocked_runs")
    return [normalize_blocked_run(item) for item in payload]


def normalize_blocked_run(item: dict[str, Any]) -> dict[str, Any]:
    run_id = required_string(item, "run_id")
    stage_id = required_string(item, "stage_id")
    blocker_class = item.get("blocker_class") or "unknown_evidence_gap"
    signature = item.get("blocker_signature_id") or stable_signature(item)
    policy_decision = item.get("policy_decision") or default_policy_decision(blocker_class)
    retry_action = item.get("retry_action") or ("none" if blocker_class == "human_gate" else "recommend_retry")
    retry_result = item.get("retry_result") or "not_attempted"
    if retry_result not in {"not_attempted", "not_allowed"}:
        raise SystemExit("P076 observations must not record side-effect retry results")
    if blocker_class == "human_gate" and retry_action != "none":
        raise SystemExit("P076 human_gate observations must not recommend or attempt retry")
    return {
        "run_id": run_id,
        "idea_or_proposal": item.get("idea_or_proposal"),
        "stage_id": stage_id,
        "stage_execution_id": item.get("stage_execution_id"),
        "status_before": item.get("status_before") or "blocked",
        "run_state_projection_status": item.get("run_state_projection_status") or item.get("status_before") or "blocked",
        "drift_details_json": item.get("drift_details_json"),
        "blocker_class": blocker_class,
        "blocker_signature_id": signature,
        "failure_class": item.get("failure_class") or blocker_class,
        "failure_summary": item.get("failure_summary") or "blocked run observed",
        "evidence_report_id": item.get("evidence_report_id"),
        "safe_retry": bool(item.get("safe_retry", False)),
        "retry_budget": item.get("retry_budget") or {
            "run_id": run_id,
            "blocker_signature_id": signature,
            "status": "observe_only",
            "window_hours": 6,
            "max_attempts": 0,
            "attempt_count": 0,
            "remaining_attempts": 0,
            "cooldown_until": None,
            "budget_state_path": str(paths(meta_root())["budget"]),
        },
        "retry_lifecycle": "not_applicable",
        "retry_action": retry_action,
        "retry_result": retry_result,
        "policy_decision": policy_decision,
        "next_systemic_action": item.get("next_systemic_action") or "inspect blocker evidence",
    }


def required_string(item: dict[str, Any], key: str) -> str:
    value = item.get(key)
    if not isinstance(value, str) or not value:
        raise SystemExit(f"blocked run missing required string field {key}")
    return value


def default_policy_decision(blocker_class: str) -> str:
    if blocker_class == "human_gate":
        return "human_gate"
    if blocker_class == "provider_or_session_failure":
        return "collect_evidence"
    return "observe_only"


def stable_signature(item: dict[str, Any]) -> str:
    seed = "|".join(
        str(item.get(key) or "")
        for key in ("run_id", "stage_id", "blocker_class", "failure_class", "failure_summary")
    )
    return "sig-" + hashlib.sha256(seed.encode("utf-8")).hexdigest()[:24]


def build_summary(blocked_runs: list[dict[str, Any]], elapsed_ms: int, poll_deadline_seconds: int) -> dict[str, Any]:
    human_gates = sum(1 for row in blocked_runs if row.get("blocker_class") == "human_gate")
    return {
        "active_total_before": len(blocked_runs),
        "blocked_before": len(blocked_runs) - human_gates,
        "running_before": 0,
        "waiting_approval_before": human_gates,
        "blocked_after": len(blocked_runs) - human_gates,
        "running_after": 0,
        "waiting_approval_after": human_gates,
        "retried_count": 0,
        "observe_only_count": len(blocked_runs),
        "cooldown_exhausted_count": 0,
        "budget_unavailable_count": 0,
        "skipped_backpressure_count": 0,
        "partial": False,
        "poll_deadline_seconds": poll_deadline_seconds,
        "poll_elapsed_ms": elapsed_ms,
    }


def canonical_hash(record: dict[str, Any]) -> str:
    clone = {k: v for k, v in record.items() if k != "canonical_record_hash"}
    encoded = json.dumps(clone, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def ensure_budget_file(path: Path) -> None:
    if path.exists():
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            raise SystemExit(f"budget file is malformed: {exc}") from exc
        if payload.get("schema_version") != BUDGET_VERSION:
            raise SystemExit(f"budget file has unsupported schema_version: {payload.get('schema_version')!r}")
        return
    durable_write_text(
        path,
        json.dumps(
            {
                "schema_version": BUDGET_VERSION,
                "generated_at": utc_now(),
                "p076_side_effect_rule": "observe_only_no_retry_dispatch",
                "attempts": [],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )


def refresh_rollup(path_map: dict[str, Path]) -> None:
    records, diagnostics = auto_retry_rollup.read_jsonl(path_map["ledger"])
    rollup = auto_retry_rollup.build_rollup(records, diagnostics)
    durable_write_text(path_map["rollup"], json.dumps(rollup, indent=2, sort_keys=True) + "\n")
    durable_write_text(
        path_map["catalog"],
        json.dumps(
            {
                "schema_version": "auto-retry-known-issues.v1",
                "generated_at": rollup["generated_at"],
                "path_resolution": {
                    "known_issue_catalog_path": str(path_map["catalog"]),
                    "generated_markdown_catalog_path": str(path_map["markdown"]),
                },
                "issues": rollup["issues"],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )
    lines = [
        "# Auto-Retry Known Issues",
        "",
        f"Generated at: `{rollup['generated_at']}`",
        "",
        "| Signature | Class | Count | Runs | Last decision | Last retry | Owner |",
        "|---|---:|---:|---:|---|---|---|",
    ]
    for issue in rollup["issues"]:
        lines.append(
            "| {sig} | {cls} | {count} | {runs} | {decision} | {retry} | {owner} |".format(
                sig=issue["blocker_signature_id"],
                cls=issue["blocker_class"],
                count=issue["observation_count"],
                runs=", ".join(issue["affected_run_ids"]),
                decision=issue.get("last_policy_decision"),
                retry=issue.get("last_retry_result"),
                owner=issue.get("proposed_owner_lane"),
            )
        )
    durable_write_text(path_map["markdown"], "\n".join(lines) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--blocked-runs-json", type=Path)
    parser.add_argument("--poll-deadline-seconds", type=int, default=300)
    args = parser.parse_args()

    start = time.monotonic()
    root = meta_root()
    path_map = paths(root)
    lock = acquire_lock(path_map["lock"], args.poll_deadline_seconds)
    if not lock["acquired"]:
        print(json.dumps({"status": "skipped_lock_held", "lock_path": str(path_map["lock"])}))
        return 2

    try:
        blocked_runs = load_blocked_runs(args.blocked_runs_json)
        ensure_budget_file(path_map["budget"])
        elapsed_ms = int((time.monotonic() - start) * 1000)
        lock_payload = lock["payload"] or {}
        record = {
            "schema_version": OBSERVATION_VERSION,
            "observation_id": f"ar_obs_{utc_basic_now()}_{uuid.uuid4().hex[:12]}",
            "canonical_record_hash": None,
            "observed_at": utc_now(),
            "source": {
                "tool": "chainworks-orchestrator-ops",
                "version": "p076",
                "workspace_root": str(Path.cwd()),
                "meta_root": str(root),
            },
            "daemon_ready": True,
            "policy_version": POLICY_VERSION,
            "writer_lock": {
                "lock_path": str(path_map["lock"]),
                "acquired": True,
                "token": lock["token"],
                "skipped_reason": None,
                "hostname": lock_payload.get("hostname"),
                "boot_id_or_session_id": lock_payload.get("boot_id_or_session_id"),
                "pid": lock_payload.get("pid"),
                "process_start_time": lock_payload.get("process_start_time"),
                "command_identity": lock_payload.get("command_identity"),
            },
            "summary": build_summary(blocked_runs, elapsed_ms, args.poll_deadline_seconds),
            "blocked_runs": blocked_runs,
            "diagnostics": [],
        }
        record["canonical_record_hash"] = canonical_hash(record)
        append_jsonl(path_map["ledger"], record)
        refresh_rollup(path_map)
        print(
            json.dumps(
                {
                    "status": "recorded",
                    "observation_id": record["observation_id"],
                    "ledger_path": str(path_map["ledger"]),
                    "blocked_run_count": len(blocked_runs),
                },
                sort_keys=True,
            )
        )
        return 0
    finally:
        release_lock(path_map["lock"], lock["token"])


if __name__ == "__main__":
    sys.exit(main())
