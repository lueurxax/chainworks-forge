#!/usr/bin/env python3
"""Fail-closed static gate for P031 thin GraphQL-only UI coverage."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


INVENTORY_PATH = Path("docs/reference/p031-thin-ui-inventory.json")
WRITE_PATH_GUIDE_PATH = Path("docs/reference/p031-operator-write-path-guide.json")
PHASE0_MANIFEST_PATH = Path("docs/reference/p031-phase-0-artifact-manifest.json")
REQUIRED_SCHEMA_VERSION = "p031-thin-ui-inventory-v1"
REQUIRED_WRITE_PATH_GUIDE_SCHEMA_VERSION = "p031-operator-write-path-guide-v1"
REQUIRED_PHASE0_MANIFEST_SCHEMA_VERSION = "p031-phase-0-artifact-manifest-v1"
GRAPHQL_DOCUMENT_GLOBS = ("**/*.graphql", "**/*.gql")
EXCLUDED_REPO_FILE_PREFIXES = (
    ".build/",
    ".chainworks/",
    ".git/",
    ".swiftpm/",
    "chainworks/",
    "control-plane/target/",
    "reports/",
    "tmp/",
)

REQUIRED_TOP_LEVEL_KEYS = {
    "schema_version",
    "governed_swift_files",
    "governed_graphql_documents",
    "embedded_graphql_documents",
    "generated_graphql_outputs",
    "degraded_fail_closed_files",
    "explicit_exclusions",
    "forbidden_pattern_groups",
}

REQUIRED_MANIFEST_ENTRY_IDS = {
    "governing_contract",
    "p043_reconciliation_evidence",
    "p031_gate_evidence",
    "ui_inventory",
    "schema_decision_record",
    "operator_write_path_guide",
    "degraded_state_evidence",
    "freshness_baseline",
    "report_payload_priority_decision",
    "ux_accessibility_signoff",
    "dogfood_signoff_template",
    "p041_parity_evidence",
}

P041_RUNTIME_ROW_PATH = Path(
    "control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json"
)
P041_RUNTIME_DETAIL_PATH = Path(
    "control-plane/target/parity/publication/current/p031-p041-parity-evidence.json"
)
P041_ROW_SCHEMA_VERSION = "p031-phase-0-runtime-manifest-row.v1"
P041_DETAIL_SCHEMA_VERSION = "p031-p041-parity-evidence.v1"
P041_READY_STATUS = "ready_same_tree_verified"

EVIDENCE_BLOCKING_STATUSES = {
    "blocked",
    "deferred",
    "failed",
    "missing",
    "not_ready",
    "pending",
    "waiver_pending",
}
PHASE0_MANIFEST_READY_STATUS = "ready"

REQUIRED_FORBIDDEN_GROUPS = {
    "mcp",
    "graphql_mutations",
    "local_write_fallback",
    "command_plumbing",
    "raw_truth_probing",
    "removed_write_controls",
}

REQUIRED_WRITE_PATH_ROW_KEYS = {
    "removed_control_id",
    "removed_control_label",
    "external_workflow_kind",
    "external_workflow_name_or_tool",
    "required_identifiers",
    "minimum_parameter_shape",
    "unavailable_reason",
    "expected_success_output",
    "follow_up_id",
    "operator_notes",
    "validation_status",
}

ALLOWED_EXTERNAL_WORKFLOW_KINDS = {
    "mcp_terminal",
    "cli",
    "automation",
    "non_p031_ui",
    "temporarily_unavailable",
}

ALLOWED_VALIDATION_STATUSES = {
    "validated",
    "pending",
    "not_validated",
    "unvalidated",
    "failed",
    "invalid",
}

REMOVED_WRITE_CONTROL_IDS = [
    "ideas.create",
    "runs.start",
    "runs.cancel",
    "stages.retry",
    "approvals.resolve",
    "steward.run_analysis",
    "session.reset",
    "session.resume",
    "runs.clone",
    "runs.compare",
    "experiments.launch",
    "runtime.health",
    "agents.reset",
]

REQUIRED_IDENTIFIERS_BY_CONTROL = {
    "runs.start": {"idea_id"},
    "runs.cancel": {"run_id"},
    "stages.retry": {"run_id", "stage_id"},
    "approvals.resolve": {"approval_id", "run_id", "stage_id"},
    "steward.run_analysis": {"run_id"},
    "session.reset": {"run_id"},
    "session.resume": {"run_id"},
    "runs.clone": {"run_id"},
    "runs.compare": {"run_id"},
    "experiments.launch": {"run_id"},
    "runtime.health": {"run_id"},
    "agents.reset": {"run_id"},
}

INITIAL_GOVERNED_SURFACES = [
    "Chainworks Forge/Views/RunsHomeView.swift",
    "Chainworks Forge/Views/RunDetailPanel.swift",
    "Chainworks Forge/Views/StageDetailView.swift",
    "Chainworks Forge/Views/ApprovalGateView.swift",
    "Chainworks Forge/Views/ArtifactInspectorView.swift",
    "Chainworks Forge/Views/RunArtifactHierarchyView.swift",
    "Chainworks Forge/Views/RunReportView.swift",
    "Chainworks Forge/Views/BlockedRunRecoveryView.swift",
    "Chainworks Forge/Views/RecoverySheet.swift",
    "Chainworks Forge/Views/RunComparisonView.swift",
    "Chainworks Forge/Views/WorkflowInspectorView.swift",
    "Chainworks Forge/Views/WorkflowMapView.swift",
    "Chainworks Forge/Views/DaemonLifecycleSurface.swift",
]

DEFAULT_FORBIDDEN_PATTERNS = {
    "mcp": [
        r"\bMCPCommandClient\b",
        r"\bMCPPolicyRuntime\b",
        r"\bMCPTransport\b",
        r"\bMCPTool\b",
        r"\bimport\s+\w*MCP\w*\b",
    ],
    "graphql_mutations": [
        r"(?m)^\s*mutation\b",
        r"\bmutation\s+[A-Za-z_][A-Za-z0-9_]*",
        r"\bGraphQLMutation\b",
        r"\.mutate\s*\(",
        r"\.perform\s*\(\s*mutation\s*:",
        r"\bMutation[A-Za-z0-9_]*SelectionSet\b",
    ],
    "local_write_fallback": [
        r"\bExecutionService\b",
        r"\bRecoveryCoordinator\b",
        r"\bRunPlanCompiler\b",
        r"\bSwiftData\b",
        r"\b@Query\b",
        r"\bModelContext\b",
        r"\bresolveApproval\b",
        r"\bstartRun\b",
        r"\bcancelRun\b",
        r"\bretryStage\b",
        r"\bresetSession\b",
        r"\bresumeRun\b",
        r"\bcloneRun\b",
        r"\bcompareRuns\b",
        r"\blaunchExperiment\b",
        r"\bruntimeHealth\b",
        r"\bagentReset\b",
        r"\bresetAgent\b",
    ],
    "command_plumbing": [
        r"\bActionInvocationIdentity\b",
        r"\bclient_command_id\b",
        r"\bCommandReceipt\b",
        r"\bCommandHandler\b",
        r"\bCommandLegality\b",
    ],
    "raw_truth_probing": [
        r"\bcontentsOfDirectory\b",
        r"\bString\s*\(\s*contentsOf\s*:",
        r"\bRunPlanFile\b",
        r"\brawArtifact",
    ],
    "removed_write_controls": [
        r"\bideas\.create\b",
        r"\bruns\.start\b",
        r"\bruns\.cancel\b",
        r"\bstages\.retry\b",
        r"\bapprovals\.resolve\b",
        r"\bsteward\.run_analysis\b",
        r"\bsession\.reset\b",
        r"\bsession\.resume\b",
        r"\bruns\.clone\b",
        r"\bruns\.compare\b",
        r"\bexperiments\.launch\b",
        r"\bruntime\.health\b",
        r"\bagents\.reset\b",
        r"\bagent\.reset\b",
        r"\bruntime-health\b",
        r"\bagent-reset\b",
        r"\breset-session\b",
        r"\bresume-session\b",
        r"(?mi)^\s*(query|subscription)\s+[A-Za-z0-9_]*(CreateIdea|StartRun|CancelRun|RetryStage|ResolveApproval|RunSteward|ResetSession|ResumeRun|CloneRun|CompareRun|LaunchExperiment|RuntimeHealth|AgentReset|ResetAgent|CommandReceipt|ClientCommandID)[A-Za-z0-9_]*\b",
        r"Button\s*\(\s*\"Approve\"",
        r"Button\s*\(\s*\"Reject\"",
        r"Button\s*\(\s*\"Start Run\"",
        r"Button\s*\(\s*\"Cancel Run\"",
        r"Button\s*\(\s*\"Retry Stage\"",
    ],
}


@dataclass(frozen=True)
class GateResult:
    errors: list[str]

    @property
    def ok(self) -> bool:
        return not self.errors


def normalize_path(value: str) -> str:
    return value.strip().lstrip("./")


def normalize_identifier(value: str) -> str:
    return (
        value.strip()
        .lower()
        .replace("-", "_")
        .replace(" ", "_")
        .replace(".", "_")
    )


NORMALIZED_REQUIRED_IDENTIFIERS_BY_CONTROL = {
    normalize_identifier(control_id): identifiers
    for control_id, identifiers in REQUIRED_IDENTIFIERS_BY_CONTROL.items()
}


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def entry_path(entry: Any) -> str | None:
    if isinstance(entry, str):
        return normalize_path(entry)
    if isinstance(entry, dict) and isinstance(entry.get("path"), str):
        return normalize_path(entry["path"])
    return None


def collect_paths(entries: Any, key: str, errors: list[str]) -> set[str]:
    if not isinstance(entries, list):
        errors.append(f"inventory key {key} must be an array")
        return set()
    paths: set[str] = set()
    for index, entry in enumerate(entries):
        path = entry_path(entry)
        if not path:
            errors.append(f"inventory key {key}[{index}] must be a string or object with path")
            continue
        if Path(path).is_absolute() or ".." in Path(path).parts:
            errors.append(f"inventory key {key}[{index}] must be a repo-relative path: {path}")
            continue
        paths.add(path)
    return paths


def validate_degraded_fail_closed_entries(entries: Any, errors: list[str]) -> None:
    if not isinstance(entries, list):
        return
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(
                f"degraded_fail_closed_files[{index}] must be an object with a fail-closed contract"
            )
            continue
        if entry.get("degraded_state_only") is not True:
            errors.append(f"degraded_fail_closed_files[{index}] must set degraded_state_only to true")
        if entry.get("control_plane_truth_only") is not True:
            errors.append(f"degraded_fail_closed_files[{index}] must set control_plane_truth_only to true")
        if entry.get("restores_local_orchestration") is not False:
            errors.append(f"degraded_fail_closed_files[{index}] must set restores_local_orchestration to false")
        if entry.get("restores_local_writes") is not False:
            errors.append(f"degraded_fail_closed_files[{index}] must set restores_local_writes to false")


def validate_explicit_exclusion_entries(entries: Any, errors: list[str]) -> None:
    if not isinstance(entries, list):
        return
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(
                "explicit_exclusions[{index}] must be an object with path and reason".format(
                    index=index
                )
            )
            continue
        reason = entry.get("reason")
        if not isinstance(reason, str) or not reason.strip():
            errors.append(f"explicit_exclusions[{index}] must include a non-empty reason")


def paths_overlap(left: str, right: str) -> bool:
    return (
        left == right
        or left.startswith(f"{right.rstrip('/')}/")
        or right.startswith(f"{left.rstrip('/')}/")
    )


def validate_category_overlap(named_sets: dict[str, set[str]], errors: list[str]) -> None:
    seen: list[tuple[str, str]] = []
    for category, paths in named_sets.items():
        for path in sorted(paths):
            overlapped = next(
                ((seen_path, seen_category) for seen_path, seen_category in seen if paths_overlap(path, seen_path)),
                None,
            )
            if overlapped:
                seen_path, seen_category = overlapped
                errors.append(
                    f"inventory path {path} overlaps {seen_path} in both {seen_category} and {category}"
                )
            else:
                seen.append((path, category))


def collect_group_ids(entries: Any, errors: list[str]) -> set[str]:
    if not isinstance(entries, list):
        errors.append("inventory key forbidden_pattern_groups must be an array")
        return set()
    group_ids: set[str] = set()
    for index, entry in enumerate(entries):
        if isinstance(entry, str):
            group_id = entry
        elif isinstance(entry, dict):
            group_id = entry.get("id") or entry.get("name")
        else:
            group_id = None
        if not isinstance(group_id, str) or not group_id:
            errors.append(f"forbidden_pattern_groups[{index}] must name a group id")
            continue
        group_ids.add(group_id)
    return group_ids


def extra_patterns_by_group(entries: Any) -> dict[str, list[str]]:
    extra: dict[str, list[str]] = {}
    if not isinstance(entries, list):
        return extra
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        group_id = entry.get("id") or entry.get("name")
        patterns = entry.get("patterns")
        if isinstance(group_id, str) and isinstance(patterns, list):
            extra[group_id] = [pattern for pattern in patterns if isinstance(pattern, str)]
    return extra


def allowed_static_guard_matches(entries: Any, errors: list[str]) -> list[dict[str, str]]:
    if entries is None:
        return []
    if not isinstance(entries, list):
        errors.append("inventory key allowed_static_guard_matches must be an array when present")
        return []
    allowed: list[dict[str, str]] = []
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"allowed_static_guard_matches[{index}] must be an object")
            continue
        path = entry.get("path")
        group_id = entry.get("group_id")
        line_contains = entry.get("line_contains")
        reason = entry.get("reason")
        if not all(non_empty_string(value) for value in [path, group_id, line_contains, reason]):
            errors.append(
                f"allowed_static_guard_matches[{index}] must include path, group_id, line_contains, and reason"
            )
            continue
        allowed.append(
            {
                "path": normalize_path(path),
                "group_id": group_id.strip(),
                "line_contains": line_contains.strip(),
            }
        )
    return allowed


def is_allowed_static_match(
    rel_path: str,
    group_id: str,
    line_text: str,
    allowed_matches: list[dict[str, str]],
) -> bool:
    return any(
        rel_path == entry["path"]
        and group_id == entry["group_id"]
        and entry["line_contains"] in line_text
        for entry in allowed_matches
    )


def validate_embedded_graphql_documents(
    entries: Any,
    governed_paths: set[str],
    repo_root: Path,
    errors: list[str],
) -> set[str]:
    if not isinstance(entries, list):
        errors.append("inventory key embedded_graphql_documents must be an array")
        return set()
    paths: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"embedded_graphql_documents[{index}] must be an object")
            continue
        path = entry_path(entry)
        owner = entry.get("owner")
        operation_names = entry.get("operation_names")
        if not path:
            errors.append(f"embedded_graphql_documents[{index}] must include path")
            continue
        if path not in governed_paths:
            errors.append(
                f"embedded_graphql_documents[{index}] path must be governed by inventory: {path}"
            )
        if not non_empty_string(owner):
            errors.append(f"embedded_graphql_documents[{index}] must include owner")
        if not isinstance(operation_names, list) or not all(
            non_empty_string(item) for item in operation_names
        ):
            errors.append(
                f"embedded_graphql_documents[{index}] operation_names must be an array of strings"
            )
        paths.add(path)

    for rel_path in sorted(governed_paths):
        path = repo_root / rel_path
        if not path.is_file():
            continue
        try:
            text = path.read_text()
        except UnicodeDecodeError:
            continue
        if "P031GraphQLDocuments" in text and rel_path not in paths:
            errors.append(
                f"P031 embedded GraphQL documents are not inventoried: {rel_path}"
            )
    return paths


def repo_files(repo_root: Path, glob: str) -> set[str]:
    files: set[str] = set()
    if glob in GRAPHQL_DOCUMENT_GLOBS:
        suffix = Path(glob).suffix
        for dirpath, dirnames, filenames in os.walk(repo_root):
            rel_dir = normalize_path(Path(dirpath).relative_to(repo_root).as_posix())
            if rel_dir == ".":
                rel_dir = ""
            dirnames[:] = [
                dirname
                for dirname in dirnames
                if not normalize_path(f"{rel_dir}/{dirname}".lstrip("/")).startswith(
                    tuple(prefix.rstrip("/") for prefix in EXCLUDED_REPO_FILE_PREFIXES)
                )
                and dirname != "target"
            ]
            for filename in filenames:
                if not filename.endswith(suffix):
                    continue
                rel = normalize_path((Path(dirpath) / filename).relative_to(repo_root).as_posix())
                if not rel.startswith(EXCLUDED_REPO_FILE_PREFIXES) and "/target/" not in f"/{rel}/":
                    files.add(rel)
        return files

    for path in repo_root.glob(glob):
        if path.is_file():
            rel = normalize_path(path.relative_to(repo_root).as_posix())
            if not rel.startswith(EXCLUDED_REPO_FILE_PREFIXES) and "/target/" not in f"/{rel}/":
                files.add(rel)
    return files


def p031_owned_swift_files(repo_root: Path) -> set[str]:
    owned: set[str] = set()
    p031_symbol = re.compile(r"\bP031[A-Za-z0-9_]*\b")
    for rel_path in repo_files(repo_root, "Chainworks Forge/**/*.swift"):
        if Path(rel_path).name.lower().startswith("p031"):
            owned.add(rel_path)
            continue
        try:
            text = (repo_root / rel_path).read_text()
        except UnicodeDecodeError:
            continue
        if p031_symbol.search(text):
            owned.add(rel_path)
    return owned


def is_path_covered(repo_root: Path, rel_path: str, covered_paths: set[str]) -> bool:
    if rel_path in covered_paths:
        return True
    for covered in covered_paths:
        covered_file = repo_root / covered
        if covered_file.is_dir() and rel_path.startswith(f"{covered.rstrip('/')}/"):
            return True
    return False


def scan_targets(repo_root: Path, rel_path: str) -> tuple[list[Path], str | None]:
    path = repo_root / rel_path
    if path.is_file():
        return [path], None
    if path.is_dir():
        return sorted(candidate for candidate in path.rglob("*") if candidate.is_file()), None
    return [], f"governed path does not exist: {rel_path}"


def scan_forbidden_patterns(
    repo_root: Path,
    paths: Iterable[str],
    inventory: dict[str, Any],
    errors: list[str],
) -> None:
    group_ids = collect_group_ids(inventory.get("forbidden_pattern_groups"), errors)
    extra_patterns = extra_patterns_by_group(inventory.get("forbidden_pattern_groups"))
    allowed_matches = allowed_static_guard_matches(
        inventory.get("allowed_static_guard_matches"), errors
    )
    patterns_by_group: dict[str, list[re.Pattern[str]]] = {}
    for group_id in sorted(group_ids):
        raw_patterns = DEFAULT_FORBIDDEN_PATTERNS.get(group_id, []) + extra_patterns.get(group_id, [])
        compiled: list[re.Pattern[str]] = []
        for raw_pattern in raw_patterns:
            try:
                compiled.append(re.compile(raw_pattern))
            except re.error as exc:
                errors.append(f"forbidden pattern {group_id}:{raw_pattern!r} is invalid: {exc}")
        patterns_by_group[group_id] = compiled

    for inventory_path in sorted(paths):
        targets, target_error = scan_targets(repo_root, inventory_path)
        if target_error:
            errors.append(target_error)
            continue
        for path in targets:
            rel_path = normalize_path(path.relative_to(repo_root).as_posix())
            try:
                text = path.read_text()
            except UnicodeDecodeError:
                errors.append(f"governed path is not UTF-8 text: {rel_path}")
                continue
            for group_id, patterns in patterns_by_group.items():
                for pattern in patterns:
                    match = pattern.search(text)
                    if match:
                        line_number = text.count("\n", 0, match.start()) + 1
                        line_text = text.splitlines()[line_number - 1] if text.splitlines() else ""
                        if is_allowed_static_match(
                            rel_path, group_id, line_text, allowed_matches
                        ):
                            continue
                        errors.append(
                            f"{rel_path}:{line_number}: forbidden P031 {group_id} pattern matched {pattern.pattern!r}"
                        )


def validate_inventory(repo_root: Path) -> GateResult:
    errors: list[str] = []
    inventory_file = repo_root / INVENTORY_PATH
    if not inventory_file.is_file():
        return GateResult(
            [
                "proposal-031: missing docs/reference/p031-thin-ui-inventory.json; "
                "P031 gate fails closed until the docs-owned inventory artifact exists"
            ]
        )

    try:
        inventory = json.loads(inventory_file.read_text())
    except json.JSONDecodeError as exc:
        return GateResult([f"proposal-031: inventory is not valid JSON: {exc}"])

    if not isinstance(inventory, dict):
        return GateResult(["proposal-031: inventory must be a JSON object"])

    missing_keys = sorted(REQUIRED_TOP_LEVEL_KEYS - set(inventory))
    if missing_keys:
        errors.append(f"inventory missing required keys: {', '.join(missing_keys)}")

    if inventory.get("schema_version") != REQUIRED_SCHEMA_VERSION:
        errors.append(
            f"inventory schema_version must be {REQUIRED_SCHEMA_VERSION!r}, got {inventory.get('schema_version')!r}"
        )

    governed = collect_paths(inventory.get("governed_swift_files", []), "governed_swift_files", errors)
    graphql_docs = collect_paths(
        inventory.get("governed_graphql_documents", []), "governed_graphql_documents", errors
    )
    generated_graphql = collect_paths(
        inventory.get("generated_graphql_outputs", []), "generated_graphql_outputs", errors
    )
    degraded = collect_paths(
        inventory.get("degraded_fail_closed_files", []), "degraded_fail_closed_files", errors
    )
    exclusions = collect_paths(inventory.get("explicit_exclusions", []), "explicit_exclusions", errors)
    validate_degraded_fail_closed_entries(
        inventory.get("degraded_fail_closed_files", []), errors
    )
    validate_explicit_exclusion_entries(inventory.get("explicit_exclusions", []), errors)
    validate_category_overlap(
        {
            "governed_swift_files": governed,
            "governed_graphql_documents": graphql_docs,
            "generated_graphql_outputs": generated_graphql,
            "degraded_fail_closed_files": degraded,
            "explicit_exclusions": exclusions,
        },
        errors,
    )
    covered = governed | graphql_docs | generated_graphql | degraded | exclusions

    group_ids = collect_group_ids(inventory.get("forbidden_pattern_groups", []), errors)
    missing_groups = sorted(REQUIRED_FORBIDDEN_GROUPS - group_ids)
    if missing_groups:
        errors.append(f"inventory missing forbidden pattern groups: {', '.join(missing_groups)}")

    validate_embedded_graphql_documents(
        inventory.get("embedded_graphql_documents", []), governed | generated_graphql, repo_root, errors
    )

    for rel_path in INITIAL_GOVERNED_SURFACES:
        if (repo_root / rel_path).is_file() and rel_path not in governed:
            errors.append(f"initial P031 governed surface is not in governed_swift_files: {rel_path}")

    views = repo_files(repo_root, "Chainworks Forge/Views/**/*.swift")
    uncovered_views = sorted(path for path in views if not is_path_covered(repo_root, path, covered))
    for rel_path in uncovered_views:
        errors.append(f"Swift view lacks P031 inventory coverage: {rel_path}")

    uncovered_p031_swift = sorted(
        path
        for path in p031_owned_swift_files(repo_root)
        if not is_path_covered(repo_root, path, covered)
    )
    for rel_path in uncovered_p031_swift:
        errors.append(f"P031-owned Swift file lacks inventory coverage: {rel_path}")

    graphql_files: set[str] = set()
    for glob in GRAPHQL_DOCUMENT_GLOBS:
        graphql_files.update(repo_files(repo_root, glob))
    uncovered_graphql = sorted(
        path for path in graphql_files if not is_path_covered(repo_root, path, covered)
    )
    for rel_path in uncovered_graphql:
        errors.append(f"GraphQL operation lacks P031 inventory coverage: {rel_path}")

    scan_forbidden_patterns(repo_root, governed | graphql_docs | generated_graphql | degraded, inventory, errors)
    return GateResult(errors)


def validate_write_path_guide(repo_root: Path) -> GateResult:
    errors: list[str] = []
    guide_file = repo_root / WRITE_PATH_GUIDE_PATH
    if not guide_file.is_file():
        return GateResult(
            [
                "proposal-031: missing docs/reference/p031-operator-write-path-guide.json; "
                "P031 gate fails closed until the docs-owned operator write-path guide exists"
            ]
        )

    try:
        guide = json.loads(guide_file.read_text())
    except json.JSONDecodeError as exc:
        return GateResult([f"proposal-031: operator write-path guide is not valid JSON: {exc}"])

    if not isinstance(guide, dict):
        return GateResult(["proposal-031: operator write-path guide must be a JSON object"])

    if guide.get("schema_version") != REQUIRED_WRITE_PATH_GUIDE_SCHEMA_VERSION:
        errors.append(
            "operator write-path guide schema_version must be "
            f"{REQUIRED_WRITE_PATH_GUIDE_SCHEMA_VERSION!r}, got {guide.get('schema_version')!r}"
        )

    rows = guide.get("rows")
    if not isinstance(rows, list):
        errors.append("operator write-path guide key rows must be an array")
        return GateResult(errors)

    seen_control_ids: dict[str, int] = {}
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            errors.append(f"operator write-path guide rows[{index}] must be an object")
            continue

        missing_keys = sorted(REQUIRED_WRITE_PATH_ROW_KEYS - set(row))
        if missing_keys:
            errors.append(
                f"operator write-path guide rows[{index}] missing required keys: "
                f"{', '.join(missing_keys)}"
            )

        raw_control_id = row.get("removed_control_id")
        if not non_empty_string(raw_control_id):
            errors.append(f"operator write-path guide rows[{index}] removed_control_id must be non-empty")
            continue
        control_id = normalize_identifier(raw_control_id)
        if control_id in seen_control_ids:
            errors.append(
                "operator write-path guide duplicate removed_control_id "
                f"{raw_control_id!r} at rows[{index}] and rows[{seen_control_ids[control_id]}]"
            )
        seen_control_ids[control_id] = index

        if not non_empty_string(row.get("removed_control_label")):
            errors.append(f"operator write-path guide rows[{index}] removed_control_label must be non-empty")

        workflow_kind = row.get("external_workflow_kind")
        normalized_workflow_kind = normalize_identifier(workflow_kind) if isinstance(workflow_kind, str) else ""
        if normalized_workflow_kind not in ALLOWED_EXTERNAL_WORKFLOW_KINDS:
            errors.append(
                f"operator write-path guide rows[{index}] external_workflow_kind must be one of "
                f"{', '.join(sorted(ALLOWED_EXTERNAL_WORKFLOW_KINDS))}"
            )

        identifiers = row.get("required_identifiers")
        if not isinstance(identifiers, list) or not all(non_empty_string(item) for item in identifiers):
            errors.append(
                f"operator write-path guide rows[{index}] required_identifiers must be an array of strings"
            )
            normalized_identifiers: set[str] = set()
        else:
            normalized_identifiers = {normalize_identifier(item) for item in identifiers}
            if not normalized_identifiers:
                errors.append(
                    f"operator write-path guide rows[{index}] required_identifiers must not be empty"
                )

        required_for_control = NORMALIZED_REQUIRED_IDENTIFIERS_BY_CONTROL.get(control_id)
        if required_for_control:
            missing_identifiers = sorted(
                normalize_identifier(item)
                for item in required_for_control
                if normalize_identifier(item) not in normalized_identifiers
            )
            if missing_identifiers:
                errors.append(
                    f"operator write-path guide rows[{index}] for {raw_control_id} missing "
                    f"required identifiers: {', '.join(missing_identifiers)}"
                )

        validation_status = row.get("validation_status")
        normalized_validation_status = (
            normalize_identifier(validation_status) if isinstance(validation_status, str) else ""
        )
        if normalized_validation_status not in ALLOWED_VALIDATION_STATUSES:
            errors.append(
                f"operator write-path guide rows[{index}] validation_status must be one of "
                f"{', '.join(sorted(ALLOWED_VALIDATION_STATUSES))}"
            )

        has_parameter_shape = non_empty_string(row.get("minimum_parameter_shape"))
        has_unavailable_reason = non_empty_string(row.get("unavailable_reason"))
        has_success_output = non_empty_string(row.get("expected_success_output"))
        has_follow_up = non_empty_string(row.get("follow_up_id"))
        if not has_parameter_shape and not has_unavailable_reason:
            errors.append(
                f"operator write-path guide rows[{index}] must include minimum_parameter_shape "
                "or unavailable_reason"
            )
        if not has_success_output and not has_follow_up:
            errors.append(
                f"operator write-path guide rows[{index}] must include expected_success_output "
                "or follow_up_id"
            )

        if normalized_workflow_kind == "temporarily_unavailable":
            if not has_unavailable_reason or not has_follow_up:
                errors.append(
                    f"operator write-path guide rows[{index}] temporarily_unavailable rows must "
                    "include unavailable_reason and follow_up_id"
                )
        elif normalized_workflow_kind in ALLOWED_EXTERNAL_WORKFLOW_KINDS:
            if not non_empty_string(row.get("external_workflow_name_or_tool")):
                errors.append(
                    f"operator write-path guide rows[{index}] external_workflow_name_or_tool "
                    "must be non-empty for available external workflows"
                )
            if not has_success_output:
                errors.append(
                    f"operator write-path guide rows[{index}] expected_success_output must be "
                    "non-empty for available external workflows"
                )

    required_control_ids = {normalize_identifier(item) for item in REMOVED_WRITE_CONTROL_IDS}
    present_control_ids = set(seen_control_ids)
    missing_control_ids = [
        control_id
        for control_id in REMOVED_WRITE_CONTROL_IDS
        if normalize_identifier(control_id) not in present_control_ids
    ]
    if missing_control_ids:
        errors.append(
            "operator write-path guide missing removed-control coverage: "
            f"{', '.join(missing_control_ids)}"
        )
    unknown_control_ids = sorted(present_control_ids - required_control_ids)
    if unknown_control_ids:
        errors.append(
            "operator write-path guide contains unknown removed_control_id values: "
            f"{', '.join(unknown_control_ids)}"
        )

    return GateResult(errors)


def evidence_status(path: Path) -> str | None:
    if not path.is_file():
        return None
    try:
        for line in path.read_text().splitlines()[:20]:
            if line.startswith("Status:"):
                return line.split(":", 1)[1].strip().lower()
    except UnicodeDecodeError:
        return None
    return None


def validate_phase0_manifest(repo_root: Path) -> GateResult:
    errors: list[str] = []
    manifest_file = repo_root / PHASE0_MANIFEST_PATH
    if not manifest_file.is_file():
        return GateResult(
            [
                "proposal-031: missing docs/reference/p031-phase-0-artifact-manifest.json; "
                "P031 gate fails closed until Phase 0 artifacts are inventoried"
            ]
        )

    try:
        manifest = json.loads(manifest_file.read_text())
    except json.JSONDecodeError as exc:
        return GateResult([f"proposal-031: Phase 0 manifest is not valid JSON: {exc}"])

    if not isinstance(manifest, dict):
        return GateResult(["proposal-031: Phase 0 manifest must be a JSON object"])

    if manifest.get("schema_version") != REQUIRED_PHASE0_MANIFEST_SCHEMA_VERSION:
        errors.append(
            "Phase 0 manifest schema_version must be "
            f"{REQUIRED_PHASE0_MANIFEST_SCHEMA_VERSION!r}, got {manifest.get('schema_version')!r}"
        )

    entries = manifest.get("entries")
    if not isinstance(entries, list):
        errors.append("Phase 0 manifest key entries must be an array")
        return GateResult(errors)

    entries_by_id: dict[str, dict[str, Any]] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"Phase 0 manifest entries[{index}] must be an object")
            continue
        entry_id = entry.get("id")
        if not non_empty_string(entry_id):
            errors.append(f"Phase 0 manifest entries[{index}] must include id")
            continue
        if entry_id in entries_by_id:
            errors.append(f"Phase 0 manifest duplicate entry id: {entry_id}")
        entries_by_id[entry_id] = entry
        for key in ["path", "owner_role", "validation_status", "blocking_phase"]:
            if not non_empty_string(entry.get(key)):
                errors.append(f"Phase 0 manifest entry {entry_id} missing {key}")
        raw_path = entry.get("path")
        if non_empty_string(raw_path):
            artifact_path = repo_root / normalize_path(raw_path)
            if not artifact_path.is_file():
                errors.append(f"Phase 0 manifest entry {entry_id} points at missing artifact {raw_path}")
            status = evidence_status(artifact_path)
            validation_status = normalize_identifier(str(entry.get("validation_status", "")))
            if status in EVIDENCE_BLOCKING_STATUSES and validation_status == "ready":
                errors.append(
                    "Phase 0 manifest entry "
                    f"{entry_id} is marked ready but evidence status is {status.upper()}"
                )

    missing_entries = sorted(REQUIRED_MANIFEST_ENTRY_IDS - set(entries_by_id))
    if missing_entries:
        errors.append(f"Phase 0 manifest missing required entries: {', '.join(missing_entries)}")

    for entry_id, entry in sorted(entries_by_id.items()):
        validation_status = normalize_identifier(str(entry.get("validation_status", "")))
        if validation_status == PHASE0_MANIFEST_READY_STATUS:
            continue
        blocking_phase = str(entry.get("blocking_phase", "")).strip().lower()
        if blocking_phase in {"phase 0d", "phase 3"}:
            continue
        else:
            errors.append(
                f"Phase 0 manifest entry {entry_id} blocks {entry.get('blocking_phase')}: "
                f"{entry.get('validation_status')} ({entry.get('path')})"
            )

    return GateResult(errors)


def _git_rev(repo_root: Path, ref: str) -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", ref],
            cwd=str(repo_root),
            stderr=subprocess.DEVNULL,
            text=True,
        ).strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return ""


def _git_status_snapshot(repo_root: Path) -> tuple[str, int, str] | None:
    try:
        status = subprocess.check_output(
            ["git", "status", "--porcelain=v1", "--untracked-files=all"],
            cwd=str(repo_root),
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None
    line_count = sum(1 for line in status.splitlines() if line)
    return status, line_count, hashlib.sha256(status.encode()).hexdigest()


def _require_ready_provenance(
    errors: list[str],
    label: str,
    provenance: Any,
    live_commit: str,
    live_tree: str,
    live_status_line_count: int,
    live_status_sha256: str,
) -> None:
    if not isinstance(provenance, dict):
        errors.append(f"P041 {label} provenance must be an object for ready_same_tree_verified")
        return

    commit_sha = provenance.get("commit_sha")
    tree_id = provenance.get("tree_id")
    status_sha256 = provenance.get("status_snapshot_sha256")
    tree_clean = provenance.get("tree_clean")
    status_line_count = provenance.get("status_snapshot_line_count")

    for field, value in (
        ("commit_sha", commit_sha),
        ("tree_id", tree_id),
        ("status_snapshot_sha256", status_sha256),
    ):
        if not isinstance(value, str) or not value.strip():
            errors.append(
                f"P041 {label} provenance requires non-empty {field} "
                "for ready_same_tree_verified live git provenance"
            )

    if commit_sha and commit_sha != live_commit:
        errors.append(
            f"P041 {label} provenance commit_sha {commit_sha[:12]} does not match "
            f"live HEAD {live_commit[:12]}; rerun './scripts/test-gate.sh proposal-041'"
        )
    if tree_id and tree_id != live_tree:
        errors.append(
            f"P041 {label} provenance tree_id {tree_id[:12]} does not match "
            f"live HEAD^{{tree}} {live_tree[:12]}; rerun './scripts/test-gate.sh proposal-041'"
        )
    if tree_clean is not True:
        errors.append(
            f"P041 {label} provenance must set tree_clean=true for ready_same_tree_verified"
        )
    if status_line_count != 0:
        errors.append(
            f"P041 {label} provenance status_snapshot_line_count is "
            f"{status_line_count} (expected 0) for ready_same_tree_verified"
        )
    if status_line_count != live_status_line_count:
        errors.append(
            f"P041 {label} provenance status_snapshot_line_count {status_line_count} "
            f"does not match live status line count {live_status_line_count}"
        )
    if status_sha256 and status_sha256 != live_status_sha256:
        errors.append(
            f"P041 {label} provenance status_snapshot_sha256 does not match "
            "the live git status snapshot"
        )


def validate_p041_parity_row(repo_root: Path) -> GateResult:
    errors: list[str] = []
    row_path = repo_root / P041_RUNTIME_ROW_PATH
    if not row_path.is_file():
        errors.append(
            f"P041 runtime row not found at {P041_RUNTIME_ROW_PATH}; "
            "run './scripts/test-gate.sh proposal-041' on a clean tree first"
        )
        return GateResult(errors)

    try:
        row = json.loads(row_path.read_text())
    except (json.JSONDecodeError, OSError) as exc:
        errors.append(f"P041 runtime row unreadable: {exc}")
        return GateResult(errors)

    if row.get("schema_version") != P041_ROW_SCHEMA_VERSION:
        errors.append(
            f"P041 runtime row schema_version mismatch: "
            f"expected {P041_ROW_SCHEMA_VERSION}, got {row.get('schema_version')}"
        )

    if row.get("detail_schema_version") != P041_DETAIL_SCHEMA_VERSION:
        errors.append(
            f"P041 runtime row detail_schema_version mismatch: "
            f"expected {P041_DETAIL_SCHEMA_VERSION}, got {row.get('detail_schema_version')}"
        )

    # Validate runtime_detail_path and reference_detail_path before checking readiness.
    # These path fields direct downstream consumers to the canonical artifacts; an
    # absolute path, traversal sequence, or wrong canonical path is a consumer safety issue
    # regardless of the overall validation_status (proposal Section 6.6 Decision 3 / PREPUSH-BLOCK-003).
    _CANONICAL_RUNTIME_DETAIL = str(P041_RUNTIME_DETAIL_PATH)
    _CANONICAL_REFERENCE_DETAIL = "docs/reference/p031-p041-parity-evidence.json"

    runtime_detail_path = row.get("runtime_detail_path", "")
    if not runtime_detail_path:
        errors.append("P041 row missing runtime_detail_path")
    elif not isinstance(runtime_detail_path, str):
        errors.append(
            f"P041 row.runtime_detail_path must be a string, got: {type(runtime_detail_path).__name__}"
        )
    elif runtime_detail_path.startswith("/") or ".." in runtime_detail_path:
        errors.append(
            f"P041 row.runtime_detail_path must be a relative canonical path with no "
            f"'..', got: {runtime_detail_path!r}"
        )
    elif runtime_detail_path != _CANONICAL_RUNTIME_DETAIL:
        errors.append(
            f"P041 row.runtime_detail_path must equal "
            f"{_CANONICAL_RUNTIME_DETAIL!r}, got: {runtime_detail_path!r}"
        )

    reference_detail_path = row.get("reference_detail_path", "")
    if not reference_detail_path:
        errors.append("P041 row missing reference_detail_path")
    elif not isinstance(reference_detail_path, str):
        errors.append(
            f"P041 row.reference_detail_path must be a string, got: {type(reference_detail_path).__name__}"
        )
    elif reference_detail_path.startswith("/") or ".." in reference_detail_path:
        errors.append(
            f"P041 row.reference_detail_path must be a relative canonical path with no "
            f"'..', got: {reference_detail_path!r}"
        )
    elif reference_detail_path != _CANONICAL_REFERENCE_DETAIL:
        errors.append(
            f"P041 row.reference_detail_path must equal "
            f"{_CANONICAL_REFERENCE_DETAIL!r}, got: {reference_detail_path!r}"
        )

    ready = row.get("validation_status") == P041_READY_STATUS
    if not ready:
        errors.append(
            f"P041 runtime row validation_status is "
            f"{row.get('validation_status')!r}, not {P041_READY_STATUS!r}; "
            "P031 acceptance requires ready_same_tree_verified — rerun "
            "'./scripts/test-gate.sh proposal-041' on a clean tree first"
        )
    live_commit = _git_rev(repo_root, "HEAD")
    live_tree = _git_rev(repo_root, "HEAD^{tree}")
    live_status = _git_status_snapshot(repo_root) if ready else None
    prov = row.get("provenance", {})
    row_commit = prov.get("commit_sha", "")
    row_tree = prov.get("tree_id", "")

    if live_commit and row_commit and row_commit != live_commit:
        errors.append(
            f"P041 runtime row commit_sha {row_commit[:12]} does not match live HEAD "
            f"{live_commit[:12]}; rerun './scripts/test-gate.sh proposal-041'"
        )
    if live_tree and row_tree and row_tree != live_tree:
        errors.append(
            f"P041 runtime row tree_id {row_tree[:12]} does not match live HEAD^{{tree}} "
            f"{live_tree[:12]}; rerun './scripts/test-gate.sh proposal-041'"
        )

    if ready:
        if not live_commit or not live_tree or live_status is None:
            errors.append(
                "P041 ready_same_tree_verified requires live git provenance "
                "(HEAD, HEAD^{tree}, and status snapshot) to be available"
            )
        else:
            _status_text, live_status_line_count, live_status_sha256 = live_status
            if live_status_line_count != 0:
                errors.append(
                    "P041 ready_same_tree_verified requires a clean live git status snapshot"
                )
            _require_ready_provenance(
                errors,
                "runtime row",
                prov,
                live_commit,
                live_tree,
                live_status_line_count,
                live_status_sha256,
            )

    detail_path = repo_root / P041_RUNTIME_DETAIL_PATH
    if detail_path.is_file():
        try:
            detail = json.loads(detail_path.read_text())
            if detail.get("schema_version") != P041_DETAIL_SCHEMA_VERSION:
                errors.append(
                    f"P041 runtime detail schema_version mismatch: "
                    f"expected {P041_DETAIL_SCHEMA_VERSION}, got {detail.get('schema_version')}"
                )
            if row.get("validation_status") != detail.get("overall_status"):
                errors.append(
                    "P041 row.validation_status does not equal detail.overall_status: "
                    f"{row.get('validation_status')} != {detail.get('overall_status')}"
                )
            if row.get("publication_state") != detail.get("publication_state"):
                errors.append(
                    "P041 row.publication_state does not equal detail.publication_state"
                )
            if row.get("publication_generation_id") != detail.get("publication_generation_id"):
                errors.append(
                    "P041 row.publication_generation_id does not equal detail.publication_generation_id"
                )
            # Section 6.2 / 6.6 Decision 4: for ready publication, detail.provenance must
            # agree with row.provenance AND with the live checkout. Checking only row.provenance
            # against live is insufficient — a stale detail with matching row provenance would pass.
            if ready:
                detail_prov = detail.get("provenance", {})
                prov_fields = (
                    "commit_sha", "tree_id", "tree_clean",
                    "status_snapshot_sha256", "status_snapshot_line_count",
                )
                for pfield in prov_fields:
                    dv = detail_prov.get(pfield)
                    rv = prov.get(pfield)
                    if dv != rv:
                        errors.append(
                            f"P041 detail.provenance.{pfield} ({dv!r}) does not match "
                            f"row.provenance.{pfield} ({rv!r}); "
                            "row and detail provenance must agree for ready publication"
                        )
                if live_commit and live_tree and live_status is not None:
                    _status_text, live_status_line_count, live_status_sha256 = live_status
                    _require_ready_provenance(
                        errors,
                        "runtime detail",
                        detail_prov,
                        live_commit,
                        live_tree,
                        live_status_line_count,
                        live_status_sha256,
                    )
        except (json.JSONDecodeError, OSError) as exc:
            errors.append(f"P041 runtime detail unreadable: {exc}")
    elif ready:
        errors.append(
            f"P041 ready_same_tree_verified requires runtime detail at {P041_RUNTIME_DETAIL_PATH}"
        )

    return GateResult(errors)


def validate_p031_contracts(repo_root: Path) -> GateResult:
    errors: list[str] = []
    errors.extend(validate_inventory(repo_root).errors)
    errors.extend(validate_write_path_guide(repo_root).errors)
    errors.extend(validate_phase0_manifest(repo_root).errors)
    errors.extend(validate_p041_parity_row(repo_root).errors)
    return GateResult(errors)


def run_gate(repo_root: Path) -> int:
    result = validate_p031_contracts(repo_root)
    if result.ok:
        print("proposal-031: thin UI inventory, static guards, and write-path guide passed")
        return 0
    for error in result.errors:
        print(error)
    return 1


class P031ThinUIGateTests(unittest.TestCase):
    def make_repo(self) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        (root / "docs/reference").mkdir(parents=True)
        (root / "Chainworks Forge/Views").mkdir(parents=True)
        return root

    def write_inventory(self, root: Path, **updates: Any) -> None:
        inventory: dict[str, Any] = {
            "schema_version": REQUIRED_SCHEMA_VERSION,
            "governed_swift_files": ["Chainworks Forge/Views/RunsHomeView.swift"],
            "governed_graphql_documents": [],
            "embedded_graphql_documents": [],
            "generated_graphql_outputs": [],
            "degraded_fail_closed_files": [],
            "explicit_exclusions": [],
            "forbidden_pattern_groups": sorted(REQUIRED_FORBIDDEN_GROUPS),
        }
        inventory.update(updates)
        (root / INVENTORY_PATH).write_text(json.dumps(inventory))

    def complete_guide_rows(self) -> list[dict[str, Any]]:
        rows: list[dict[str, Any]] = []
        for control_id in REMOVED_WRITE_CONTROL_IDS:
            if control_id == "approvals.resolve":
                identifiers = ["approval_id", "run_id", "stage_id"]
            elif control_id in {"ideas.create", "runs.start"}:
                identifiers = ["idea_id"]
            elif control_id in {"stages.retry"}:
                identifiers = ["run_id", "stage_id"]
            else:
                identifiers = ["run_id"]
            rows.append(
                {
                    "removed_control_id": control_id,
                    "removed_control_label": control_id,
                    "external_workflow_kind": "temporarily unavailable",
                    "external_workflow_name_or_tool": None,
                    "required_identifiers": identifiers,
                    "minimum_parameter_shape": None,
                    "unavailable_reason": "P031-FOLLOWUP-WRITE-PATH",
                    "expected_success_output": None,
                    "follow_up_id": "P031-FOLLOWUP-WRITE-PATH",
                    "operator_notes": None,
                    "validation_status": "pending",
                }
            )
        return rows

    def write_guide(self, root: Path, rows: list[dict[str, Any]] | None = None, **updates: Any) -> None:
        guide: dict[str, Any] = {
            "schema_version": REQUIRED_WRITE_PATH_GUIDE_SCHEMA_VERSION,
            "rows": rows if rows is not None else self.complete_guide_rows(),
        }
        guide.update(updates)
        (root / WRITE_PATH_GUIDE_PATH).write_text(json.dumps(guide))

    def write_p041_runtime(
        self,
        root: Path,
        row_updates: dict[str, Any] | None = None,
        detail_updates: dict[str, Any] | None = None,
    ) -> None:
        publication_root = root / "control-plane/target/parity/publication/current"
        publication_root.mkdir(parents=True)
        provenance = {
            "commit_sha": "",
            "tree_id": "",
            "tree_clean": True,
            "status_snapshot_sha256": "",
            "status_snapshot_line_count": 0,
        }
        row: dict[str, Any] = {
            "schema_version": P041_ROW_SCHEMA_VERSION,
            "id": "p041_parity_evidence",
            "runtime_detail_path": str(P041_RUNTIME_DETAIL_PATH),
            "reference_detail_path": "docs/reference/p031-p041-parity-evidence.json",
            "validation_status": P041_READY_STATUS,
            "publication_state": "published_ready",
            "publication_generation_id": "gen-test",
            "detail_schema_version": P041_DETAIL_SCHEMA_VERSION,
            "provenance": dict(provenance),
        }
        detail: dict[str, Any] = {
            "schema_version": P041_DETAIL_SCHEMA_VERSION,
            "overall_status": P041_READY_STATUS,
            "publication_state": "published_ready",
            "publication_generation_id": "gen-test",
            "provenance": dict(provenance),
        }
        if row_updates:
            row.update(row_updates)
        if detail_updates:
            detail.update(detail_updates)
        (publication_root / "p031-phase-0-manifest-row.json").write_text(json.dumps(row))
        (publication_root / "p031-p041-parity-evidence.json").write_text(json.dumps(detail))

    def test_missing_inventory_fails_closed(self) -> None:
        root = self.make_repo()
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertIn("missing docs/reference/p031-thin-ui-inventory.json", result.errors[0])

    def test_missing_write_path_guide_fails_closed(self) -> None:
        root = self.make_repo()
        result = validate_write_path_guide(root)
        self.assertFalse(result.ok)
        self.assertIn("missing docs/reference/p031-operator-write-path-guide.json", result.errors[0])

    def test_p041_non_ready_row_is_rejected_directly(self) -> None:
        for non_ready_status in (
            "blocked_missing_evidence",
            "blocked_divergence",
            "blocked_dirty_tree",
            "blocked_timeout",
            "blocked_interrupted",
            "blocked_in_progress",
            "blocked_manual_recovery",
        ):
            with self.subTest(validation_status=non_ready_status):
                root = self.make_repo()
                self.write_p041_runtime(
                    root,
                    row_updates={
                        "validation_status": non_ready_status,
                        "publication_state": "blocked",
                    },
                    detail_updates={
                        "overall_status": non_ready_status,
                        "publication_state": "blocked",
                    },
                )

                result = validate_p041_parity_row(root)

                self.assertFalse(result.ok, f"expected error for status {non_ready_status}")
                self.assertTrue(
                    any(non_ready_status in error for error in result.errors),
                    f"expected {non_ready_status!r} named in errors: {result.errors}",
                )
                self.assertTrue(
                    any("ready_same_tree_verified" in error for error in result.errors),
                    f"expected ready_same_tree_verified named in errors: {result.errors}",
                )

    def test_p041_ready_runtime_requires_live_git_provenance(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(root)

        result = validate_p041_parity_row(root)

        self.assertFalse(result.ok)
        self.assertTrue(
            any("live git provenance" in error for error in result.errors),
            result.errors,
        )

    def test_p041_row_runtime_detail_path_rejects_absolute(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(
            root,
            row_updates={"runtime_detail_path": "/absolute/path/p031-p041-parity-evidence.json"},
        )
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("runtime_detail_path" in e and "relative" in e for e in result.errors),
            f"expected relative-path error, got: {result.errors}",
        )

    def test_p041_row_runtime_detail_path_rejects_traversal(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(
            root,
            row_updates={"runtime_detail_path": "control-plane/../target/parity/publication/current/p031-p041-parity-evidence.json"},
        )
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("runtime_detail_path" in e and ".." in e for e in result.errors),
            f"expected traversal error, got: {result.errors}",
        )

    def test_p041_row_runtime_detail_path_rejects_mismatched(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(
            root,
            row_updates={"runtime_detail_path": "wrong/path/parity-evidence.json"},
        )
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("runtime_detail_path" in e for e in result.errors),
            f"expected runtime_detail_path error, got: {result.errors}",
        )

    def test_p041_row_runtime_detail_path_rejects_missing(self) -> None:
        root = self.make_repo()
        row_updates: dict[str, Any] = {}
        row_updates["runtime_detail_path"] = ""  # type: ignore[assignment]
        self.write_p041_runtime(root, row_updates=row_updates)
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("runtime_detail_path" in e for e in result.errors),
            f"expected runtime_detail_path error, got: {result.errors}",
        )

    def test_p041_row_runtime_detail_path_rejects_non_string(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(root, row_updates={"runtime_detail_path": 42})
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("runtime_detail_path" in e and "string" in e for e in result.errors),
            f"expected runtime_detail_path type error, got: {result.errors}",
        )

    def test_p041_row_reference_detail_path_rejects_absolute(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(
            root,
            row_updates={"reference_detail_path": "/docs/reference/p031-p041-parity-evidence.json"},
        )
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("reference_detail_path" in e and "relative" in e for e in result.errors),
            f"expected relative-path error, got: {result.errors}",
        )

    def test_p041_row_reference_detail_path_rejects_traversal(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(
            root,
            row_updates={"reference_detail_path": "docs/../reference/p031-p041-parity-evidence.json"},
        )
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("reference_detail_path" in e and ".." in e for e in result.errors),
            f"expected traversal error, got: {result.errors}",
        )

    def test_p041_row_reference_detail_path_rejects_mismatched(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(
            root,
            row_updates={"reference_detail_path": "docs/reference/other-evidence.json"},
        )
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("reference_detail_path" in e for e in result.errors),
            f"expected reference_detail_path error, got: {result.errors}",
        )

    def test_p041_row_reference_detail_path_rejects_non_string(self) -> None:
        root = self.make_repo()
        self.write_p041_runtime(root, row_updates={"reference_detail_path": 42})
        result = validate_p041_parity_row(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("reference_detail_path" in e and "string" in e for e in result.errors),
            f"expected reference_detail_path type error, got: {result.errors}",
        )

    def test_valid_write_path_guide_passes(self) -> None:
        root = self.make_repo()
        self.write_guide(root)
        self.assertTrue(validate_write_path_guide(root).ok)

    def test_write_path_guide_requires_complete_removed_control_coverage(self) -> None:
        root = self.make_repo()
        rows = [
            row
            for row in self.complete_guide_rows()
            if row["removed_control_id"] != "runs.cancel"
        ]
        self.write_guide(root, rows=rows)
        result = validate_write_path_guide(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("runs.cancel" in error for error in result.errors), result.errors)

    def test_write_path_guide_rows_require_contract_fields(self) -> None:
        root = self.make_repo()
        rows = self.complete_guide_rows()
        rows[0] = {
            "removed_control_id": "ideas.create",
            "removed_control_label": "",
            "external_workflow_kind": "CLI",
            "external_workflow_name_or_tool": "",
            "required_identifiers": [],
            "validation_status": "unknown",
        }
        self.write_guide(root, rows=rows)
        result = validate_write_path_guide(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("missing required keys" in error for error in result.errors), result.errors)
        self.assertTrue(any("removed_control_label must be non-empty" in error for error in result.errors))
        self.assertTrue(any("required_identifiers must not be empty" in error for error in result.errors))

    def test_write_path_guide_approval_resolution_requires_copied_identifiers(self) -> None:
        root = self.make_repo()
        rows = self.complete_guide_rows()
        for row in rows:
            if row["removed_control_id"] == "approvals.resolve":
                row["required_identifiers"] = ["approval_id", "run_id"]
        self.write_guide(root, rows=rows)
        result = validate_write_path_guide(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("stage_id" in error for error in result.errors), result.errors)

    def test_write_path_guide_requires_identifiers_for_normalized_approval_control_id(self) -> None:
        root = self.make_repo()
        rows = self.complete_guide_rows()
        for row in rows:
            if row["removed_control_id"] == "approvals.resolve":
                row["removed_control_id"] = "approvals resolve"
                row["required_identifiers"] = ["approval_id", "run_id"]
        self.write_guide(root, rows=rows)
        result = validate_write_path_guide(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("stage_id" in error for error in result.errors), result.errors)

    def test_write_path_guide_removed_controls_require_control_specific_identifiers(self) -> None:
        scenarios = [
            ("runs.cancel", ["stage_id"], "run_id"),
            ("stages.retry", ["run_id"], "stage_id"),
            ("runs.start", ["run_id"], "idea_id"),
            ("agents.reset", ["agent_id"], "run_id"),
        ]
        for control_id, identifiers, expected_missing in scenarios:
            with self.subTest(control_id=control_id):
                root = self.make_repo()
                rows = self.complete_guide_rows()
                for row in rows:
                    if row["removed_control_id"] == control_id:
                        row["required_identifiers"] = identifiers
                self.write_guide(root, rows=rows)
                result = validate_write_path_guide(root)
                self.assertFalse(result.ok)
                self.assertTrue(any(expected_missing in error for error in result.errors), result.errors)

    def test_valid_inventory_passes_clean_view(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("import SwiftUI\nstruct RunsHomeView {}\n")
        self.write_inventory(root)
        self.assertTrue(validate_inventory(root).ok)

    def test_inventory_can_govern_p031_support_boundary_with_explicit_contract_literals(
        self,
    ) -> None:
        root = self.make_repo()
        support_path = "Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift"
        (root / "Chainworks Forge/Support").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / support_path).write_text(
            "\n".join(
                [
                    "enum P031GraphQLDocuments {",
                    "  static let runsHome = \"\"\"query P031RunsHome { runs { id } }\"\"\"",
                    "}",
                    "let rejectedMessage = \"P031 UI must not execute GraphQL mutation operation\"",
                    "let removedControlIDs = [\"ideas.create\", \"runs.start\", \"approvals.resolve\"]",
                ]
            )
        )
        self.write_inventory(
            root,
            governed_swift_files=[
                "Chainworks Forge/Views/RunsHomeView.swift",
                support_path,
            ],
            embedded_graphql_documents=[
                {
                    "path": support_path,
                    "owner": "P031GraphQLDocuments",
                    "operation_names": ["P031RunsHome"],
                }
            ],
            allowed_static_guard_matches=[
                {
                    "path": support_path,
                    "group_id": "graphql_mutations",
                    "line_contains": "must not execute GraphQL mutation operation",
                    "reason": "validator rejection copy names the forbidden operation kind",
                },
                {
                    "path": support_path,
                    "group_id": "removed_write_controls",
                    "line_contains": "removedControlIDs",
                    "reason": "write-path guide contract enumerates removed controls as data",
                },
            ],
        )

        self.assertTrue(validate_inventory(root).ok)

    def test_manifest_requires_phase0d_and_release_evidence_entries(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        self.write_inventory(root)
        self.write_guide(root)
        (root / "docs/reference/p031-phase-0-artifact-manifest.json").write_text(
            json.dumps(
                {
                    "schema_version": "p031-phase-0-artifact-manifest-v1",
                    "status": "ready",
                    "entries": [
                        {
                            "id": "governing_contract",
                            "path": "docs/reference/query-projections-and-client-consumption-contract.md",
                            "owner_role": "P031 release owner",
                            "validation_status": "ready",
                            "blocking_phase": "Phase 1",
                        }
                    ],
                }
            )
        )

        result = validate_p031_contracts(root)

        self.assertFalse(result.ok)
        self.assertTrue(any("manifest missing required entries" in error for error in result.errors), result.errors)
        self.assertTrue(any("degraded_state_evidence" in error for error in result.errors), result.errors)

    def test_manifest_ready_entry_cannot_point_at_blocked_evidence(self) -> None:
        root = self.make_repo()
        evidence_path = "docs/evidence/p031-freshness-baseline.md"
        (root / "docs/evidence").mkdir(parents=True)
        (root / evidence_path).write_text(
            "\n".join(
                [
                    "# P031 Freshness Baseline",
                    "",
                    "Status: BLOCKED",
                    "Owner: P031 macOS thin UI owner",
                    "Blocking Phase: Phase 0d",
                ]
            )
        )
        (root / PHASE0_MANIFEST_PATH).write_text(
            json.dumps(
                {
                    "schema_version": REQUIRED_PHASE0_MANIFEST_SCHEMA_VERSION,
                    "entries": [
                        {
                            "id": "freshness_baseline",
                            "path": evidence_path,
                            "owner_role": "P031 macOS thin UI owner",
                            "validation_status": "ready",
                            "blocking_phase": "Phase 0d",
                        }
                    ],
                }
            )
        )

        result = validate_phase0_manifest(root)

        self.assertFalse(result.ok)
        self.assertTrue(
            any("evidence status is BLOCKED" in error for error in result.errors),
            result.errors,
        )

    def test_manifest_allows_later_phase_blockers_when_they_are_not_marked_ready(self) -> None:
        root = self.make_repo()
        (root / "docs/evidence").mkdir(parents=True)
        evidence_path = "docs/evidence/p031-dogfood-signoff.md"
        (root / evidence_path).write_text(
            "\n".join(
                [
                    "# P031 Dogfood Sign-Off Template",
                    "",
                    "Status: BLOCKED",
                    "Owner: P031 release owner",
                    "Blocking Phase: Phase 3",
                ]
            )
        )
        (root / PHASE0_MANIFEST_PATH).write_text(
            json.dumps(
                {
                    "schema_version": REQUIRED_PHASE0_MANIFEST_SCHEMA_VERSION,
                    "entries": [
                        {
                            "id": "dogfood_signoff_template",
                            "path": evidence_path,
                            "owner_role": "P031 release owner",
                            "validation_status": "blocked",
                            "blocking_phase": "Phase 3",
                        }
                    ],
                }
            )
        )

        result = validate_phase0_manifest(root)

        self.assertTrue(
            all("Phase 0 manifest entry dogfood_signoff_template blocks" not in error for error in result.errors),
            result.errors,
        )

    def test_degraded_fail_closed_entries_require_control_plane_truth_contract(self) -> None:
        root = self.make_repo()
        degraded_path = "Chainworks Forge/Views/DegradedRunsHomeView.swift"
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / degraded_path).write_text("struct DegradedRunsHomeView {}\n")

        self.write_inventory(root, degraded_fail_closed_files=[degraded_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("degraded_fail_closed_files[0] must be an object" in error for error in result.errors),
            result.errors,
        )

        self.write_inventory(
            root,
            degraded_fail_closed_files=[
                {
                    "path": degraded_path,
                    "degraded_state_only": True,
                    "control_plane_truth_only": True,
                    "restores_local_orchestration": False,
                    "restores_local_writes": False,
                }
            ],
        )
        self.assertTrue(validate_inventory(root).ok)

    def test_degraded_fail_closed_files_are_scanned_for_local_orchestration(self) -> None:
        root = self.make_repo()
        degraded_path = "Chainworks Forge/Views/DegradedRunsHomeView.swift"
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / degraded_path).write_text("let service: ExecutionService\n")

        self.write_inventory(
            root,
            degraded_fail_closed_files=[
                {
                    "path": degraded_path,
                    "degraded_state_only": True,
                    "control_plane_truth_only": True,
                    "restores_local_orchestration": False,
                    "restores_local_writes": False,
                }
            ],
        )

        result = validate_inventory(root)

        self.assertFalse(result.ok)
        self.assertTrue(
            any(
                degraded_path in error and "forbidden P031 local_write_fallback pattern" in error
                for error in result.errors
            ),
            result.errors,
        )

    def test_inventory_path_category_overlap_fails_closed(self) -> None:
        root = self.make_repo()
        view_path = "Chainworks Forge/Views/RunsHomeView.swift"
        (root / view_path).write_text("struct RunsHomeView {}\n")
        self.write_inventory(
            root,
            explicit_exclusions=[view_path],
        )
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("overlaps" in error for error in result.errors), result.errors)

    def test_inventory_path_category_nested_overlap_fails_closed(self) -> None:
        root = self.make_repo()
        view_path = "Chainworks Forge/Views/RunsHomeView.swift"
        (root / view_path).write_text("struct RunsHomeView {}\n")
        self.write_inventory(
            root,
            governed_swift_files=["Chainworks Forge/Views"],
            explicit_exclusions=[
                {"path": view_path, "reason": "attempted nested exclusion"}
            ],
        )
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("overlaps" in error for error in result.errors), result.errors)

    def test_explicit_exclusions_require_review_reason(self) -> None:
        root = self.make_repo()
        view_path = "Chainworks Forge/Views/RunsHomeView.swift"
        excluded_path = "Chainworks Forge/Views/DebugOnlyView.swift"
        (root / view_path).write_text("struct RunsHomeView {}\n")
        (root / excluded_path).write_text("struct DebugOnlyView {}\n")

        self.write_inventory(root, explicit_exclusions=[excluded_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any("explicit_exclusions[0] must be an object" in error for error in result.errors)
        )

        self.write_inventory(
            root,
            explicit_exclusions=[
                {"path": excluded_path, "reason": "debug-only diagnostic surface"}
            ],
        )
        self.assertTrue(validate_inventory(root).ok)

    def test_uncovered_view_fails(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / "Chainworks Forge/Views/NewThinView.swift").write_text("struct NewThinView {}\n")
        self.write_inventory(root)
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("NewThinView.swift" in error for error in result.errors))

    def test_uncovered_p031_support_file_fails(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Support").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / "Chainworks Forge/Support/P031ThinStore.swift").write_text("struct P031ThinStore {}\n")
        self.write_inventory(root)
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("P031ThinStore.swift" in error for error in result.errors), result.errors)

    def test_uncovered_p031_support_symbol_fails_even_without_p031_filename(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Support").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        support_path = "Chainworks Forge/Support/ThinWorkflowReadStore.swift"
        (root / support_path).write_text("struct ThinWorkflowReadStore: P031WorkflowReadStore {}\n")
        self.write_inventory(root)
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any(support_path in error for error in result.errors), result.errors)

    def test_forbidden_pattern_fails(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("let client = MCPCommandClient()\n")
        self.write_inventory(root)
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("forbidden P031 mcp pattern" in error for error in result.errors))

    def test_removed_write_control_patterns_fail(self) -> None:
        snippets = [
            ("ideas_create", "ideas.create([:])"),
            ("runs_start", "runs.start(id)"),
            ("runs_cancel", "runs.cancel(id)"),
            ("stages_retry", "stages.retry(id)"),
            ("approvals_resolve", "approvals.resolve(id)"),
            ("steward_run_analysis", "steward.run_analysis(id)"),
            ("session_reset", "session.reset()"),
            ("session_resume", "session.resume()"),
            ("runs_clone", "runs.clone(id)"),
            ("runs_compare", "runs.compare(lhs, rhs)"),
            ("experiments_launch", "experiments.launch(template)"),
            ("runtime_health", "runtime.health()"),
            ("agents_reset", "agents.reset(id)"),
            ("agent_reset", "agent.reset(id)"),
            ("approve_button", "Button(\"Approve\") {}"),
            ("reject_button", "Button(\"Reject\") {}"),
            ("start_run_button", "Button(\"Start Run\") {}"),
            ("cancel_run_button", "Button(\"Cancel Run\") {}"),
            ("retry_stage_button", "Button(\"Retry Stage\") {}"),
        ]
        for name, snippet in snippets:
            with self.subTest(name=name):
                root = self.make_repo()
                (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text(f"{snippet}\n")
                self.write_inventory(root)
                result = validate_inventory(root)
                self.assertFalse(result.ok)
                self.assertTrue(
                    any("forbidden P031 removed_write_controls pattern" in error for error in result.errors),
                    result.errors,
                )

    def test_local_write_fallback_patterns_fail(self) -> None:
        snippets = [
            ("execution_service", "let service: ExecutionService"),
            ("recovery_coordinator", "let recovery = RecoveryCoordinator()"),
            ("run_plan_compiler", "RunPlanCompiler.compile(path)"),
            ("swift_data", "import SwiftData"),
            ("model_context", "@Environment(\\.modelContext) var modelContext: ModelContext"),
            ("resolve_approval", "resolveApproval(id)"),
            ("start_run", "startRun(request)"),
            ("cancel_run", "cancelRun(id)"),
            ("retry_stage", "retryStage(id)"),
            ("reset_session", "resetSession()"),
            ("resume_run", "resumeRun(id)"),
            ("clone_run", "cloneRun(id)"),
            ("compare_runs", "compareRuns(lhs, rhs)"),
            ("launch_experiment", "launchExperiment(config)"),
            ("runtime_health", "runtimeHealth.refresh()"),
            ("agent_reset", "agentReset(id)"),
            ("reset_agent", "resetAgent(id)"),
        ]
        for name, snippet in snippets:
            with self.subTest(name=name):
                root = self.make_repo()
                (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text(f"{snippet}\n")
                self.write_inventory(root)
                result = validate_inventory(root)
                self.assertFalse(result.ok)
                self.assertTrue(
                    any("forbidden P031 local_write_fallback pattern" in error for error in result.errors),
                    result.errors,
                )

    def test_command_plumbing_patterns_fail(self) -> None:
        snippets = [
            ("action_invocation_identity", "let identity = ActionInvocationIdentity()"),
            ("client_command_id", 'let id = "client_command_id"'),
            ("command_receipt", "let receipt: CommandReceipt"),
            ("command_handler", "let handler: CommandHandler"),
            ("command_legality", "CommandLegality.allowed"),
        ]
        for name, snippet in snippets:
            with self.subTest(name=name):
                root = self.make_repo()
                (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text(f"{snippet}\n")
                self.write_inventory(root)
                result = validate_inventory(root)
                self.assertFalse(result.ok)
                self.assertTrue(
                    any("forbidden P031 command_plumbing pattern" in error for error in result.errors),
                    result.errors,
                )

    def test_raw_truth_probing_patterns_fail(self) -> None:
        snippets = [
            ("contents_of_directory", "try contentsOfDirectory(atPath: path)"),
            ("string_contents", "try String(contentsOf: url)"),
            ("run_plan_file", "let plan: RunPlanFile"),
            ("raw_artifact", "let rawArtifactPath = path"),
        ]
        for name, snippet in snippets:
            with self.subTest(name=name):
                root = self.make_repo()
                (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text(f"{snippet}\n")
                self.write_inventory(root)
                result = validate_inventory(root)
                self.assertFalse(result.ok)
                self.assertTrue(
                    any("forbidden P031 raw_truth_probing pattern" in error for error in result.errors),
                    result.errors,
                )

    def test_bundled_resource_data_load_is_not_raw_truth_probing(self) -> None:
        root = self.make_repo()
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text(
            "let data = url.flatMap { try? Data(contentsOf: $0) }\n"
        )
        self.write_inventory(root)

        self.assertTrue(validate_inventory(root).ok)

    def test_generated_graphql_mutation_output_fails(self) -> None:
        root = self.make_repo()
        generated_path = "Chainworks Forge/Generated/GraphQL/Mutations.swift"
        (root / "Chainworks Forge/Generated/GraphQL").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / generated_path).write_text("struct StartRunMutation: GraphQLMutation {}\n")
        self.write_inventory(root, generated_graphql_outputs=[generated_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("forbidden P031 graphql_mutations pattern" in error for error in result.errors))

    def test_governed_graphql_mutation_document_fails(self) -> None:
        root = self.make_repo()
        document_path = "Chainworks Forge/GraphQL/StartRun.graphql"
        (root / "Chainworks Forge/GraphQL").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / document_path).write_text("mutation StartRun { startRun(input: {}) { run { id } } }\n")
        self.write_inventory(root, governed_graphql_documents=[document_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any(
                document_path in error and "forbidden P031 graphql_mutations pattern" in error
                for error in result.errors
            ),
            result.errors,
        )

    def test_uncovered_gql_document_fails_closed(self) -> None:
        root = self.make_repo()
        document_path = "Chainworks Forge/GraphQL/RunsHome.gql"
        (root / "Chainworks Forge/GraphQL").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / document_path).write_text("query RunsHome { runs { id } }\n")
        self.write_inventory(root)
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any(document_path in error for error in result.errors), result.errors)

    def test_governed_gql_mutation_document_fails(self) -> None:
        root = self.make_repo()
        document_path = "Chainworks Forge/GraphQL/StartRun.gql"
        (root / "Chainworks Forge/GraphQL").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / document_path).write_text("mutation StartRun { startRun(input: {}) { run { id } } }\n")
        self.write_inventory(root, governed_graphql_documents=[document_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any(
                document_path in error and "forbidden P031 graphql_mutations pattern" in error
                for error in result.errors
            ),
            result.errors,
        )

    def test_governed_graphql_document_with_removed_control_operation_name_fails(self) -> None:
        root = self.make_repo()
        document_path = "Chainworks Forge/GraphQL/StartRunReadback.graphql"
        (root / "Chainworks Forge/GraphQL").mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / document_path).write_text("query StartRunReadback { runs { id } }\n")
        self.write_inventory(root, governed_graphql_documents=[document_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any(
                document_path in error
                and "forbidden P031 removed_write_controls pattern" in error
                for error in result.errors
            ),
            result.errors,
        )

    def test_directory_inventory_entry_covers_nested_view(self) -> None:
        root = self.make_repo()
        nested_dir = root / "Chainworks Forge/Views/Thin"
        nested_dir.mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (nested_dir / "RunSummaryView.swift").write_text("struct RunSummaryView {}\n")
        self.write_inventory(
            root,
            governed_swift_files=[
                "Chainworks Forge/Views/RunsHomeView.swift",
                "Chainworks Forge/Views/Thin",
            ],
        )
        self.assertTrue(validate_inventory(root).ok)

    def test_generated_graphql_output_directory_is_scanned(self) -> None:
        root = self.make_repo()
        generated_dir = "Chainworks Forge/Generated/GraphQL"
        generated_path = f"{generated_dir}/Mutations.swift"
        (root / generated_dir).mkdir(parents=True)
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / generated_path).write_text("struct StartRunMutation: GraphQLMutation {}\n")
        self.write_inventory(root, generated_graphql_outputs=[generated_dir])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(
            any(generated_path in error and "forbidden P031 graphql_mutations pattern" in error for error in result.errors),
            result.errors,
        )

    def test_provider_toolchain_graphql_cache_is_not_repo_surface(self) -> None:
        root = self.make_repo()
        cache_path = (
            root
            / "chainworks/toolchains/providers/codex/session/rust/cargo/registry/src/"
            / "async-graphql/tests/services/minimal.graphql"
        )
        cache_path.parent.mkdir(parents=True)
        cache_path.write_text("query ThirdPartyFixture { __typename }\n")
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        self.write_inventory(root)

        self.assertTrue(validate_inventory(root).ok)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".", help="Repository root to validate")
    parser.add_argument("--self-test", action="store_true", help="Run script self-tests")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(P031ThinUIGateTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    return run_gate(Path(args.repo_root).resolve())


if __name__ == "__main__":
    raise SystemExit(main())
