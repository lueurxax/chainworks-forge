import json
import sys
from pathlib import Path

artifact_path = Path(sys.argv[1])
expected_schema = sys.argv[2]
expected_revision = sys.argv[3]
gate_name = sys.argv[4]

def fail(message: str) -> None:
    raise SystemExit(f"{gate_name}: {message}")

if not artifact_path.is_file():
    fail(f"missing control artifact {artifact_path}")

try:
    payload = json.loads(artifact_path.read_text(encoding="utf-8"))
except json.JSONDecodeError as exc:
    fail(f"{artifact_path} is not valid JSON: {exc}")

if not isinstance(payload, dict):
    fail(f"{artifact_path} must contain one top-level JSON object")

schema = payload.get("schema")
if schema is None:
    schema = payload.get("schema_version")
if schema != expected_schema:
    fail(
        f"{artifact_path} schema mismatch: expected {expected_schema!r}, got {schema!r}"
    )

revision = payload.get("proposal_revision_id")
if revision != expected_revision:
    fail(
        f"{artifact_path} proposal_revision_id mismatch: expected {expected_revision!r}, got {revision!r}"
    )

def has_present_path(obj, path):
    current = obj
    for key in path:
        if not isinstance(current, dict) or key not in current:
            return False
        current = current[key]
    if isinstance(current, str):
        return bool(current.strip())
    if isinstance(current, (list, dict)):
        return bool(current)
    return current is not None

producer_evidence_paths = (
    ("producer_output_timestamp",),
    ("produced_at",),
    ("source_commit",),
    ("source_commit_sha",),
    ("source", "commit"),
    ("source", "commit_sha"),
)
if not any(has_present_path(payload, path) for path in producer_evidence_paths):
    fail(
        f"{artifact_path} must name its current producer output timestamp or source commit"
    )

def normalized_payload_text(obj):
    return (
        json.dumps(obj, sort_keys=True, separators=(",", ":"))
        .lower()
        .replace("-", "_")
        .replace(" ", "_")
        .replace("/", "_")
    )

def normalized_term(term):
    return term.lower().replace("-", "_").replace(" ", "_").replace("/", "_")

def require_term_groups(schema_name, groups):
    text = normalized_payload_text(payload)
    missing = []
    for label, terms in groups:
        normalized_terms = [normalized_term(term) for term in terms]
        if not any(term in text for term in normalized_terms):
            missing.append(label)
    if missing:
        fail(
            f"{artifact_path} is missing minimum {schema_name} coverage: "
            + ", ".join(missing)
        )

def require_all_terms(schema_name, clauses):
    text = normalized_payload_text(payload)
    missing = []
    for label, terms in clauses:
        normalized_terms = [normalized_term(term) for term in terms]
        if not all(term in text for term in normalized_terms):
            missing.append(label)
    if missing:
        fail(
            f"{artifact_path} is missing required {schema_name} detail: "
            + ", ".join(missing)
        )

def iter_mapping_records(obj):
    if isinstance(obj, dict):
        yield obj
        for value in obj.values():
            yield from iter_mapping_records(value)
    elif isinstance(obj, list):
        for value in obj:
            yield from iter_mapping_records(value)

def record_key_text(record):
    return normalized_payload_text(list(record.keys()))

