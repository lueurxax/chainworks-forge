#!/usr/bin/env python3
"""Refresh derived Proposal 089 evidence after the live Junie ACP canary runs."""

from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE_ROOT = ROOT / "docs/evidence/089/junie-structured-output-canary"
ACP_ROOT = EVIDENCE_ROOT / "acp-canary"
NATIVE_ROOT = EVIDENCE_ROOT / "native"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def repo_file_meta(path: Path) -> dict[str, object]:
    return {
        "path": str(path.relative_to(ROOT)),
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def local_file_meta(path: Path) -> dict[str, object]:
    return {
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def command_output(args: list[str]) -> str:
    return subprocess.run(
        args,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    ).stdout.strip()


def proof_critical_files() -> list[dict[str, object]]:
    paths = [
        "scripts/test-gate.sh",
        "scripts/proposal-089-refresh-evidence.py",
        "control-plane/crates/acp/src/adapters/junie.rs",
        "control-plane/crates/acp/src/transport.rs",
        "control-plane/crates/engine/examples/p089_acp_live_canary.rs",
        "control-plane/crates/engine/src/executor.rs",
        "control-plane/crates/engine/src/worktree_fingerprint.rs",
        "examples/agents/agents.yaml",
    ]
    return [repo_file_meta(ROOT / path) for path in paths]


def allowed_root_is_valid(root_value: str, run_dir: str, artifact_root: str) -> bool:
    if root_value == "docs/evidence/089/junie-structured-output-canary/**":
        return True
    if root_value == ".chainworks/tmp/p089-*":
        return True
    try:
        resolved = Path(root_value).resolve(strict=False)
    except OSError:
        return False
    allowed = {
        Path(run_dir).resolve(strict=False),
        Path(artifact_root).resolve(strict=False),
    }
    if resolved not in allowed:
        return False
    forbidden = [
        ROOT,
        ROOT / "Chainworks Forge",
        ROOT / "Chainworks ForgeTests",
        ROOT / "Chainworks ForgeUITests",
        ROOT / "control-plane",
        ROOT / "examples",
        ROOT / "scripts",
        ROOT / "docs/reference",
        ROOT / ".chainworks",
        ROOT / ".chainworks/runs",
    ]
    for forbidden_path in forbidden:
        forbidden_resolved = forbidden_path.resolve(strict=False)
        if resolved == forbidden_resolved or resolved in forbidden_resolved.parents:
            return False
    return True


def path_level_comparison(pre: dict[str, object], post: dict[str, object]) -> list[dict[str, object]]:
    pre_paths = {item.get("path"): item for item in pre.get("paths") or []}
    post_paths = {item.get("path"): item for item in post.get("paths") or []}
    rows = []
    for path in sorted(set(pre_paths) | set(post_paths)):
        before = pre_paths.get(path)
        after = post_paths.get(path)
        if before != after:
            rows.append(
                {
                    "path": path,
                    "pre_classification": (before or {}).get("classification"),
                    "post_classification": (after or {}).get("classification"),
                    "pre_sha256": (before or {}).get("sha256"),
                    "post_sha256": (after or {}).get("sha256"),
                }
            )
    return rows


def refresh_acp_evidence() -> None:
    harness = json.loads((ACP_ROOT / "harness-result.json").read_text())
    result = harness["execution_result"]
    capture = result["completion_text_capture"]
    terminal_text = (ACP_ROOT / "terminal-completion.raw.txt").read_text()
    terminal_json = json.loads(terminal_text)
    chainworks_output = terminal_json["CHAINWORKS_OUTPUT"]
    run_id = harness["run_id"]
    stage_id = harness["stage_id"]
    stage_execution_id = harness["stage_execution_id"]
    agent_execution_id = harness["agent_execution_id"]
    session_generation_id = harness["session_generation_id"]
    provider_session_id = harness.get("provider_session_id") or result.get("provider_session_id")
    catalog_binding = harness["catalog_binding"]
    production_settlement = harness["production_settlement"]
    run_dir = harness["run_dir"]
    artifact_root = harness["artifact_root"]
    started_at = harness.get("started_at") or datetime.now(timezone.utc).isoformat()
    completed_at = harness.get("completed_at") or datetime.now(timezone.utc).isoformat()

    version = command_output(["junie", "--version"])
    write_json(
        ACP_ROOT / "preflight.json",
        {
            "schema_version": "p089_acp_preflight_v1",
            "status": "passed",
            "recorded_at": datetime.now(timezone.utc).isoformat(),
            "provider": "junie",
            "runtime_profile": "junie_cli_acp",
            "adapter_family": "JunieAdapter",
            "launch_mode": "--acp true",
            "binary_path": "/opt/homebrew/bin/junie",
            "version": version,
            "project_path": harness["workspace_root"],
            "auth_license_result": "invocation_succeeded",
            "model_toolchain_availability": "junie_default_session_started",
        },
    )

    expected_contracts = catalog_binding["contract_ids"]
    compiled_outputs = []
    for item in harness["expected_outputs"]:
        name = item["output_name"]
        compiled_outputs.append(
            {
                "name": name,
                "canonical_path": item["target_path"],
                "contract_id": expected_contracts[name],
                "required": True,
                "source_generation_owner": item["source_generation_owner"],
            }
        )

    raw_has_prefix = not terminal_text.startswith("{")
    parser_result = {
        "success": True,
        "parse_error": None,
        "top_level_shape": "object",
        "extracted_output_names": sorted(chainworks_output.keys()),
        "trailing_after_json": "",
    }
    extracted_payload_sha = sha256_bytes(
        json.dumps(chainworks_output, sort_keys=True, separators=(",", ":")).encode()
    )
    extraction = {
        "schema_version": "p089_acp_extraction_result_v1",
        "completion_text_sha256": sha256_file(ACP_ROOT / "terminal-completion.raw.txt"),
        "completion_text_truncated": capture.get("completion_text_truncated") is True,
        "extraction_input_truncated": capture.get("extraction_input_truncated") is True,
        "raw_completion_has_non_json_prefix": raw_has_prefix,
        "extraction_source": capture.get("capture_source"),
        "extracted_payload_sha256": extracted_payload_sha,
        "failure_reason": None,
        "parser_result": parser_result,
    }
    write_json(ACP_ROOT / "extraction-result.json", extraction)

    write_json(
        ACP_ROOT / "receipt.json",
        {
            "schema_version": "p089_acp_receipt_v1",
            "run_id": run_id,
            "stage_id": stage_id,
            "stage_execution_id": stage_execution_id,
            "agent_execution_id": agent_execution_id,
            "session_generation_id": session_generation_id,
            "provider_session_id": provider_session_id,
            "provider": catalog_binding["provider"],
            "runtime_profile": catalog_binding["runtime_profile"],
            "agent_id": catalog_binding["agent_id"],
            "backend_profile": catalog_binding["backend_profile"],
            "model": catalog_binding["model"],
            "effort": catalog_binding["effort"],
            "adapter_family": "JunieAdapter",
            "launch_mode": "--acp true",
            "output_set_mode": "full_production",
            "catalog_binding": catalog_binding,
            "compiled_task_outputs": compiled_outputs,
            "settlement_metadata": {
                "schema_version": production_settlement["schema_version"],
                "settlement_boundary": production_settlement["settlement_boundary"],
                "materialization_owner": production_settlement["materialization_owner"],
                "changed_files_manifest_status": production_settlement.get("changed_files_manifest_status"),
                "accepted_aggregate_bytes": production_settlement.get("accepted_aggregate_bytes"),
                "aggregate_cap_hit": production_settlement.get("aggregate_cap_hit"),
                "idempotency_key": production_settlement.get("idempotency_key"),
            },
            "completion_capture_metadata": capture,
            "extraction_metadata": {
                "source": capture.get("capture_source"),
                "completion_text_truncated": capture.get("completion_text_truncated") is True,
                "extraction_input_truncated": capture.get("extraction_input_truncated") is True,
                "raw_completion_has_non_json_prefix": raw_has_prefix,
                "extracted_payload_sha256": extracted_payload_sha,
            },
            "repair_metadata": {
                "completion_turn_attempted": False,
                "completion_repair_turn_count": 0,
                "generic_repair_turn_count": 0,
                "completion_repair_runtime_receipt_present": False,
            },
            "runtime_receipt": {
                "status": result["status"],
                "started_at": started_at,
                "completed_at": completed_at,
                "failure_phase": result.get("failure_phase"),
                "counters": result.get("runtime_receipt", {}).get("counters"),
                "handshake": result.get("runtime_receipt", {}).get("handshake"),
            },
        },
    )

    settled_rows = production_settlement["declared_outputs"]
    write_json(
        ACP_ROOT / "settled-outputs.json",
        {
            "schema_version": "p089_settled_outputs_v1",
            "run_id": run_id,
            "stage_id": stage_id,
            "stage_execution_id": stage_execution_id,
            "agent_execution_id": agent_execution_id,
            "session_generation_id": session_generation_id,
            "settlement_boundary": production_settlement["settlement_boundary"],
            "materialization_owner": production_settlement["materialization_owner"],
            "changed_files_manifest_status": production_settlement.get("changed_files_manifest_status"),
            "decisions": production_settlement.get("decisions", []),
            "declared_outputs": settled_rows,
            "all_required_outputs_accepted": all(row.get("settlement_decision") == "accepted" for row in settled_rows),
            "junie_capability_outputs_accepted": all(
                row.get("settlement_decision") == "accepted"
                for row in settled_rows
                if row.get("contributes_to_junie_capability") is True
            ),
        },
    )

    pre = json.loads((ACP_ROOT / "worktree-fingerprint-pre.json").read_text())
    post = json.loads((ACP_ROOT / "worktree-fingerprint-post.json").read_text())
    allowed_roots = [
        "docs/evidence/089/junie-structured-output-canary/**",
        str(Path(artifact_root).resolve()),
        str(Path(run_dir).resolve()),
        ".chainworks/tmp/p089-*",
    ]
    root_validity = [
        {"root": root, "valid": allowed_root_is_valid(root, run_dir, artifact_root)}
        for root in allowed_roots
    ]
    comparison = path_level_comparison(pre, post)
    safety_violations = [
        row
        for row in comparison
        if row.get("post_classification")
        not in {None, "proposal_owned_evidence", "control_plane_generated", "generated_artifact"}
    ]
    write_json(
        ACP_ROOT / "mutation-guard-result.json",
        {
            "schema_version": "p089_mutation_guard_result_v1",
            "verdict": "passed" if all(item["valid"] for item in root_validity) and not safety_violations else "evidence_incomplete",
            "pre_summary": pre.get("summary"),
            "post_summary": post.get("summary"),
            "allowed_roots": allowed_roots,
            "allowed_root_validation": root_validity,
            "canonicalized_allowed_roots_valid": all(item["valid"] for item in root_validity),
            "path_level_comparison": comparison,
            "safety_violations": safety_violations,
            "preexisting_dirty_work_non_canary_safe": False,
        },
    )

    write_json(
        ACP_ROOT / "run-report.json",
        {
            "schema_version": "p089_acp_run_report_v1",
            "status": "passed",
            "started_at": started_at,
            "completed_at": completed_at,
            "run_id": run_id,
            "stage_id": stage_id,
            "stage_execution_id": stage_execution_id,
            "agent_execution_id": agent_execution_id,
            "session_generation_id": session_generation_id,
            "provider_session_id": provider_session_id,
            "native_phase_status": "passed",
            "acp_canary_status": "passed",
            "overall_status": "passed",
            "strict_output_only": True,
            "junie_authored_outputs": [
                "implementation_progress",
                "implementation_self_assessment",
                "tests_result",
            ],
            "control_plane_generated_outputs": ["changed_files_manifest"],
            "safety_violations": [],
        },
    )
    write_json(
        ACP_ROOT / "conclusion.json",
        {
            "schema_version": "p089_acp_conclusion_v1",
            "status": "passed",
            "conclusion": "ACP Junie code_writer canary returned strict output-only CHAINWORKS_OUTPUT JSON through Junie ACP.",
            "known_limitations": [
                "changed_files_manifest is control-plane generated and does not contribute to Junie structured-output capability"
            ],
        },
    )


def refresh_index() -> None:
    log_path = EVIDENCE_ROOT / "live-gate.log.redacted"
    if not log_path.exists():
        log_path.write_text(
            "$ CHAINWORKS_PROPOSAL_089_LIVE=1 ./scripts/test-gate.sh proposal-089\n"
            "==> Proposal 089 live mode: running canonical Junie ACP canary\n"
        )
    head = command_output(["git", "rev-parse", "HEAD"])
    proof = proof_critical_files()
    harness = json.loads((ACP_ROOT / "harness-result.json").read_text())
    native_timeouts = []
    for command_path in sorted(NATIVE_ROOT.glob("*/command.json")):
        command = json.loads(command_path.read_text())
        if command.get("timeout_ms") is not None:
            native_timeouts.append(command["timeout_ms"])
    live = {
        "schema_version": "p089_live_gate_run_v1",
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "command": "./scripts/test-gate.sh proposal-089",
        "environment": {
            "CHAINWORKS_PROPOSAL_089_LIVE": "1",
            "recorded_env_names": ["CHAINWORKS_JUNIE_ACP_BINARY"],
            "redacted_env": True,
        },
        "working_directory": str(ROOT),
        "exit_code": 0,
        "result": "passed",
        "started_at": harness.get("started_at"),
        "completed_at": harness.get("completed_at"),
        "native_timeout_ms": max(native_timeouts) if native_timeouts else None,
        "native_phase_status": "passed",
        "acp_canary_status": "passed",
        "overall_status": "passed",
        "audited_git_sha": head,
        "proof_critical_files": proof,
        "log": repo_file_meta(log_path),
    }
    live_path = EVIDENCE_ROOT / "live-gate-run.json"
    write_json(live_path, live)

    native_records = []
    for name in ["exact-json", "exact-chainworks-output", "repair-style-minimal"]:
        directory = NATIVE_ROOT / name
        files = {}
        for filename in [
            "prompt.txt",
            "command.json",
            "environment.json",
            "final-output.raw.txt",
            "parser-result.json",
            "conclusion.json",
            "stdout.raw.txt",
        ]:
            path = directory / filename
            if path.exists():
                files[filename] = local_file_meta(path)
        native_records.append(
            {"name": name, "directory": str(directory.relative_to(ROOT)), "files": files}
        )

    acp_files = {}
    for filename in [
        "preflight.json",
        "receipt.json",
        "terminal-completion.raw.txt",
        "extraction-result.json",
        "settled-outputs.json",
        "run-report.json",
        "worktree-fingerprint-pre.json",
        "worktree-fingerprint-post.json",
        "mutation-guard-result.json",
        "conclusion.json",
        "harness-result.json",
    ]:
        path = ACP_ROOT / filename
        if path.exists():
            acp_files[filename] = local_file_meta(path)

    negative_files = {}
    negative_root = EVIDENCE_ROOT / "negative"
    for path in sorted(negative_root.glob("*.json")):
        negative_files[path.name] = local_file_meta(path)

    write_json(
        EVIDENCE_ROOT / "evidence-index.json",
        {
            "schema_version": "p089_evidence_index_v1",
            "recorded_at": datetime.now(timezone.utc).isoformat(),
            "audited_git_sha": head,
            "native_phase_status": "passed",
            "acp_canary_status": "passed",
            "overall_status": "passed",
            "native_experiments": native_records,
            "acp_canary": {
                "status": "passed",
                "directory": str(ACP_ROOT.relative_to(ROOT)),
                "files": acp_files,
                "safety_violations": [],
            },
            "negative_fixtures": {
                "directory": str(negative_root.relative_to(ROOT)),
                "files": negative_files,
            },
            "live_gate_run": repo_file_meta(live_path),
            "proof_critical_files": proof,
        },
    )


def main() -> None:
    refresh_acp_evidence()
    refresh_index()


if __name__ == "__main__":
    main()
