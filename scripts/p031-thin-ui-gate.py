#!/usr/bin/env python3
"""Fail-closed static gate for P031 thin GraphQL-only UI coverage."""

from __future__ import annotations

import argparse
import json
import os
import re
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


INVENTORY_PATH = Path("docs/reference/p031-thin-ui-inventory.json")
WRITE_PATH_GUIDE_PATH = Path("docs/reference/p031-operator-write-path-guide.json")
REQUIRED_SCHEMA_VERSION = "p031-thin-ui-inventory-v1"
REQUIRED_WRITE_PATH_GUIDE_SCHEMA_VERSION = "p031-operator-write-path-guide-v1"
LEGACY_MODE_ENVIRONMENT = "CHAINWORKS_THIN_UI_MODE=legacy"
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
    "generated_graphql_outputs",
    "legacy_only_files",
    "explicit_exclusions",
    "forbidden_pattern_groups",
}

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


def validate_legacy_only_entries(entries: Any, errors: list[str]) -> None:
    if not isinstance(entries, list):
        return
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(
                f"legacy_only_files[{index}] must be an object tied to {LEGACY_MODE_ENVIRONMENT}"
            )
            continue
        if entry.get("legacy_mode_only") is not True:
            errors.append(f"legacy_only_files[{index}] must set legacy_mode_only to true")
        if entry.get("mode_env") != LEGACY_MODE_ENVIRONMENT:
            errors.append(
                f"legacy_only_files[{index}] must set mode_env to {LEGACY_MODE_ENVIRONMENT!r}"
            )


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
    legacy = collect_paths(inventory.get("legacy_only_files", []), "legacy_only_files", errors)
    exclusions = collect_paths(inventory.get("explicit_exclusions", []), "explicit_exclusions", errors)
    validate_legacy_only_entries(inventory.get("legacy_only_files", []), errors)
    validate_explicit_exclusion_entries(inventory.get("explicit_exclusions", []), errors)
    validate_category_overlap(
        {
            "governed_swift_files": governed,
            "governed_graphql_documents": graphql_docs,
            "generated_graphql_outputs": generated_graphql,
            "legacy_only_files": legacy,
            "explicit_exclusions": exclusions,
        },
        errors,
    )
    covered = governed | graphql_docs | generated_graphql | legacy | exclusions

    group_ids = collect_group_ids(inventory.get("forbidden_pattern_groups", []), errors)
    missing_groups = sorted(REQUIRED_FORBIDDEN_GROUPS - group_ids)
    if missing_groups:
        errors.append(f"inventory missing forbidden pattern groups: {', '.join(missing_groups)}")

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

    scan_forbidden_patterns(repo_root, governed | graphql_docs | generated_graphql, inventory, errors)
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


def validate_p031_contracts(repo_root: Path) -> GateResult:
    errors: list[str] = []
    errors.extend(validate_inventory(repo_root).errors)
    errors.extend(validate_write_path_guide(repo_root).errors)
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
            "generated_graphql_outputs": [],
            "legacy_only_files": [],
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

    def test_legacy_only_entries_must_be_tied_to_legacy_mode(self) -> None:
        root = self.make_repo()
        legacy_path = "Chainworks Forge/Views/LegacyRollbackView.swift"
        (root / "Chainworks Forge/Views/RunsHomeView.swift").write_text("struct RunsHomeView {}\n")
        (root / legacy_path).write_text("let service: ExecutionService\n")

        self.write_inventory(root, legacy_only_files=[legacy_path])
        result = validate_inventory(root)
        self.assertFalse(result.ok)
        self.assertTrue(any("legacy_only_files[0] must be an object" in error for error in result.errors))

        self.write_inventory(
            root,
            legacy_only_files=[
                {
                    "path": legacy_path,
                    "legacy_mode_only": True,
                    "mode_env": LEGACY_MODE_ENVIRONMENT,
                }
            ],
        )
        self.assertTrue(validate_inventory(root).ok)

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