def require_ticket_map_acceptance_coverage():
    acceptance_clauses = (
        ("AC1 proposal_review_router SystemTask", ("proposal_review_router", "systemtask")),
        ("AC2 successful/failed routing artifacts", ("agentselectionplanv1", "routingreceipt", "validationfailurejson")),
        ("AC3 compiled dynamic binding snapshot", ("compileddynamicagentbinding", "resolvedagent", "rollout_wave")),
        ("AC4 dynamic_parallel materialization idempotency", ("dynamic_parallel", "idempotent")),
        ("AC5 selected_outputs_from aggregation", ("selected_outputs_from", "stale", "unselected")),
        ("AC6 legacy fixed quartet", ("legacy_fixed", "fixed_quartet")),
        ("AC7 Phase 3 core specialist golden-output validation", ("phase_3_core", "golden_output")),
        ("AC8 disabled later-wave materialization guard", ("disabled_later_wave", "materialized")),
        ("AC9 operator routing UI", ("operator_ui", "routing_progress", "under_specified", "evidence_redaction")),
        ("AC10 evidence projection surfaces", ("graphql", "mcp", "swift_debug", "evidence_projection")),
        ("AC11 Phase 0b control artifacts", ("phase_0b", "control_artifacts", "proposal_060")),
        ("AC12 implementation_ticket_map fields", ("implementation_ticket_map", "reviewer_lens", "cross_phase_slip")),
        ("AC13 shadow cutover gates", ("shadow_cutover", "quality", "budget", "rollback")),
    )

    records = list(iter_mapping_records(payload))
    missing_acceptance_mappings = []
    mapping_field_groups = (
        ("owner", ("owner",)),
        ("phase", ("phase",)),
        ("files/modules", ("files", "modules")),
        ("test gate", ("test_gate", "test gate")),
        ("reviewer lens", ("reviewer_lens", "reviewer lens")),
        ("high-parity-risk flag", ("high_parity_risk", "high-parity-risk")),
        ("joint-review lens", ("joint_review", "swift_rust", "swift/rust")),
        ("cross-phase slip status", ("cross_phase_slip", "cross-phase slip")),
    )
    mapping_field_key_terms = {
        normalized_term(term)
        for _, terms in mapping_field_groups
        for term in terms
    }

    mapping_records = [
        record
        for record in records
        if any(term in record_key_text(record) for term in mapping_field_key_terms)
    ]

    for label, terms in acceptance_clauses:
        normalized_terms = [normalized_term(term) for term in terms]
        matching_records = [
            record
            for record in mapping_records
            if all(term in normalized_payload_text(record) for term in normalized_terms)
        ]
        if not matching_records:
            missing_acceptance_mappings.append(label)
            continue

        unmapped_fields = []
        for field_label, field_terms in mapping_field_groups:
            if not any(
                any(
                    normalized_term(field_term) in normalized_payload_text(record)
                    for field_term in field_terms
                )
                for record in matching_records
            ):
                unmapped_fields.append(field_label)
        if unmapped_fields:
            missing_acceptance_mappings.append(
                f"{label} mapping fields ({', '.join(unmapped_fields)})"
            )

    if missing_acceptance_mappings:
        fail(
            f"{artifact_path} is missing required implementation_ticket_map_v1 detail: "
            + ", ".join(missing_acceptance_mappings)
        )

    parity_deliverables = (
        "routing_contract_fixtures_v1",
        "frozen_snapshot_helper_inventory_v1",
    )
    missing = []
    for deliverable in parity_deliverables:
        matching_records = [
            record
            for record in records
            if normalized_term(deliverable) in normalized_payload_text(record)
        ]
        if not matching_records:
            missing.append(f"{deliverable} ticket")
            continue
        if not any(
            "high_parity_risk" in normalized_payload_text(record)
            and "owner" in normalized_payload_text(record)
            and (
                "joint_review" in normalized_payload_text(record)
                or "swift_rust" in normalized_payload_text(record)
            )
            for record in matching_records
        ):
            missing.append(f"{deliverable} high-parity joint-review ownership")
    if missing:
        fail(
            f"{artifact_path} is missing implementation_ticket_map_v1 parity-risk controls: "
            + ", ".join(missing)
        )

def iter_corpus_collections(obj, path=()):
    if isinstance(obj, dict):
        for key, value in obj.items():
            current_path = path + (str(key),)
            key_text = "_".join(normalized_term(part) for part in current_path)
            if "corpus" in key_text:
                if isinstance(value, list):
                    yield value
                elif isinstance(value, dict):
                    yield list(value.values())
            yield from iter_corpus_collections(value, current_path)
    elif isinstance(obj, list):
        for index, value in enumerate(obj):
            yield from iter_corpus_collections(value, path + (str(index),))

def require_baseline_shadow_corpus():
    collections = [
        collection
        for collection in iter_corpus_collections(payload)
        if collection
    ]
    if not collections:
        fail(f"{artifact_path} is missing proposal_review_baseline_v1 shadow corpus membership")

    largest_collection = max(collections, key=len)
    corpus_count = len(largest_collection)
    if corpus_count < 5 or corpus_count > 8:
        fail(
            f"{artifact_path} proposal_review_baseline_v1 shadow corpus must contain 5-8 proposals; "
            f"found {corpus_count}"
        )

    corpus_text = normalized_payload_text(largest_collection)

    def has_clause(clauses):
        return any(
            all(normalized_term(term) in corpus_text for term in clause)
            for clause in clauses
        )

    coverage_groups = (
        ("UI-only", (("ui_only",), ("ui", "macos"), ("ui", "swiftui"))),
        ("Rust backend", (("rust_backend",), ("rust", "backend"), ("rust", "control_plane"))),
        ("security/API", (("security_api",), ("security", "api"), ("graphql", "security"))),
        ("reliability/retry", (("reliability_retry",), ("reliability", "retry"), ("resume", "retry"))),
        ("rollout", (("rollout",), ("cutover",), ("release",))),
        ("mixed-stack", (("mixed_stack",), ("mixed", "stack"), ("swift", "rust"), ("macos", "rust"))),
    )
    missing = [label for label, clauses in coverage_groups if not has_clause(clauses)]
    if missing:
        fail(
            f"{artifact_path} is missing proposal_review_baseline_v1 shadow corpus coverage: "
            + ", ".join(missing)
        )

def require_baseline_dependency_confirmations():
    text = normalized_payload_text(payload)
    missing = []
    if not all(normalized_term(term) in text for term in ("p017", "informative_only")):
        missing.append("P017 informative-only confirmation")
    if normalized_term("p047") not in text or not any(
        normalized_term(term) in text for term in ("extension", "fallback")
    ):
        missing.append("P047 extension or local compiler fallback")
    if normalized_term("token_measurement_mode") not in text:
        missing.append("token measurement mode decision")
    if missing:
        fail(
            f"{artifact_path} is missing proposal_review_baseline_v1 dependency confirmations: "
            + ", ".join(missing)
        )

def require_storage_compatibility_matrix_entity_coverage():
    entity_terms = (
        ("SystemExecution", "systemexecution"),
        ("RoutingReceipt", "routingreceipt"),
        ("AgentSelectionPlanV1", "agentselectionplanv1"),
        ("DynamicMaterializationRecord", "dynamicmaterializationrecord"),
        ("ReviewCorpusBundleV2", "reviewcorpusbundlev2"),
    )
    required_behaviors = (
        ("old-run readback", ("old_run_readback", "old run readback")),
        ("rollback readback", ("rollback_readback", "rollback readback")),
        ("GraphQL exposure", ("graphql",)),
        ("MCP exposure", ("mcp",)),
        ("report precedence", ("report_precedence", "report")),
        ("recovery precedence", ("recovery_precedence", "recovery")),
        ("migration behavior", ("migration_behavior", "migration")),
    )

    records = list(iter_mapping_records(payload))
    row_key_terms = ("entity", "artifact", "contract", "record", "row", "name")
    missing = []
    for entity_label, entity_term in entity_terms:
        matching_records = [
            record
            for record in records
            if entity_term in normalized_payload_text(record)
            and any(term in normalized_payload_text(list(record.keys())) for term in row_key_terms)
        ]
        if not matching_records:
            missing.append(f"{entity_label} matrix row")
            continue

        record_text = normalized_payload_text(matching_records)
        missing_behaviors = [
            behavior_label
            for behavior_label, terms in required_behaviors
            if not any(normalized_term(term) in record_text for term in terms)
        ]
        if missing_behaviors:
            missing.append(f"{entity_label} ({', '.join(missing_behaviors)})")

    if missing:
        fail(
            f"{artifact_path} is missing storage_compatibility_matrix_v1 per-entity coverage: "
            + ", ".join(missing)
        )

def require_routing_fixture_dynamic_phase_contract():
    text = normalized_payload_text(payload)
    required_clauses = (
        ("system.routing YAML fixture", ("system.routing", "yaml")),
        ("system_task proposal_review_router fixture", ("system_task", "proposal_review_router")),
        ("dynamic_parallel YAML fixture", ("dynamic_parallel", "yaml")),
        ("compiler-owned candidate bindings", ("candidate_bindings",)),
        ("selected_outputs_from selector fixture", ("selected_outputs_from",)),
        ("Swift Codable representation", ("swift", "codable")),
        ("Rust serde representation", ("rust", "serde")),
        ("compiled-plan snapshot", ("compiled_plan", "snapshot")),
        ("invalid-case parity matrix", ("invalid_case", "parity_matrix")),
        ("attempt-scoped selected_outputs_from run identity", ("selected_outputs_from", "run_id", "attempt")),
        ("attempt-scoped selected_outputs_from plan binding", ("selected_outputs_from", "plan_hash", "binding_id")),
        ("attempt-scoped selected_outputs_from execution identity", ("selected_outputs_from", "agent_execution_id", "proposal_review_v1")),
    )
    missing = [
        label
        for label, terms in required_clauses
        if not all(normalized_term(term) in text for term in terms)
    ]
    if missing:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 dynamic phase contract: "
            + ", ".join(missing)
        )

def require_routing_fixture_reviewer_count_coverage():
    records = list(iter_mapping_records(payload))
    count_records = [
        record
        for record in records
        if any(term in record_key_text(record) for term in ("reviewer_count", "selected_count", "selected_reviewers", "selected_agents"))
    ]
    if not count_records:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 selected reviewer count coverage"
        )

    missing = []
    for count in (2, 3, 4, 5):
        count_text = str(count)
        matching_records = [
            record
            for record in count_records
            if count_text in normalized_payload_text(record)
        ]
        if not matching_records:
            missing.append(f"{count} selected reviewers")
            continue
        record_text = normalized_payload_text(matching_records)
        if not all(term in record_text for term in ("dynamic_parallel", "selected_outputs_from")):
            missing.append(f"{count} reviewer dynamic_parallel/selected_outputs_from coverage")
    if missing:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 selected reviewer count coverage: "
            + ", ".join(missing)
        )

def require_routing_fixture_validation_guard_cases():
    text = normalized_payload_text(payload)
    required_clauses = (
        ("force_include validation", ("force_include",)),
        ("force_exclude validation", ("force_exclude",)),
        ("mandatory conflict validation", ("mandatory_conflict",)),
        ("disabled rollout wave validation", ("disabled_rollout_wave",)),
        ("unknown agent validation", ("unknown_agent",)),
        ("placeholder resolved agent validation", ("placeholder_resolved_agent",)),
    )
    missing = [
        label
        for label, terms in required_clauses
        if not all(normalized_term(term) in text for term in terms)
    ]
    if missing:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 validation guard cases: "
            + ", ".join(missing)
        )

def require_routing_fixture_deterministic_parity():
    records = list(iter_mapping_records(payload))
    parity_records = [
        record
        for record in records
        if "same_input" in normalized_payload_text(record)
        or "deterministic_parity" in record_key_text(record)
    ]
    if not parity_records:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 deterministic Swift/Rust parity cases: same fixture input"
        )

    required_terms = (
        ("same fixture input", ("same_input", "same fixture input")),
        ("Swift implementation", ("swift",)),
        ("Rust implementation", ("rust",)),
        ("selected order", ("selected_order", "selected order")),
        ("evidence IDs", ("evidence_ids", "evidence id")),
        ("plan_hash", ("plan_hash",)),
    )
    missing = []
    parity_text = normalized_payload_text(parity_records)
    for label, terms in required_terms:
        if not any(normalized_term(term) in parity_text for term in terms):
            missing.append(label)
    if missing:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 deterministic Swift/Rust parity cases: "
            + ", ".join(missing)
        )

def iter_routing_metadata_field_collections(obj, path=()):
    if isinstance(obj, dict):
        for key, value in obj.items():
            current_path = path + (str(key),)
            key_text = "_".join(normalized_term(part) for part in current_path)
            if "routing_metadata" in key_text and (
                "required_fields" in key_text
                or key_text.endswith("fields")
            ):
                yield value
            yield from iter_routing_metadata_field_collections(value, current_path)
    elif isinstance(obj, list):
        for index, value in enumerate(obj):
            yield from iter_routing_metadata_field_collections(value, path + (str(index),))

def require_routing_fixture_metadata_required_fields():
    required_fields = (
        "routing_id",
        "family",
        "capabilities",
        "stacks",
        "surfaces",
        "risks",
        "enabled_for_proposal_review",
        "rollout_wave",
    )
    collections = list(iter_routing_metadata_field_collections(payload))
    if not collections:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 routing metadata required fields: "
            + ", ".join(required_fields)
        )

    text = normalized_payload_text(collections)
    missing = [
        field
        for field in required_fields
        if normalized_term(field) not in text
    ]
    if missing:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 routing metadata required fields: "
            + ", ".join(missing)
        )

def require_routing_fixture_catalog_launch_scope():
    text = normalized_payload_text(payload)
    existing_reviewers = (
        "proposal_reviewer_product_owner",
        "proposal_reviewer_ux",
        "proposal_reviewer_ui",
        "proposal_reviewer_architect",
    )
    phase_3_core_reviewers = (
        "proposal_reviewer_macos",
        "proposal_reviewer_apple_architect",
        "proposal_reviewer_rust_architect",
        "proposal_reviewer_reliability",
        "proposal_reviewer_security",
        "proposal_reviewer_api_contract",
        "proposal_reviewer_observability_rollout",
    )
    later_wave_reviewers = (
        "proposal_reviewer_ios",
        "proposal_reviewer_web",
        "proposal_reviewer_design",
        "proposal_reviewer_golang",
        "proposal_reviewer_dba",
        "proposal_reviewer_performance",
    )

    missing = []
    for reviewer in existing_reviewers:
        if normalized_term(reviewer) not in text:
            missing.append(f"existing reviewer {reviewer}")
    for reviewer in phase_3_core_reviewers:
        if normalized_term(reviewer) not in text:
            missing.append(f"phase_3_core reviewer {reviewer}")
    for reviewer in later_wave_reviewers:
        if normalized_term(reviewer) not in text:
            missing.append(f"later-wave reviewer {reviewer}")

    required_clauses = (
        ("enabled phase_3_core executable scope", ("phase_3_core", "enabled_for_proposal_review", "true")),
        ("disabled later-wave executable guard", ("later_wave", "enabled_for_proposal_review", "false")),
        ("disabled rollout-wave materialization rejection", ("disabled_by_rollout_wave", "cannot_materialize")),
    )
    for label, terms in required_clauses:
        if not all(normalized_term(term) in text for term in terms):
            missing.append(label)

    if missing:
        fail(
            f"{artifact_path} is missing routing_contract_fixtures_v1 catalog launch scope: "
            + ", ".join(missing)
        )

def require_frozen_snapshot_inventory_reader_mappings():
    text = normalized_payload_text(payload)
    records = [
        record
        for record in iter_mapping_records(payload)
        if "reader" in record_key_text(record)
    ]
    if not records:
        fail(
            f"{artifact_path} is missing frozen_snapshot_helper_inventory_v1 reader mappings"
        )

    if not any(term in text for term in ("repo_search", "search_pattern", "rg", "grep", "file_match")):
        fail(
            f"{artifact_path} is missing frozen_snapshot_helper_inventory_v1 repo search evidence"
        )

    allowed_status_terms = ("snapshot_backed", "legacy_fallback", "removal_ticket")
    unmapped = []
    for index, record in enumerate(records, start=1):
        record_text = normalized_payload_text(record)
        if "post_start" not in record_text and "post-start" not in record_text:
            unmapped.append(f"reader row {index} missing post-start scope")
        if "workflow" not in record_text and "catalog" not in record_text:
            unmapped.append(f"reader row {index} missing workflow/catalog surface")
        if not any(term in record_text for term in allowed_status_terms):
            unmapped.append(f"reader row {index} missing snapshot-backed, legacy fallback, or removal ticket mapping")
    if unmapped:
        fail(
            f"{artifact_path} is missing frozen_snapshot_helper_inventory_v1 row detail: "
            + ", ".join(unmapped)
        )

def require_fixed_quartet_inventory_surface_coverage():
    required_surfaces = (
        ("Swift", ("swift",)),
        ("Rust", ("rust",)),
        ("fixtures", ("fixtures", "fixture")),
        ("reports", ("reports", "report")),
        ("feedback fidelity", ("feedback_fidelity", "feedback")),
        ("rereview scope", ("rereview_scope", "rereview")),
        ("UI views", ("ui_views", "ui_view", "ui")),
    )
    records = [
        record
        for record in iter_mapping_records(payload)
        if any(
            term in normalized_payload_text(record)
            for term in ("product_owner", "ux", "architect", "fixed_quartet")
        )
        and any(
            term in record_key_text(record)
            for term in ("surface", "module", "path", "assumption", "scope")
        )
    ]
    missing = []
    for label, terms in required_surfaces:
        matching_records = [
            record
            for record in records
            if any(normalized_term(term) in normalized_payload_text(record) for term in terms)
        ]
        if not matching_records:
            missing.append(f"{label} surface")
            continue
        if not any(
            any(term in normalized_payload_text(record) for term in ("migration", "legacy_behavior", "intentional_legacy"))
            for record in matching_records
        ):
            missing.append(f"{label} migration or legacy behavior")
    if missing:
        fail(
            f"{artifact_path} is missing hardcoded_fixed_quartet_inventory_v1 surface coverage: "
            + ", ".join(missing)
        )

def require_routing_calibration_expected_selection_rows():
    records = [
        record
        for record in iter_mapping_records(payload)
        if any(
            normalized_term(str(key)) in ("scenario", "case")
            for key in record.keys()
        )
    ]
    missing_row_expectations = [
        f"scenario row {index} expected selection"
        for index, record in enumerate(records, start=1)
        if not any(
            term in record_key_text(record)
            for term in ("expected_selection", "selected_reviewers", "selected_agents", "expected_outcome")
        )
    ]
    required_scenarios = (
        ("UI/macOS", (("ui", "macos"), ("ui_only",))),
        ("Rust retry/resume", (("rust", "retry", "resume"),)),
        ("security/API", (("security", "api"),)),
        ("reliability", (("reliability",),)),
        ("rollout", (("rollout",),)),
        ("under-specified", (("under_specified",), ("under-specified",))),
        ("override", (("override", "force_include", "force_exclude"),)),
        ("mandatory-overflow", (("mandatory_overflow",), ("mandatory-overflow",))),
    )

    def record_matches(record, clauses):
        record_text = normalized_payload_text(record)
        return any(
            all(normalized_term(term) in record_text for term in clause)
            for clause in clauses
        )

    missing = missing_row_expectations
    for label, clauses in required_scenarios:
        matching_records = [
            record for record in records if record_matches(record, clauses)
        ]
        if not matching_records:
            missing.append(f"{label} scenario")
            continue
        if not any(
            any(
                term in record_key_text(record)
                for term in ("expected_selection", "selected_reviewers", "selected_agents", "expected_outcome")
            )
            for record in matching_records
        ):
            missing.append(f"{label} expected selection")

    under_specified_records = [
        record
        for record in records
        if record_matches(record, (("under_specified",), ("under-specified",)))
    ]
    if under_specified_records and not any(
        all(term in normalized_payload_text(record) for term in ("product_owner", "architect"))
        and any(term in normalized_payload_text(record) for term in ("warning", "caution"))
        for record in under_specified_records
    ):
        missing.append("under-specified fallback product_owner plus architect caution")

    overflow_records = [
        record
        for record in records
        if record_matches(record, (("mandatory_overflow",), ("mandatory-overflow",)))
    ]
    if overflow_records and not any(
        any(term in normalized_payload_text(record) for term in ("fail_closed", "routing_conflict", "no_agentselectionplanv1"))
        for record in overflow_records
    ):
        missing.append("mandatory-overflow fail-closed outcome")

    if missing:
        fail(
            f"{artifact_path} is missing routing_calibration_report_v1 expected selection rows: "
            + ", ".join(missing)
        )

def require_ticket_map_review_routing_ingress_path():
    text = normalized_payload_text(payload)
    required_clauses = (
        ("RunStartOptionsV2.review_routing block", ("runstartoptionsv2", "review_routing")),
        ("Swift launch/clone ingress", ("swift", "launch", "clone")),
        ("Rust StartRunCmd ingress", ("startruncmd",)),
        ("GraphQL startRun ingress", ("graphql", "startrun")),
        ("MCP runs.start ingress", ("mcp", "runs.start")),
        ("command journal redaction", ("command_journal", "redaction")),
        ("persisted run snapshot truth", ("persisted", "snapshot")),
        ("RoutingReceipt operator actions", ("routingreceipt", "operator_actions")),
        ("AgentSelectionPlanV1 input snapshot hashes", ("agentselectionplanv1", "input_snapshot_hashes")),
    )
    missing = [
        label
        for label, terms in required_clauses
        if not all(normalized_term(term) in text for term in terms)
    ]
    if missing:
        fail(
            f"{artifact_path} is missing implementation_ticket_map_v1 review_routing ingress path: "
            + ", ".join(missing)
        )

contract_term_groups = {
    "proposal_review_baseline_v1": (
        ("fixed quartet reviewer count", ("fixed_quartet", "reviewer_count")),
        ("token measurement mode", ("token_measurement_mode", "actual_provider_tokens", "proxy")),
        ("corpus membership", ("corpus", "corpus_membership")),
        ("reviewer outputs", ("reviewer_outputs", "reviewer output")),
        ("quality baseline", ("quality_baseline", "baseline_quality")),
    ),
    "storage_compatibility_matrix_v1": (
        ("old-run readback", ("old_run_readback", "old run readback")),
        ("rollback readback", ("rollback_readback", "rollback readback")),
        ("GraphQL exposure", ("graphql",)),
        ("MCP exposure", ("mcp",)),
        ("report precedence", ("report_precedence", "report")),
        ("recovery precedence", ("recovery_precedence", "recovery")),
        ("migration behavior", ("migration", "migration_behavior")),
        ("SystemExecution", ("systemexecution", "system_execution")),
        ("RoutingReceipt", ("routingreceipt", "routing_receipt")),
        ("AgentSelectionPlanV1", ("agentselectionplanv1", "agent_selection_plan_v1")),
        ("DynamicMaterializationRecord", ("dynamicmaterializationrecord", "dynamic_materialization_record")),
        ("ReviewCorpusBundleV2", ("reviewcorpusbundlev2", "review_corpus_bundle_v2")),
    ),
    "routing_contract_fixtures_v1": (
        ("Swift parity", ("swift",)),
        ("Rust parity", ("rust",)),
        ("canonicalization", ("canonicalization", "canonical")),
        ("evidence IDs", ("evidence_ids", "evidence id")),
        ("scores", ("scores", "score")),
        ("selected order", ("selected_order", "selected order")),
        ("plan_hash", ("plan_hash",)),
        ("warnings", ("warnings", "warning")),
        ("validation failures", ("validation_failures", "validation failures", "validation_failure")),
        ("override conflicts", ("override_conflict", "force_include", "force_exclude")),
        ("under_specified fallback", ("under_specified",)),
        ("mandatory overflow", ("mandatory_overflow", "mandatory overflow")),
        ("system.routing DSL", ("system.routing",)),
        ("dynamic_parallel DSL", ("dynamic_parallel",)),
        ("selected_outputs_from selector", ("selected_outputs_from",)),
        ("compiled-plan snapshots", ("compiled_plan", "compiled plan")),
        ("invalid-case parity matrix", ("invalid_case", "invalid case")),
    ),
    "frozen_snapshot_helper_inventory_v1": (
        ("post-start readers", ("post_start", "post-start")),
        ("workflow snapshot", ("workflow_snapshot", "workflow snapshot")),
        ("catalog snapshot", ("catalog_snapshot", "catalog snapshot")),
        ("snapshot-backed helper", ("snapshot_backed", "snapshot-backed")),
        ("legacy fallback", ("legacy_fallback", "legacy fallback")),
    ),
    "hardcoded_fixed_quartet_inventory_v1": (
        ("product_owner", ("product_owner",)),
        ("ux", ("ux",)),
        ("ui", ("ui",)),
        ("architect", ("architect",)),
        ("migration or legacy behavior", ("migration", "legacy_behavior", "intentional_legacy")),
    ),
    "implementation_ticket_map_v1": (
        ("acceptance criteria", ("acceptance_criteria", "acceptance criterion")),
        ("owner", ("owner",)),
        ("phase", ("phase",)),
        ("files/modules", ("files", "modules")),
        ("test gate", ("test_gate", "test gate")),
        ("reviewer lens", ("reviewer_lens", "reviewer lens")),
        ("high-parity-risk", ("high_parity_risk", "high-parity-risk")),
        ("joint Swift/Rust review", ("joint_review", "swift_rust", "swift/rust")),
        ("cross-phase slip status", ("cross_phase_slip", "cross-phase slip")),
    ),
    "routing_calibration_report_v1": (
        ("UI example", ("ui", "macos")),
        ("Rust retry/resume example", ("rust", "retry", "resume")),
        ("security/API example", ("security", "api")),
        ("reliability example", ("reliability",)),
        ("rollout example", ("rollout",)),
        ("under-specified example", ("under_specified", "under-specified")),
        ("override example", ("override", "force_include", "force_exclude")),
        ("mandatory-overflow example", ("mandatory_overflow", "mandatory-overflow")),
    ),
}

if expected_schema not in contract_term_groups:
    fail(f"no minimum coverage validator registered for schema {expected_schema!r}")
require_term_groups(expected_schema, contract_term_groups[expected_schema])
if expected_schema == "proposal_review_baseline_v1":
    require_baseline_shadow_corpus()
    require_baseline_dependency_confirmations()
if expected_schema == "storage_compatibility_matrix_v1":
    require_storage_compatibility_matrix_entity_coverage()
if expected_schema == "routing_contract_fixtures_v1":
    require_routing_fixture_dynamic_phase_contract()
    require_routing_fixture_reviewer_count_coverage()
    require_routing_fixture_validation_guard_cases()
    require_routing_fixture_deterministic_parity()
    require_routing_fixture_metadata_required_fields()
    require_routing_fixture_catalog_launch_scope()
if expected_schema == "frozen_snapshot_helper_inventory_v1":
    require_frozen_snapshot_inventory_reader_mappings()
if expected_schema == "hardcoded_fixed_quartet_inventory_v1":
    require_fixed_quartet_inventory_surface_coverage()
if expected_schema == "implementation_ticket_map_v1":
    require_ticket_map_acceptance_coverage()
    require_ticket_map_review_routing_ingress_path()
if expected_schema == "routing_calibration_report_v1":
    require_routing_calibration_expected_selection_rows()
