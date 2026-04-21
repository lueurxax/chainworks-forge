{
  "proposal_revision_id": "p053-r7-2026-04-20",
  "source_review_pass_id": "p053-review-pass-1",
  "title": "Proposal 053: Bounded ACP Artifact Discovery and Startup Latency",
  "date": "2026-04-20",
  "status": "Revised for implementation readiness after R6 score-lift review",
  "run_id": "4b3a582a-b39f-4e97-9454-d142301a6f1f",
  "source_idea": "/Users/user/Documents/Chainworks Forge/.chainworks/runs/4b3a582a-b39f-4e97-9454-d142301a6f1f/context/idea.md",
  "primary_surface": "Rust control-plane ACP execution, engine artifact settlement, workflow compilation, runtime facts, reports, MCP/GraphQL readback, and macOS operator diagnostics.",
  "hard_dependencies": [
    "P037 ACP execution supervision and idle watchdog",
    "P050 per-run workspace isolation",
    "P057 canonical artifact contracts",
    "P058 ACP runtime facts and output settlement"
  ],
  "related_but_not_blocking": [
    {
      "id": "P051",
      "title": "Shared Xcode MCP bridge pool",
      "relationship": "P053 must not regress MCP startup diagnostics and must keep P051 bridge behavior unchanged, but the proposal-053|p053 gate is independent of the P051 gate."
    }
  ],
  "canonical_gate": "proposal-053|p053",
  "executive_summary": "Fresh ACP startup currently performs recursive filesystem discovery before the provider receives ACP initialize. A dogfood run on an approximately 8.9 GB workspace with 126,643 files showed more than one minute of local pre-handshake latency that was easy to misattribute to provider slowness. P053 removes all broad repository and worktree scanning before initialize, then replaces broad artifact inference with an engine-owned discovery settlement pipeline: typed expected-output specs, deterministic digest-backed pre-prompt metadata, control-plane generated changed-file manifests, exact-path acceptance decisions, bounded current-run meta-root discovery, versioned durable diagnostics, and explicit compatibility fallback only when a frozen run policy or audited one-shot override permits it. R7 closes the R6 score-lift review by making byte caps provisional until a Phase 0 production-size sample validates or tunes them, tiering acceptance criteria into independently shippable phases, field-completing OutputDiscoveryDecision and the CapturedOutput validation adapter, making legacy override identity race-free with retry scheduling, adding crash-consistency invariants, naming legacy fallback caps, and tightening UI overflow behavior. Required-output truth remains in the engine and P057/P058 contract layer; raw file existence after a rejection cannot satisfy a required output.",
  "problem": {
    "statement": "ACP startup performs a full recursive workspace snapshot before sending initialize, so local filesystem work can dominate session startup while sitting outside the provider handshake timeout.",
    "observed_baseline": "A live dogfood run measured more than one minute of local pre-handshake latency on an approximately 8.9 GB workspace with 126,643 files.",
    "failures": [
      "Provider startup appears slow even when the provider has not received initialize.",
      "Handshake timeout and retry classification do not cover the expensive local pre-handshake work.",
      "Repeated fresh sessions multiply the cost.",
      "Current post-prompt filesystem diff can infer outputs from dirty worktrees rather than declared contracts.",
      "Path-only expected_output_paths cannot carry labels, requiredness, reuse policy, size policy, or durable rejection reasons.",
      "Current engine validation can read declared target paths from disk after discovery, which would bypass stale, oversized, escaped, or unauthorized exact-path rejection unless P053 changes the handoff."
    ]
  },
  "goals": [
    "Send ACP initialize promptly for fresh sessions regardless of repository size.",
    "Remove recursive repository, worktree, and workspace-root scans from the pre-initialize path.",
    "Use typed expected-output specs and exact expected paths as the primary discovery mechanism for declared outputs.",
    "Prevent stale, rejected, escaped, unauthorized, or oversized files from being accepted later by disk-based validation.",
    "Bound opportunistic discovery to the current run chainworks_meta_root only.",
    "Generate repo-backed changed-file manifests after prompt completion and before exact-path acceptance.",
    "Keep required/optional output settlement in the engine and P057/P058 artifact-contract layer.",
    "Persist discovery diagnostics for operator UI, reports, GraphQL, MCP readback, and tests.",
    "Make legacy broad discovery explicit, audited, capped, post-prompt only, and off by default.",
    "Validate candidate output byte caps against representative production runs before implementation fixes the defaults.",
    "Ship in phased slices so the core latency fix can be proven independently from compatibility and UI follow-ons.",
    "Provide deterministic tests and a proposal-053|p053 gate that prove behavior rather than host-speed timing."
  ],
  "non_goals": [
    "Do not switch ACP transport from stdio to HTTP.",
    "Do not change P051 shared Xcode MCP bridge pooling.",
    "Do not make .chainworks tracked by git.",
    "Do not migrate historical artifact bytes or rewrite completed run artifacts.",
    "Do not run broad legacy discovery by default for post-P053 runs.",
    "Do not use local UI smoke tests as the primary proof path for this Rust control-plane change.",
    "Do not create a second output-validation system parallel to P057/P058.",
    "Do not let ACP transport decide required-output truth.",
    "Do not add contract-specific artifact size maxima in P053; Phase 0 may tune global defaults or add a workflow-level cap override if production sampling shows the candidate caps are too low, while contract-tunable maxima remain a named follow-up."
  ],
  "resolved_design_decisions": [
    {
      "id": "D-01",
      "title": "Engine-owned discovery settlement",
      "resolves": [
        "ARCH-R2-01",
        "ARCH-R2-03",
        "SUG-ARCH-R2-01",
        "SUG-ARCH-R2-02"
      ],
      "decision": "Final declared-output acceptance moves to the engine. ACP transport removes startup snapshotting and returns prompt transcript, provider output envelopes, provider timing, and pre-prompt metadata captured at the session boundary, but it does not make final required-output settlement decisions. After prompt completion, the engine generates declared control-plane artifacts, then builds OutputDiscoveryDecision records from accepted provider envelopes, accepted exact-path reads, control-plane generated outputs, and bounded meta-root evidence. validate_task_outputs and declared artifact persistence consume accepted decisions, not arbitrary target_path reads from disk.",
      "tradeoff": "The discovery pipeline becomes a little more explicit in the engine, but this is the only reliable way to ensure rejected files cannot be accepted later by raw disk presence."
    },
    {
      "id": "D-02",
      "title": "Typed expected-output specs",
      "resolves": [
        "ARCH-R2-02",
        "UX-AS-02",
        "UI-ASM-01"
      ],
      "decision": "Add ExpectedOutputSpec as the engine-to-runtime discovery contract while retaining expected_output_paths: Vec<String> as a compatibility projection. Each spec includes output_name, output_role, target_path, companion_path when applicable, display_label, contract_id, requiredness, reuse_policy, default max_bytes, authorized_roots, and source_generation_owner. UI and reports read labels and reasons from these specs and the resulting decisions.",
      "tradeoff": "This extends the request/result contract, but avoids duplicating engine policy in transport and avoids guessing artifact labels from paths."
    },
    {
      "id": "D-03",
      "title": "Exact-path freshness and reuse policy",
      "resolves": [
        "PO-B-01",
        "ARCH-01",
        "ARCH-R3-01",
        "ARCH-R4-01",
        "SLB-01"
      ],
      "decision": "The default per-output reuse_policy is must_produce. Existing list-based workflow YAML remains valid: AgentTask.outputs stays a list of artifact names. P053 adds an optional sibling per-task map output_policies keyed by output name. Example: outputs: [proposal_current]; output_policies: { proposal_current: { reuse_policy: allow_unchanged_existing } }. Unknown output_policies keys fail compilation because they do not match an item in outputs. Unknown reuse_policy values fail compilation with a clear validation error. Rust adds AgentTask.output_policies: Option<HashMap<String, OutputPolicyDef>> and CompiledTask.output_policies with serde defaults; Swift mirror structs add the same optional map; existing list-only workflows deserialize, compile, snapshot-hash, and run unchanged. For must_produce, an exact-path file satisfies the invocation only when it was absent before prompt and exists after prompt, its post-prompt content_digest differs from the pre-prompt content_digest for files within the exact-read cap, it was accepted from a trusted provider output envelope, or it was generated by a control-plane step in the current invocation. PrePromptExpectedOutputMetadata stores output_name, resolved_path, canonical_path when resolvable, root_class, existed, file_type, size_bytes, content_digest for regular files at or below the exact-read cap, mtime_ns as diagnostic-only, and baseline_status. mtime and size alone never prove freshness. Unreadable, oversized, non-regular, escaped, wrong-root, or uncertain baseline states become typed warning/rejection states rather than silent acceptance. The compiler freezes reuse_policy into the RunPlanSnapshot and ExpectedOutputSpec. allow_unchanged_existing accepts an unchanged pre-existing file only after normal root, symlink, regular-file, and size checks and records provenance as declared_reuse_policy in runtime facts/source-generation metadata.",
      "tradeoff": "The reuse syntax adds small schema surface, but it makes intentional reuse auditable and makes the acceptance criterion testable."
    },
    {
      "id": "D-04",
      "title": "Changed-files manifest sequencing and ownership",
      "resolves": [
        "ARCH-R2-03",
        "ARCH-02",
        "PO-05",
        "SLB-02",
        "SLB-12"
      ],
      "decision": "The engine runs generate_changed_files_manifest_if_declared after ACP prompt completion and before exact-path acceptance or contract validation. If the workflow declares changed_files_manifest and the invocation has a repo-backed worktree, the engine writes the canonical manifest path as a control-plane generated artifact. If an agent already wrote that path, preserve it at changed_files_manifest.agent.json and write the control-plane manifest at the canonical path. The OutputDiscoveryDecision provenance for the canonical path is control_plane_generated.",
      "tradeoff": "Agent-authored summaries remain evidence, but deterministic git-backed source-change truth is canonical for audit and review."
    },
    {
      "id": "D-05",
      "title": "Legacy broad discovery policy and frozen run truth",
      "resolves": [
        "PO-01",
        "ARCH-04",
        "ARCH-R2-05",
        "ARCH-R3-02",
        "ARCH-R4-02",
        "SLB-03"
      ],
      "decision": "New runs read workflow YAML discovery.legacy_broad_discovery_policy with allowed values disabled and workflow_opt_in. The workflow compiler freezes the value into the immutable RunPlanSnapshot and the engine passes a typed LegacyBroadDiscoveryPolicy to the discovery pipeline. Editing YAML affects only new runs and reruns. Existing frozen runs do not inherit changed YAML. For an already-started run that needs compatibility fallback, the primary operator path is a retry/resume command carrying legacy_discovery_override_policy and reason so retry scheduling and override persistence happen in the same transaction. The transaction allocates the target StageExecutionId, writes command_journal and legacy_discovery_overrides, and binds the override row to target_stage_execution_id plus target_attempt_number. A standalone overrideLegacyDiscoveryPolicy GraphQL mutation and legacy_discovery_override_create MCP tool may only attach to an already-created pending retry attempt that has not started prompt execution. The command requires PrincipalClass::Operator and is rejected for Agent or Observer callers. Persistence owner is db::repos::legacy_discovery_overrides backed by legacy_discovery_overrides. The row includes override_id, run_id, stage_id, workflow_id, target_stage_execution_id, target_attempt_number, actor_id, reason, created_at, expires_at_attempt, requested_policy, from_policy, approval_source, idempotency_key, consumed_at, and status. The idempotency key is run_id:stage_id:target_attempt_number:requested_policy. engine::recovery::load_legacy_discovery_override reads the bound pending row in the retry/resume scheduling transaction, converts it to LegacyBroadDiscoveryPolicy::WorkflowOptIn for that attempt only, marks it consumed before the resumed agent prompt begins, and fails closed for stale, expired, wrong-stage, wrong-attempt, duplicate-conflicting, already-started, or already-consumed records. GraphQL/MCP readback exposes pending, consumed, rejected, and expired overrides.",
      "tradeoff": "A per-run override is extra recovery surface, but it respects immutable run snapshots and avoids pretending YAML edits mutate in-flight truth."
    },
    {
      "id": "D-06",
      "title": "Exact-read and meta-root security bounds",
      "resolves": [
        "ARCH-03",
        "ARCH-R2-06",
        "ARCH-R3-04",
        "SLB-08",
        "SLB-13",
        "PO-NB-03",
        "PO-NB-06"
      ],
      "decision": "Exact expected-path reads canonicalize existing files, reject symlink escapes, require canonical targets under the per-output authorized root set in ExpectedOutputSpec, read only regular files, and use candidate caps of 10 MiB per output and 64 MiB aggregate accepted declared-output bytes until Phase 0 production sampling validates or tunes those values. Phase 0 samples at least 20 representative recent agent executions and reports p50, p90, and p99 declared output sizes and aggregate accepted bytes. If p90 exceeds either candidate cap, P053 must tune the defaults or add a workflow-level discovery.output_size_policy override before implementation proceeds. Authorized roots are not a permissive global union: machine outputs for write-enabled repo work authorize the effective worktree path for that output; read-only evidence outputs authorize the resolved workspace path only when the declared output path was resolved there; run-owned artifacts authorize the current run chainworks_meta_root; control-plane artifacts authorize their generated target path root; and companion outputs inherit the root class of their machine output unless explicitly resolved under chainworks_meta_root. Wrong-run meta roots and original workspace paths for write-enabled worktree outputs are rejected. Bounded meta-root discovery scans only the current run chainworks_meta_root, never follows symlinks, skips cache/build/runtime directories, and enforces max_files_visited_for_import=500, max_bytes_per_file=1 MiB, and max_total_bytes=10 MiB unless Phase 0 sampling justifies tuned defaults. Generic logs directories are not skipped under chainworks_meta_root so intentional log artifacts can be discovered. Contract-specific maxima are deferred to a follow-up.",
      "tradeoff": "Some large outputs need explicit declared paths or future contract maxima, but P053 avoids unbounded byte reads and avoids silently suppressing run-owned log artifacts."
    },
    {
      "id": "D-07",
      "title": "Durable diagnostics owner",
      "resolves": [
        "ARCH-R2-04",
        "ARCH-R3-03",
        "ARCH-R4-03",
        "ARCH-05",
        "PO-04",
        "SLB-10"
      ],
      "decision": "DiscoveryDiagnostics is an optional ExecutionResult projection for compatibility, but durable ownership is a dedicated agent_execution_discovery_diagnostics table keyed by agent_execution_id, not an overloaded runtime-facts row. The table stores discovery_diagnostics_json as a schema-versioned discovery_diagnostics_v1 payload plus indexed summary columns: discovery_schema_version, legacy_broad_discovery_used, missing_required_output_count, rejected_output_count, stale_output_count, meta_discovery_truncated, git_manifest_status, and resume_warning_count. agent_execution_runtime_facts remains the scalar P058 summary owner for output_settlement and valid_required_outputs; failed-stage evidence embeds a projection or reference to the diagnostics row, not an independent source of truth. Persistence invariant: accepted decisions, diagnostics summary, runtime output settlement, source-generation claims, and active contract import either commit in one transaction or write enough generation identifiers for deterministic daemon-restart reconciliation. Readers that observe diagnostics without matching runtime facts or active artifact generations must show a reconciliation_pending diagnostic and must not treat required outputs as valid until the P057/P058 settlement record is present. Reports, GraphQL, MCP readback, RunDetailPanel, FailedStageEvidencePanel, and ArtifactInspectorView project from the versioned payload and preserve unknown future fields. Old records project defaults and empty decision lists.",
      "tradeoff": "Persisting structured facts creates fixture updates, but logs-only diagnostics cannot support UI/report/readback requirements."
    },
    {
      "id": "D-08",
      "title": "Deterministic proof gate",
      "resolves": [
        "PO-02",
        "ARCH-07",
        "ARCH-R3-05",
        "SLB-04"
      ],
      "decision": "The proposal-053|p053 gate asserts behavior, not wall-clock speed: a fake ACP provider must receive initialize before any traversal under workspace_root or effective worktree_root can occur. P053 introduces an injectable DiscoveryFilesystem boundary used by transport and engine discovery code for directory traversal, file metadata, canonicalization, and file reads. The test implementation records every operation with phase labels and fails if any traversal operation under workspace_root or effective_worktree_root occurs before initialize_sent_at. Git startup checks go through GitManifestRunner and are similarly phase-recorded. Timing fields remain observability, not pass/fail proof.",
      "tradeoff": "Instrumentation is more work than a timing threshold, but it is deterministic across CI hosts."
    },
    {
      "id": "D-09",
      "title": "Git manifest command boundary",
      "resolves": [
        "ARCH-R2-07",
        "ARCH-R4-05",
        "ARCH-06",
        "SLB-11"
      ],
      "decision": "Introduce engine::commands::GitManifestRunner for P053 unless an equivalent timeout/cancellation-aware helper already exists at implementation time. It executes git without a shell, passes args structurally, sets cwd to the effective worktree root, enforces a 5 second timeout, terminates the child on timeout or cancellation, strips no content except NUL parsing boundaries, and returns typed statuses available, timeout, not_git_repository, command_failed, and not_declared without failing ACP execution. The canonical manifest is working-tree-current, not base-revision-relative. It normalizes git status --porcelain=v1 -z into staged_changes, unstaged_changes, deleted_files, renamed_files, untracked_files, and conflicted_files, and records diff_stat_text from the current worktree diff. Base-revision comparison can be added later as a separate manifest field but is not required for P053.",
      "tradeoff": "A named boundary adds a small engine helper, but prevents inconsistent direct command handling."
    },
    {
      "id": "D-10",
      "title": "P037 supervision interaction",
      "resolves": [
        "PO-NB-04"
      ],
      "decision": "P037 idle watchdog supervision ends with ACP prompt completion for one-shot execution. P053 post-prompt discovery, exact-path acceptance, bounded meta-root discovery, and git manifest generation are engine phases with their own bounded timeouts and progress runtime facts. They must not be counted as provider idle time. Live-session reuse still records generation/session ids, but post-prompt discovery is associated with the current invocation attempt.",
      "tradeoff": "This adds phase boundaries to runtime facts, but avoids false provider idle timeout classification."
    },
    {
      "id": "D-12",
      "title": "Exact-output aggregate bounds",
      "resolves": [
        "ARCH-R4-04"
      ],
      "decision": "ExpectedOutputSpec.max_bytes applies uniformly to provider output envelopes, exact-path files, and control-plane generated outputs before they are accepted for declared-output validation. Candidate defaults are 10 MiB per output and 64 MiB aggregate accepted declared-output bytes per agent execution, but they are not final until Phase 0 production output-size sampling confirms p90 declared output and p90 aggregate sizes fit below them. Exact-output acceptance has a 10 second phase timeout and is cancellation-aware through DiscoveryFilesystem. Oversized provider envelopes are rejected before writing to declared paths. When the aggregate cap is reached, remaining undecided outputs receive aggregate_exact_output_cap diagnostics and required outputs settle as missing_required_outputs unless already accepted by a lower-byte provenance path. Bounded meta-root supplemental discovery keeps its separate 10 MiB total candidate cap and cannot satisfy required outputs unless mapped through an ExpectedOutputSpec decision.",
      "tradeoff": "The aggregate cap can reject unusually large multi-output workflows until they declare narrower outputs or future contract-specific maxima, but it bounds memory, disk, and UI payload pressure in the new engine-owned acceptance path."
    },
    {
      "id": "D-11",
      "title": "Operator UI density and semantics",
      "resolves": [
        "UX-01",
        "UX-02",
        "UX-03",
        "UX-NB-02",
        "UI-ISS-01",
        "UI-ISS-02",
        "UI-ISS-03",
        "UI-01",
        "UI-02",
        "UI-03",
        "UI-04",
        "SLB-05",
        "SLB-06",
        "SLB-07",
        "SLB-14",
        "SLB-16"
      ],
      "decision": "RunDetailPanel uses a compact vertical Missing Artifacts list at sidebar widths rather than a five-column table. The sidebar shows the first five missing rows by default, then an 'and N more' disclosure or scroll region so diagnostics cannot push primary run state off-screen. Wider report/detail surfaces may use a table. Copy Path remains available for known expected paths. Open Location is enabled only when the file exists or an existing parent directory can be opened; otherwise it is disabled with a reason tooltip. Discovery Mode is grouped with runtime provenance or diagnostics metadata for normal Exact Path and Bounded Meta Root states, but escalates to a visible warning badge near run status when legacy broad discovery, resume warning, missing expected-output metadata, or override_active is true. Status colors use existing semantic tokens: error red for missing, symlink_escape, unauthorized_root, and contract_invalid; warning orange/yellow for stale_expected_output, oversized, capped, and legacy/resume warnings. Capped indicators expose whether the file-count, per-file byte, or total-byte cap was hit and include the numeric threshold in tooltips.",
      "tradeoff": "The UI contract is more prescriptive, but it prevents unreadable diagnostics in the 280-340 pt sidebar."
    },
    {
      "id": "D-13",
      "title": "Field-complete discovery decision handoff",
      "resolves": [
        "ARCH-R6-01",
        "SLB-02"
      ],
      "decision": "OutputDiscoveryDecision is the single engine-owned handoff from discovery to validation, artifact persistence, diagnostics, runtime facts, and source-generation claims. Its schema includes output_name, output_role, target_path, companion_of, status, reason, provenance, canonical_path, root_class, baseline_status, size_bytes, content_digest, max_bytes_applied, aggregate_bytes_after_acceptance, accepted_payload_ref, accepted_bytes_sha256, generated_by, diagnostics, and decision_at. Accepted decisions are converted by one builder into CapturedOutput records for validate_task_outputs and declared artifact persistence. Missing and rejected decisions never expose accepted bytes or artifact refs.",
      "tradeoff": "A larger decision struct is more explicit than a path list, but it prevents the old raw-disk-read bypass from reappearing in validation or persistence."
    },
    {
      "id": "D-14",
      "title": "Phased delivery and independent gates",
      "resolves": [
        "PO-R2-01",
        "PO-R2-04",
        "PO-R2-05",
        "SUG-ARCH-R6-01",
        "SLB-04",
        "SLB-08",
        "SLB-09"
      ],
      "decision": "P053 delivery is tiered. Phase 0 validates candidate byte caps against at least 20 representative production agent executions and either confirms or tunes the defaults before implementation hard-codes them. Phase 1 is the independently shippable latency and correctness core: no pre-initialize traversal, typed expected outputs, exact-path acceptance, bounded meta-root discovery, missing_required_outputs, accepted-decision validation handoff, core metrics, and the proposal-053|p053 behavioral gate. Phase 2 adds integrity and compatibility hardening: reuse policy, GitManifestRunner, diagnostics persistence, crash reconciliation, legacy override, and richer readback. Phase 3 tracks macOS operator UI surfaces and report polish as a separate work package. The core p053 gate is engine/control-plane proof and does not require local UI smoke tests.",
      "tradeoff": "Phasing exposes that not every valuable surface has to ship atomically, but it keeps the latency fix from being blocked by lower-risk UI polish while still preserving explicit follow-on acceptance criteria."
    }
  ],
  "current_system_facts": [
    "control-plane/crates/acp/src/lib.rs currently defines ExecutionRequest.expected_output_paths as Vec<String> and chainworks_meta_root as Option<String>.",
    "control-plane/crates/acp/src/transport.rs currently calls snapshot_workspace before ACP initialize and again after prompt completion.",
    "control-plane/crates/engine/src/contracts.rs currently has load_declared_output_bytes reading each declared target_path directly from disk.",
    "control-plane/crates/workflow/src/plan.rs OutputSchema currently carries contract id, formats, validation mode, artifact names, and required fields, but no max-size field.",
    "docs/reference/runtime-contract.md states that run start compiles an immutable RunPlanSnapshot and resume uses the stored snapshot, not latest YAML.",
    "P050 defines a daemon-owned .chainworks/runs/{run_id} meta root and CHAINWORKS_META_ROOT propagation for ACP subprocesses.",
    "P057/P058 define canonical artifact contracts, source-generation claims, runtime facts, AgentOutputSettlement::missing_required_outputs, and AgentFailureKind::MissingRequiredOutputs."
  ],
  "proposed_behavior": {
    "fresh_acp_sequence": [
      "Build ExpectedOutputSpec records from declared outputs and companion outputs before ACP execution.",
      "Spawn provider subprocess.",
      "Send ACP initialize without snapshot_workspace, recursive traversal, broad git status, broad git diff, or exact output file reads.",
      "Await initialize under provider handshake timeout.",
      "Send session/new and await response under provider handshake timeout.",
      "Capture pre-prompt metadata only for ExpectedOutputSpec target paths and companion paths.",
      "Send session/prompt and stream transcript and provider output envelopes.",
      "Return prompt result, provider envelopes, timing observations, and pre-prompt metadata to the engine.",
      "Generate declared control-plane artifacts such as changed_files_manifest.",
      "Run engine-owned exact-path acceptance and build OutputDiscoveryDecision records.",
      "Run bounded current-run meta-root discovery for supplemental evidence.",
      "Optionally run capped legacy broad discovery only if the frozen run policy or audited per-run override permits it.",
      "Validate and persist artifacts only from accepted decisions and P057/P058 source-generation truth."
    ],
    "disallowed_pre_initialize_work": [
      "snapshot_workspace(workspace_root)",
      "snapshot_workspace(worktree_root)",
      "recursive find, walkdir, or equivalent traversal under repository, workspace, or worktree",
      "broad git status or git diff as a startup prerequisite",
      "exact output file reads before initialize"
    ],
    "expected_output_spec": {
      "ownership": "Built by the engine from DeclaredOutput, OutputSchema, workflow output config, companion output config, worktree/root policy, and runtime artifact paths.",
      "compatibility": "ExecutionRequest.expected_output_paths remains as a path-only projection for prompt instructions and older adapters. New code consumes expected_outputs: Vec<ExpectedOutputSpec> when present.",
      "workflow_yaml_policy_shape": "outputs remains a list of strings. Per-output settings live in sibling output_policies keyed by output name; this is backward-compatible with AgentTask.outputs: Option<Vec<String>> and existing examples.",
      "authorized_root_derivation": [
        "For write-enabled repo work, declared repo outputs authorize only the effective worktree target path root.",
        "For read-only stages, declared workspace outputs authorize the resolved workspace target path root only.",
        "For run-owned artifacts, declared paths authorize only the current run chainworks_meta_root.",
        "For control-plane generated outputs, declared paths authorize the engine-generated target path root.",
        "For companion outputs, use the machine output root class unless the companion path is explicitly resolved under chainworks_meta_root.",
        "Never authorize a global union of workspace_root, worktree_root, and chainworks_meta_root for every output."
      ],
      "fields": [
        "output_name",
        "output_role: machine|companion|control_plane",
        "target_path",
        "display_label",
        "contract_id",
        "required: bool",
        "reuse_policy: must_produce|allow_unchanged_existing",
        "max_bytes: candidate default 10485760 for exact reads, finalized after Phase 0 production sampling",
        "aggregate_acceptance_cap_bytes: candidate default 67108864 per agent execution, finalized after Phase 0 production sampling",
        "authorized_roots",
        "source_generation_owner",
        "companion_of when applicable"
      ]
    },
    "pre_prompt_expected_output_metadata": {
      "record_fields": [
        "output_name",
        "target_path",
        "canonical_path when resolvable",
        "root_class",
        "existed",
        "file_type",
        "size_bytes",
        "content_digest for regular files at or below max_bytes",
        "mtime_ns diagnostic only",
        "baseline_status"
      ],
      "baseline_statuses": [
        "absent",
        "regular_digest_captured",
        "oversized",
        "unreadable",
        "not_regular_file",
        "symlink_escape",
        "unauthorized_root",
        "uncertain"
      ],
      "comparison_rule": "For must_produce, content_digest difference proves change for files within the exact-read cap. mtime and size are diagnostic only and never prove freshness on their own. Same-size rewrites and coarse timestamp changes must still be detected by digest. Uncertain baselines become typed warnings or rejections according to requiredness and reuse_policy."
    },
    "output_discovery_decision": {
      "ownership": "Engine-owned durable decision consumed by validation and persistence.",
      "fields": [
        "output_name",
        "output_role: machine|companion|control_plane",
        "target_path",
        "companion_of when applicable",
        "status: accepted|missing|rejected",
        "reason",
        "provenance: provider_envelope|exact_path|declared_reuse_policy|control_plane_generated|bounded_meta_root|legacy_broad_discovery",
        "canonical_path when resolvable",
        "root_class",
        "baseline_status",
        "size_bytes",
        "content_digest",
        "max_bytes_applied",
        "aggregate_bytes_after_acceptance",
        "accepted_payload_ref for accepted file-backed or provider-backed payloads",
        "accepted_bytes_sha256 for accepted payloads",
        "generated_by for control-plane artifacts",
        "diagnostics",
        "decision_at"
      ],
      "statuses": [
        "accepted",
        "missing",
        "rejected"
      ],
      "reasons": [
        "provider_envelope",
        "exact_path_new",
        "exact_path_changed",
        "declared_reuse_policy",
        "control_plane_generated",
        "missing_after_prompt",
        "stale_expected_output",
        "baseline_uncertain",
        "baseline_unreadable",
        "oversized",
        "aggregate_exact_output_cap",
        "symlink_escape",
        "unauthorized_root",
        "wrong_run_meta_root",
        "not_regular_file",
        "contract_invalid",
        "read_error"
      ],
      "validation_adapter": "A single builder converts accepted OutputDiscoveryDecision records into CapturedOutput values consumed by validate_task_outputs and declared artifact persistence. The builder rejects missing or rejected decisions and never re-reads target_path from disk.",
      "rule": "Only accepted decisions supply bytes or artifact references to validate_task_outputs and declared artifact persistence. Missing or rejected decisions remain durable diagnostics and cannot be bypassed by later disk reads."
    },
    "bounded_meta_root_discovery": {
      "root_rule": "Scan only the absolute current run chainworks_meta_root. If absent or empty, skip bounded discovery and emit a diagnostic warning. Never fall back to workspace_root scanning.",
      "caps": {
        "max_files_visited_for_import": 500,
        "max_bytes_per_file": 1048576,
        "max_total_bytes": 10485760
      },
      "skip_directories": [
        ".git",
        ".hg",
        ".svn",
        ".claude",
        ".codex",
        ".gemini",
        "node_modules",
        "target",
        "DerivedData",
        ".build",
        "dist",
        "build",
        "tmp",
        "temp",
        "cache"
      ],
      "rules": [
        "Canonicalize the root and each traversed directory.",
        "Never follow symlinks.",
        "Skip any path that canonicalizes outside the meta root.",
        "Read only regular files.",
        "Import small regular files under caps as supplemental evidence.",
        "Do not let supplemental evidence satisfy a declared required output unless it also maps to an accepted ExpectedOutputSpec decision.",
        "Expose truncated_by_file_cap, truncated_by_file_size, and truncated_by_total_bytes."
      ]
    },
    "legacy_broad_discovery": {
      "default": "disabled",
      "new_run_yaml": "discovery.legacy_broad_discovery_policy",
      "allowed_values": [
        "disabled",
        "workflow_opt_in"
      ],
      "frozen_run_behavior": "Compiled into RunPlanSnapshot. Editing YAML after run start does not affect resume.",
      "existing_run_override": "An audited per-run compatibility override can enable workflow_opt_in for the next resume attempt only; otherwise operators rerun with updated YAML.",
      "requirements": [
        "Runs only after prompt completion and control-plane generated artifacts.",
        "Uses a 5 second timeout, max_files_visited=1000, max_bytes_per_file=1 MiB, and max_total_bytes=10 MiB.",
        "Is cancellation-aware and reports timeout, file-count, per-file byte, and total-byte truncation separately.",
        "Does not outrank accepted declared outputs, provider envelopes, or source-generation settlement.",
        "Logs workflow, stage, run id, root, reason, file count, elapsed time, and truncation."
      ]
    }
  },
  "architecture": {
    "acp_transport": [
      "Remove snapshot_workspace from AcpSession::start.",
      "Remove standard post-prompt snapshot diff from transport discovery.",
      "Keep ACP transport responsible for initialize/session-new/session-prompt, transcript streaming, provider envelope extraction, provider timing, MCP observation, close diagnostics, and precise pre-prompt metadata capture for typed expected paths.",
      "Do not let transport decide required/optional output truth."
    ],
    "engine_discovery_pipeline": [
      "Build ExpectedOutputSpec from declared outputs, companions, workflow reuse policy, and path resolution.",
      "Call ACP runtime and receive transcript, provider envelopes, timings, and pre-prompt metadata.",
      "Run generate_changed_files_manifest_if_declared before exact-path acceptance.",
      "Build OutputDiscoveryDecision records for each declared machine and companion output.",
      "Convert accepted OutputDiscoveryDecision records into CapturedOutput values through a single builder.",
      "Replace or overload load_declared_output_bytes so validation reads accepted decision bytes/artifact refs only.",
      "Persist missing, stale, rejected, and accepted decisions into agent_execution_discovery_diagnostics, with P057/P058 settlement and runtime summaries projecting the stage truth.",
      "Set AgentOutputSettlement::missing_required_outputs and AgentFailureKind::MissingRequiredOutputs through P057/P058 for required missing or ineligible outputs."
    ],
    "discovery_filesystem_boundary": {
      "name": "DiscoveryFilesystem",
      "purpose": "Central injectable boundary for P053 filesystem metadata, canonicalization, file reads, and bounded traversal.",
      "requirements": [
        "Transport and engine discovery paths use DiscoveryFilesystem rather than direct recursive filesystem traversal.",
        "Each operation records phase, root, path, operation type, and timestamp in tests.",
        "The proposal-053|p053 fake implementation fails the gate if traversal under workspace_root or effective_worktree_root occurs before initialize_sent_at.",
        "Exact-path metadata and file reads are allowed only after session/new under the exact-path phase.",
        "Bounded traversal is allowed only under current run chainworks_meta_root after prompt completion."
      ]
    },
    "legacy_discovery_override_contract": {
      "primary_command": "Command::RetryStage with optional legacy_discovery_override_policy and reason fields, writing retry attempt and override atomically",
      "secondary_command": "Command::OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd) only for an already-created pending retry attempt that has not started prompt execution",
      "operator_entry_points": [
        "GraphQL retryStage legacyDiscoveryOverride input",
        "GraphQL mutation overrideLegacyDiscoveryPolicy for pending attempts only",
        "MCP tool legacy_discovery_override_create for pending attempts only"
      ],
      "authorization": "Requires CallerContext.principal_class == PrincipalClass::Operator; Agent and Observer callers are rejected before persistence.",
      "storage": "new legacy_discovery_overrides table owned by db::repos::legacy_discovery_overrides",
      "scope": [
        "run_id",
        "stage_id",
        "workflow_id",
        "target_stage_execution_id",
        "target_attempt_number"
      ],
      "fields": [
        "override_id",
        "run_id",
        "stage_id",
        "workflow_id",
        "target_stage_execution_id",
        "target_attempt_number",
        "actor_id",
        "reason",
        "created_at",
        "expires_at_attempt",
        "requested_policy",
        "from_policy",
        "approval_source",
        "idempotency_key",
        "consumed_at",
        "status"
      ],
      "idempotency_key": "run_id:stage_id:target_attempt_number:requested_policy",
      "transaction_boundary": "The primary retry command allocates target_stage_execution_id and writes command_journal plus legacy_discovery_overrides in one transaction. The secondary override command may only update a pending retry attempt in one transaction before prompt execution begins.",
      "engine_read_point": "engine::recovery::load_legacy_discovery_override reads the row bound to the current target_stage_execution_id before prompt execution and converts it to LegacyBroadDiscoveryPolicy::WorkflowOptIn for that attempt only.",
      "clear_semantics": "Mark consumed before the resumed agent prompt begins; reject expired, stale, stage-mismatched, wrong-attempt, already-started, already consumed, or duplicate conflicting overrides.",
      "readback": "GraphQL and MCP expose pending, consumed, rejected, and expired overrides for operator audit."
    },
    "workflow_compiler": [
      "Keep AgentTask.outputs and CompiledTask.outputs as list-based artifact names for backward compatibility.",
      "Add optional sibling map AgentTask.output_policies and CompiledTask.output_policies keyed by output name.",
      "Support output_policies.<output_name>.reuse_policy with default must_produce and allowed value allow_unchanged_existing.",
      "Reject output_policies keys not present in outputs and reject unknown reuse_policy values with clear compiler errors.",
      "Add matching Swift WorkflowDefinition and run-plan snapshot mirror fields with serde/Decodable defaults.",
      "Ensure existing list-only workflows compile unchanged and preserve snapshot hash behavior except for explicit output_policies additions.",
      "Add discovery.legacy_broad_discovery_policy with default disabled and allowed value workflow_opt_in.",
      "Run Phase 0 output-size sampling before finalizing cap defaults; if p90 declared or aggregate outputs exceed candidate caps, either tune defaults or add workflow-level discovery.output_size_policy with max_exact_output_bytes and max_aggregate_declared_output_bytes.",
      "Reject duplicate declared output names or target paths within the same stage unless the duplicate is an explicit companion relationship; control_plane outputs such as changed_files_manifest must have a deterministic owner.",
      "Freeze both policies into RunPlanSnapshot so resume uses immutable truth.",
      "Do not add per-contract size-max fields in P053; populate ExpectedOutputSpec.max_bytes from validated P053 defaults or a validated workflow-level output_size_policy."
    ],
    "changed_files_manifest": {
      "engine_step": "generate_changed_files_manifest_if_declared",
      "sequence": "after ACP prompt completion and before exact-path acceptance, declared-output import, and validation",
      "trigger": "workflow declares changed_files_manifest and invocation has repo-backed write worktree",
      "runner": "engine::commands::GitManifestRunner",
      "commands": [
        "git -C <effective_worktree_root> diff --name-only -z",
        "git -C <effective_worktree_root> diff --stat",
        "git -C <effective_worktree_root> status --porcelain=v1 -z --untracked-files=all"
      ],
      "timeout_ms": 5000,
      "normalization": {
        "scope": "working_tree_current",
        "porcelain_xy_mapping": [
          "index column changes become staged_changes",
          "worktree column changes become unstaged_changes",
          "D in either column becomes deleted_files with staged or unstaged phase metadata",
          "R entries become renamed_files with old_path and new_path",
          "?? entries become untracked_files",
          "U or unmerged combinations become conflicted_files"
        ],
        "base_revision_relative": "out of scope for P053; future manifests may add a base_revision_diff section without changing the working_tree_current semantics"
      },
      "statuses": [
        "available",
        "timeout",
        "not_git_repository",
        "command_failed",
        "not_declared"
      ],
      "manifest_fields": [
        "schema_version",
        "status",
        "scope: working_tree_current",
        "worktree_root",
        "git_repository_root",
        "staged_changes",
        "unstaged_changes",
        "deleted_files",
        "renamed_files",
        "conflicted_files",
        "untracked_files",
        "diff_stat_text",
        "ignored_or_unavailable_reason",
        "generated_at",
        "source",
        "timeout_ms"
      ]
    },
    "durable_diagnostics": {
      "transient_projection": "ExecutionResult.discovery_diagnostics: Option<DiscoveryDiagnostics> with serde defaults.",
      "durable_owner": "new agent_execution_discovery_diagnostics table keyed by agent_execution_id, plus scalar summaries in the same table.",
      "payload_shape": "discovery_diagnostics_v1 JSON payload with schema_version, run_id, stage_id, agent_execution_id, discovery_policy, phase_timings, expected_output_decisions, missing_required_outputs, meta_root_discovery, legacy_override, git_manifest, warnings, and ui_labels.",
      "indexed_summary_fields": [
        "discovery_schema_version",
        "legacy_broad_discovery_used",
        "missing_required_output_count",
        "rejected_output_count",
        "stale_output_count",
        "meta_discovery_truncated",
        "git_manifest_status",
        "resume_warning_count"
      ],
      "physical_storage": [
        "agent_execution_discovery_diagnostics.agent_execution_id primary key",
        "agent_execution_discovery_diagnostics.discovery_diagnostics_json",
        "agent_execution_discovery_diagnostics indexed summary columns",
        "failed-stage evidence stores a projection/reference to the diagnostics row rather than a competing copy"
      ],
      "persisted_fields": [
        "phase timing fields",
        "legacy discovery policy and use",
        "meta-root cap and truncation fields",
        "expected-output decision rows",
        "artifact display labels",
        "missing required output rows",
        "git manifest status",
        "resume compatibility warnings"
      ],
      "log_only_fields": [
        "verbose provider stderr diagnostics already covered by existing logs",
        "raw git stderr beyond typed manifest failure summary"
      ],
      "persistence_invariant": "accepted decisions, diagnostics summary, runtime output settlement, source-generation claims, and active contract import commit together when possible. If a crash leaves a partial write, daemon restart reconciliation uses agent_execution_id, artifact_generation_id, and decision_at to either complete projections or mark reconciliation_pending without validating required outputs.",
      "partial_read_behavior": "If diagnostics exist without matching runtime facts or active artifact generations, GraphQL/MCP/reports show reconciliation_pending and required outputs remain invalid until P057/P058 settlement exists.",
      "readback": "Reports, GraphQL, MCP, RunDetailPanel, FailedStageEvidencePanel, and ArtifactInspectorView project from the versioned payload and indexed summaries. Unknown future fields are preserved in GraphQL/MCP readback as opaque extension data. Old records project empty/default diagnostics."
    },
    "macos_operator_surfaces": [
      "RunDetailPanel shows Discovery Mode in diagnostics/runtime metadata grouping, not as an extra primary header badge.",
      "RunDetailPanel shows a compact vertical Missing Artifacts list at sidebar widths, limits the default visible rows to five, and uses a disclosure or scroll region for additional rows.",
      "FailedStageEvidencePanel shows a dedicated Missing Required Outputs section.",
      "Stage Detail and Evidence panels show Startup Performance grouped into Forge overhead and Provider latency.",
      "Run Report and Stage Detail show Capped warnings with the specific cap reason and numeric threshold.",
      "Open Location is disabled for missing files unless an existing parent directory can be opened; Copy Path remains available for known expected paths.",
      "ArtifactInspectorView renders changed_files_manifest as Source Changes with staged, unstaged, deleted, renamed, untracked, conflicted, and diff stat groups plus raw JSON fallback."
    ]
  },
  "ux_ui_notes": {
    "operator_behavior": [
      "A fresh run should show provider startup promptly instead of appearing stuck before initialize.",
      "Missing required outputs must be visible as a typed missing_required_outputs state with artifact labels and expected paths.",
      "Discovery Mode must make exact-path, bounded meta-root, legacy fallback, and resume warning behavior explicit.",
      "Startup timing must separate Forge overhead from provider latency.",
      "Stale outputs must look distinct from never-produced outputs."
    ],
    "visual_contract": [
      "Use compact vertical rows or grouped boxes for missing artifacts inside narrow sidebars.",
      "Use wider tables only in full detail/report contexts.",
      "Keep normal Discovery Mode in diagnostics/runtime metadata, but promote legacy fallback, resume warning, missing metadata, and override_active states to a warning badge near primary run status.",
      "Use semantic error red for missing, symlink_escape, unauthorized_root, and contract_invalid.",
      "Use warning orange/yellow for stale_expected_output, oversized, capped, legacy fallback, and resume compatibility warnings.",
      "Provide tooltips or inline detail affordances for Discovery Mode and Capped indicators, including the numeric cap threshold that was hit.",
      "Show at most five missing artifact rows in the narrow sidebar before a disclosure or scroll region.",
      "Provide Copy Path for known expected paths; Open Location opens the existing file or nearest existing parent directory and is disabled with a reason when neither is available.",
      "Track Declare as Output as a named follow-up, not a P053 requirement."
    ]
  },
  "delivery_phases": [
    {
      "phase": "Phase 0",
      "name": "Cap Validation And Contract Freeze",
      "exit_criteria": [
        "Sample at least 20 representative recent agent executions.",
        "Report p50, p90, and p99 declared output file sizes and aggregate accepted declared-output bytes.",
        "Confirm p90 fits below 10 MiB per output and 64 MiB aggregate, or tune defaults/add workflow-level discovery.output_size_policy before implementation proceeds.",
        "Freeze OutputDiscoveryDecision, CapturedOutput builder, ExpectedOutputSpec, DiscoveryFilesystem, and GitManifestRunner interfaces."
      ],
      "gate_relationship": "Required before Phase 1 implementation starts; not a runtime gate."
    },
    {
      "phase": "Phase 1",
      "name": "Core Startup Latency Fix",
      "exit_criteria": [
        "Remove pre-initialize snapshot_workspace and standard post-prompt broad snapshot diff.",
        "Send initialize before any traversal under workspace_root or effective worktree_root.",
        "Build ExpectedOutputSpec and PrePromptExpectedOutputMetadata for declared outputs.",
        "Accept exact-path/provider/control-plane outputs through OutputDiscoveryDecision and CapturedOutput only.",
        "Bound supplemental discovery to current run chainworks_meta_root.",
        "Set missing_required_outputs for required outputs that are missing, stale, rejected, or over cap.",
        "Emit core phase timing and count metrics.",
        "Pass proposal-053|p053 behavioral gate."
      ],
      "gate_relationship": "Independently shippable engine/control-plane proof; does not require macOS UI smoke tests."
    },
    {
      "phase": "Phase 2",
      "name": "Integrity, Compatibility, And Durable Readback",
      "exit_criteria": [
        "Enable output_policies reuse_policy with Rust and Swift schema parity.",
        "Generate changed_files_manifest through GitManifestRunner.",
        "Persist discovery_diagnostics_v1 in agent_execution_discovery_diagnostics with restart reconciliation.",
        "Add race-free legacy discovery override for frozen resumes.",
        "Implement GraphQL, MCP, reports, and recovery readback for diagnostics and overrides.",
        "Add duplicate declared-output detection for multi-task stages."
      ],
      "gate_relationship": "Can be verified after Phase 1 without weakening the no-pre-initialize-scan guarantee."
    },
    {
      "phase": "Phase 3",
      "name": "Operator UI Surfaces",
      "exit_criteria": [
        "Map diagnostics to RunDetailPanel, FailedStageEvidencePanel, Stage Detail, Run Report, and ArtifactInspectorView.",
        "Apply compact sidebar row limits, warning badge promotion, Capped threshold tooltips, Copy Path, and Open Location behavior.",
        "Track Declare as Output as a named follow-up outside P053."
      ],
      "gate_relationship": "Tracked UI work package with its own acceptance subset; the core p053 gate remains Rust control-plane behavior."
    }
  ],
  "rollout": {
    "implementation_order": [
      "Phase 0: sample at least 20 representative production agent executions and finalize candidate cap defaults or workflow-level cap override policy.",
      "Phase 0: define OutputDiscoveryDecision and CapturedOutput builder interfaces before transport rewrites or parallel coding.",
      "Add failing tests for no traversal before initialize, stale file rejection bypass prevention, and changed_files_manifest sequencing.",
      "Add DiscoveryFilesystem and GitManifestRunner test seams so traversal and git operations are phase-recorded.",
      "Add ExpectedOutputSpec, PrePromptExpectedOutputMetadata, OutputDiscoveryDecision, output_policies reuse_policy, and legacy discovery policy data shapes with serde defaults.",
      "Add backward-compatible workflow output_policies map to Rust and Swift schema mirrors while preserving existing outputs list behavior.",
      "Add agent_execution_discovery_diagnostics table with discovery_diagnostics_v1 payload and indexed summaries.",
      "Add retry/resume legacy discovery override input, pending-attempt-only OverrideLegacyDiscoveryPolicy entry points, legacy_discovery_overrides table, and recovery loader.",
      "Remove pre-initialize snapshot_workspace and standard post-prompt snapshot diff from ACP transport.",
      "Capture pre-prompt metadata for expected paths after session/new and before session/prompt.",
      "Add engine::commands::GitManifestRunner and generate_changed_files_manifest_if_declared.",
      "Replace disk-based declared-output validation reads with accepted decision inputs.",
      "Add exact-path acceptance security, freshness, reuse, and size checks.",
      "Add bounded current-run meta-root discovery and absent-meta-root warnings.",
      "Persist discovery diagnostics into agent_execution_discovery_diagnostics and write P057/P058 settlement/runtime summary projections.",
      "Add macOS view-model/UI mappings for missing outputs, discovery mode, startup performance, capped warnings, and source changes.",
      "Register proposal-053|p053 in scripts/test-gate.sh and document it in docs/reference/test-gates.md.",
      "Update docs/reference ACP artifact discovery and runtime fact documentation."
    ],
    "migration_runbook": [
      "Before resume, identify affected in-flight runs by querying run snapshots or runtime facts where expected_outputs are absent/empty, discovery schema version is before p053, or legacy broad discovery was previously inferred from snapshot diff.",
      "For new attempts, add declared output paths and optional discovery.legacy_broad_discovery_policy to workflow YAML before rerun.",
      "For already-frozen runs, do not assume YAML edits affect resume. Either rerun with a newly compiled snapshot or use the retry/resume legacy discovery override input so the target StageExecutionId and override row are created atomically. The standalone overrideLegacyDiscoveryPolicy and legacy_discovery_override_create paths are only for already-created pending retry attempts that have not started prompt execution.",
      "A valid override is consumed before the resumed agent prompt begins; stale, expired, duplicate conflicting, or wrong-stage overrides fail closed and leave legacy broad discovery disabled.",
      "At resume time, emit a warning naming run id, workflow, stage, frozen policy, expected-output completeness, and whether a compatibility override is active.",
      "Operators confirm P053 is active by seeing acp_pre_initialize_local_latency_ms for every ACP session and no acp_legacy_broad_discovery_used warnings for migrated workflows."
    ],
    "rollback": [
      "The removal of pre-initialize broad scanning can be reverted independently only if exact-path discovery has a critical regression.",
      "Workflow-level or audited run-level legacy broad discovery can temporarily support specific workflows while declarations are fixed.",
      "Git manifest generation can return timeout or unavailable without failing ACP execution.",
      "Strict stale-output enforcement can be temporarily downgraded to warning only through an explicit rollout flag if production migration proves too disruptive; the default target behavior remains strict missing_required_outputs."
    ]
  },
  "metrics_and_observability": {
    "structured_fields": [
      "acp_pre_initialize_local_latency_ms",
      "acp_initialize_latency_ms",
      "acp_session_new_latency_ms",
      "acp_prompt_duration_ms",
      "acp_pre_prompt_metadata_latency_ms",
      "acp_control_plane_manifest_latency_ms",
      "acp_exact_output_acceptance_latency_ms",
      "acp_meta_root_discovery_latency_ms",
      "acp_git_changed_files_latency_ms",
      "acp_expected_outputs_found_count",
      "acp_expected_outputs_missing_count",
      "acp_expected_outputs_stale_count",
      "acp_expected_outputs_rejected_count",
      "acp_meta_discovery_truncated",
      "acp_meta_discovery_truncation_reason",
      "acp_legacy_broad_discovery_policy",
      "acp_legacy_broad_discovery_used",
      "acp_git_manifest_status",
      "acp_resume_discovery_warning",
      "acp_discovery_schema_version",
      "acp_discovery_override_status",
      "acp_missing_required_output_count",
      "acp_rejected_output_count",
      "acp_stale_output_count",
      "acp_exact_output_acceptance_timeout",
      "acp_exact_output_aggregate_bytes",
      "acp_exact_output_aggregate_cap_hit",
      "acp_cap_validation_sample_size",
      "acp_cap_validation_p90_output_bytes",
      "acp_cap_validation_p90_aggregate_bytes",
      "acp_legacy_broad_discovery_timeout_ms",
      "acp_legacy_broad_discovery_truncation_reason",
      "acp_reconciliation_pending"
    ],
    "production_confirmation": [
      "Every ACP session emits acp_pre_initialize_local_latency_ms and provider latency fields.",
      "On the reference approximately 8.9 GB / 126,643-file workspace, acp_pre_initialize_local_latency_ms should be below 1000 ms for fresh sessions as an observability target, not as the deterministic gate assertion.",
      "Phase 0 records production cap validation p50, p90, and p99 values before final defaults are frozen.",
      "Warning-level logs are emitted whenever acp_legacy_broad_discovery_used is true, naming workflow, stage, and run id.",
      "Warning-level logs are emitted when bounded meta-root caps are hit, including the exact cap reason.",
      "Representative large-workspace runs show initialize sent before any workspace traversal hook executes.",
      "discovery_diagnostics_v1 payloads remain readable through report, GraphQL, and MCP projections with unknown future fields preserved."
    ],
    "ui_mapping": [
      "Runtime fact timing fields drive Startup Performance.",
      "Expected-output decisions drive Missing Artifacts and Missing Required Outputs rows.",
      "meta_discovery_truncated and truncation reason drive Capped indicators.",
      "legacy fallback and resume warning facts drive Discovery Mode warning states.",
      "git_manifest_status and manifest artifact content drive Source Changes rendering."
    ]
  },
  "tests_and_proof_gate": {
    "gate": "proposal-053|p053",
    "registration_targets": [
      "scripts/test-gate.sh",
      "docs/reference/test-gates.md"
    ],
    "behavioral_assertions": [
      "A fake ACP provider receives initialize before any traversal under workspace_root or effective worktree_root can occur.",
      "Pre-initialize startup does not call DiscoveryFilesystem traversal, snapshot_workspace, walkdir, find, or broad git status/diff hooks.",
      "Provider handshake latency is reported separately from local pre-initialize latency.",
      "Rejected OutputDiscoveryDecision records cannot be bypassed by load_declared_output_bytes or declared artifact persistence.",
      "Phase 1 p053 gate can pass with legacy broad discovery fully disabled."
    ],
    "focused_tests": [
      "No pre-initialize broad scan using an instrumented traversal hook and fake provider.",
      "DiscoveryFilesystem records phase-labeled operations and fails the gate if traversal under workspace_root or effective_worktree_root happens before initialize_sent_at.",
      "ExpectedOutputSpec is built with labels, requiredness, reuse_policy, authorized roots, and default max_bytes.",
      "Phase 0 cap validation fixture records at least 20 sampled executions and fails proposal readiness if candidate caps are frozen without p50/p90/p99 output-size data or a documented cap-tuning decision.",
      "OutputDiscoveryDecision schema contains the field-complete validation handoff and the CapturedOutput builder rejects missing or rejected decisions.",
      "Existing outputs: [artifact] workflows compile unchanged, while output_policies can set reuse_policy for named outputs and reject unknown keys or values.",
      "Duplicate declared output names or target paths within a multi-task stage fail compilation or produce deterministic wrong-owner diagnostics before execution.",
      "PrePromptExpectedOutputMetadata captures content digests for files within cap and does not use mtime/size alone as freshness proof.",
      "Same-size stale files and coarse-timestamp rewrites are detected by digest and settle as stale_expected_output when provider omits them.",
      "Unreadable, oversized, escaped, unauthorized, non-regular, and uncertain baselines produce typed warning or rejection states.",
      "A stale required file exists before prompt and provider omits it; the decision is stale_expected_output and settlement is missing_required_outputs.",
      "reuse_policy allow_unchanged_existing accepts unchanged files only after normal security and size checks and records declared_reuse_policy provenance.",
      "Authorized roots are derived per ExpectedOutputSpec and reject wrong-run meta-root paths and original workspace paths for write-enabled worktree outputs.",
      "Exact expected path symlink escape, unauthorized root, non-regular file, and oversized file are rejected with typed diagnostics.",
      "Provider envelopes, exact-path files, and control-plane generated declared outputs all enforce validated ExpectedOutputSpec.max_bytes and aggregate acceptance caps.",
      "Exact-output acceptance times out after 10 seconds, is cancellation-aware, and reports aggregate_exact_output_cap when the aggregate cap is reached.",
      "Accepted decisions, not raw target_path reads, feed validate_task_outputs.",
      "changed_files_manifest is generated before exact-path acceptance and can satisfy its declared output.",
      "Agent-authored changed_files_manifest conflicts are preserved as changed_files_manifest.agent.json while the control-plane manifest is canonical.",
      "Bounded meta-root discovery imports supplemental run-owned files within caps and reports file-count, per-file-byte, and total-byte truncation reasons.",
      "Absent chainworks_meta_root skips bounded discovery and warns without workspace fallback.",
      "Legacy broad discovery fallback enforces 5 second timeout, 1000 visited-file cap, 1 MiB per-file cap, 10 MiB total cap, cancellation, and truncation diagnostics.",
      "Legacy broad discovery is disabled by default, compiled into RunPlanSnapshot for new runs, and available to frozen runs only through audited per-run override.",
      "Audited per-run legacy discovery overrides are bound to target_stage_execution_id or rejected for already-started attempts; race tests cover duplicate overrides, retry-before-override, override-before-retry, and consumed-before-prompt semantics.",
      "RetryStage legacy override input requires operator principal, writes command journal and legacy_discovery_overrides atomically, and exposes GraphQL/MCP readback.",
      "GitManifestRunner returns available, timeout, not_git_repository, and command_failed without failing ACP execution.",
      "Changed-files manifest normalizes porcelain statuses into staged, unstaged, deleted, renamed, untracked, and conflicted groups with working_tree_current scope.",
      "discovery_diagnostics_v1 persists detailed payloads plus indexed summaries and preserves unknown future fields in readback.",
      "Crash/restart or partial-write tests prove diagnostics, runtime settlement, source-generation claims, and active contract import either commit together or reconcile to reconciliation_pending without validating required outputs.",
      "P037 idle watchdog does not classify post-prompt discovery or git manifest generation as provider idle time.",
      "Missing required outputs include display labels, output ids, expected absolute paths, statuses, and diagnostic reasons.",
      "RunDetailPanel view-model maps missing outputs to compact vertical rows at sidebar width, limits default visible rows, and defines disabled/open-nearest-parent behavior for Open Location.",
      "Status color/icon mapping covers missing, stale, oversized, capped, symlink_escape, unauthorized_root, and contract_invalid."
    ]
  },
  "risks_and_mitigations": [
    {
      "risk": "Candidate byte caps are too low for real workflows.",
      "impact": "Valid outputs could be rejected as oversized and required outputs could falsely settle as missing_required_outputs.",
      "mitigation": "Phase 0 samples at least 20 representative production executions and reports p50/p90/p99 per-output and aggregate sizes. If p90 exceeds candidate caps, P053 tunes defaults or adds workflow-level discovery.output_size_policy before implementation proceeds."
    },
    {
      "risk": "A legacy workflow relied on implicit broad discovery outside declared paths.",
      "impact": "Artifacts may be missed under the new default.",
      "mitigation": "Expose detection queries, Discovery Mode warnings, workflow-level fallback for new runs, audited per-run compatibility override for frozen resumes, and guidance to add declared outputs."
    },
    {
      "risk": "ExpectedOutputSpec construction is incomplete or mismatches prompt path rendering.",
      "impact": "Required outputs may be falsely missing or mislabeled.",
      "mitigation": "Build specs from the same DeclaredOutput and companion path rendering used by the prompt builder; test machine and companion outputs together."
    },
    {
      "risk": "Stale-output enforcement breaks workflows that intentionally reuse artifacts.",
      "impact": "Previously passing workflows may fail as missing_required_outputs.",
      "mitigation": "Provide explicit per-output reuse_policy syntax with runtime provenance, and allow a short rollout observation flag if production migration needs it."
    },
    {
      "risk": "P053 adds new runtime fact and fixture surface.",
      "impact": "Implementation touches reports, UI view models, MCP/GraphQL readback, and tests.",
      "mitigation": "Use serde defaults, project empty diagnostics for old records, and implement view-model mapping after engine facts are stable."
    },
    {
      "risk": "Diagnostics, settlement, and artifact import can partially persist across a crash.",
      "impact": "Readback could show diagnostics that disagree with runtime facts or active artifact generations.",
      "mitigation": "Use a transaction or deterministic restart reconciliation invariant; readers show reconciliation_pending and do not validate required outputs until P057/P058 settlement exists."
    },
    {
      "risk": "The legacy discovery override subsystem is too costly for a short migration window.",
      "impact": "Implementation effort could delay the core latency fix.",
      "mitigation": "Keep the override in Phase 2, retire it after the migration window unless telemetry shows continued need, and keep the Phase 1 gate independent of the fallback."
    },
    {
      "risk": "Meta-root caps hide supplemental files.",
      "impact": "Operators may not see all run-owned evidence.",
      "mitigation": "Show the exact cap reason in UI/logs and require critical outputs to be declared exact paths."
    },
    {
      "risk": "Git manifest generation is slow or unreliable in large repos.",
      "impact": "Post-prompt evidence may be delayed or unavailable.",
      "mitigation": "Use GitManifestRunner with a 5 second timeout, cancellation, structured args, and nonfatal status results."
    },
    {
      "risk": "Security boundaries around file paths are missed.",
      "impact": "External agent paths could escape authorized roots or cause large reads.",
      "mitigation": "Require security review before PR landing; if a named security reviewer is unavailable, record formal risk acceptance before merge."
    }
  ],
  "open_questions": [
    {
      "id": "OQ-01",
      "question": "Should contract-specific output size maxima be added to catalog contracts after P053?",
      "default_for_implementation": "No contract-specific maxima in P053. Phase 0 validates global candidate caps and may tune them or add workflow-level discovery.output_size_policy. Track contract-specific maxima as follow-up P053A.",
      "blocking": false
    },
    {
      "id": "OQ-02",
      "question": "Should Startup Performance default to a segmented horizontal duration bar or categorized compact list?",
      "default_for_implementation": "Prefer the segmented bar when enough width exists; use categorized compact list in narrow sidebars. Both must separate Forge overhead from Provider latency.",
      "blocking": false
    },
    {
      "id": "OQ-03",
      "question": "Should Declare as Output be added as a quick-fix action?",
      "default_for_implementation": "Not in P053. Track as follow-up because it edits workflow YAML and needs separate permissions, validation, and frozen-run semantics.",
      "blocking": false
    }
  ],
  "reviewer_feedback_resolution": [
    {
      "issue_id": "PO-B-01",
      "status": "addressed",
    "resolution": "Defined output_policies.<output_name>.reuse_policy with allowed values must_produce and allow_unchanged_existing, compiler freezing behavior, acceptance semantics, and runtime provenance while preserving outputs as a list."
    },
    {
      "issue_id": "ARCH-R2-01",
      "status": "addressed",
      "resolution": "Added engine-owned OutputDiscoveryDecision handoff and required validation/persistence to consume accepted decisions instead of raw disk reads."
    },
    {
      "issue_id": "ARCH-R2-02",
      "status": "addressed",
      "resolution": "Added ExpectedOutputSpec with labels, contract id, requiredness, reuse policy, max bytes, authorized roots, and source-generation owner while retaining path-only compatibility projection."
    },
    {
      "issue_id": "ARCH-R2-03",
      "status": "addressed",
      "resolution": "Sequenced generate_changed_files_manifest_if_declared before exact-path acceptance and validation."
    },
    {
      "issue_id": "ARCH-R2-04",
      "status": "addressed",
    "resolution": "Named agent_execution_discovery_diagnostics as the durable diagnostics owner, with P058 runtime facts and failed-stage evidence carrying summaries or projections."
    },
    {
      "issue_id": "ARCH-R2-05",
      "status": "addressed",
      "resolution": "Separated new-run YAML policy from frozen-run resume behavior and added audited per-run compatibility override."
    },
    {
      "issue_id": "ARCH-R2-06",
      "status": "addressed",
      "resolution": "Resolved size policy as fixed P053 defaults with contract-specific maxima deferred to P053A."
    },
    {
      "issue_id": "ARCH-R2-07",
      "status": "addressed",
      "resolution": "Named engine::commands::GitManifestRunner and specified shell-free args, cwd, timeout, cancellation, and typed statuses."
    },
    {
      "issue_id": "PO-NB-01",
      "status": "addressed",
      "resolution": "Clarified P051 is related but not a hard dependency and the P053 gate is independent."
    },
    {
      "issue_id": "PO-NB-02",
      "status": "addressed",
      "resolution": "Changed security review from soft optionality to required before PR landing or formal risk acceptance."
    },
    {
      "issue_id": "PO-NB-03",
      "status": "addressed",
      "resolution": "Added cap rationale and made cap-specific UI/log indicators required."
    },
    {
      "issue_id": "PO-NB-04",
      "status": "addressed",
      "resolution": "Defined P037 watchdog boundary and separate engine phase timeouts/progress facts."
    },
    {
      "issue_id": "PO-NB-05",
      "status": "addressed",
      "resolution": "Added migration detection query/runbook for pre-P053 and incomplete expected-output snapshots."
    },
    {
      "issue_id": "PO-NB-06",
      "status": "addressed",
      "resolution": "Removed generic logs from meta-root skip directories and clarified intentional run-owned log artifacts are discoverable."
    },
    {
      "issue_id": "UI-ISS-01",
      "status": "addressed",
      "resolution": "Replaced narrow sidebar table requirement with compact vertical rows and reserved tables for wider surfaces."
    },
    {
      "issue_id": "UI-ISS-02",
      "status": "addressed",
      "resolution": "Grouped Discovery Mode with runtime provenance/diagnostics rather than adding an unprioritized header badge."
    },
    {
      "issue_id": "UI-ISS-03",
      "status": "addressed",
      "resolution": "Specified semantic status color mapping for error and warning discovery states."
    },
    {
      "issue_id": "UX-ISSUE-01",
      "status": "deferred",
      "resolution": "Declare as Output is tracked as a follow-up because it edits workflow YAML and needs separate frozen-run semantics."
    },
    {
      "issue_id": "UX-ISSUE-02",
      "status": "addressed",
      "resolution": "Capped indicators now expose whether file-count, per-file byte, or total-byte limits were hit."
    },
    {
      "issue_id": "ARCH-R3-01",
      "status": "addressed",
      "resolution": "Defined PrePromptExpectedOutputMetadata fields, digest-based comparison, and tests for same-size stale files and coarse-timestamp rewrites."
    },
    {
      "issue_id": "ARCH-R3-02",
      "status": "addressed",
      "resolution": "Specified one-shot legacy discovery override storage, scope, fields, load point, expiry, idempotency, and consume-before-prompt behavior."
    },
    {
      "issue_id": "ARCH-R3-03",
      "status": "addressed",
      "resolution": "Specified discovery_diagnostics_v1 as a versioned detailed payload with compact indexed summaries and unknown-field preservation in readback."
    },
    {
      "issue_id": "ARCH-R3-04",
      "status": "addressed",
      "resolution": "Defined per-output authorized_roots derivation instead of a permissive global root union."
    },
    {
      "issue_id": "ARCH-R3-05",
      "status": "addressed",
      "resolution": "Named DiscoveryFilesystem as the traversal, metadata, canonicalization, and file-read instrumentation seam for the p053 gate."
    },
    {
      "issue_id": "UX-NB-02",
      "status": "addressed",
      "resolution": "Normal Discovery Mode remains grouped with diagnostics, while legacy fallback, resume warning, missing metadata, and override_active states are promoted to visible warning badges near run status."
    },
    {
      "issue_id": "ARCH-R4-01",
      "status": "addressed",
      "resolution": "Resolved reuse_policy schema as a backward-compatible sibling output_policies map keyed by existing outputs entries, with Rust/Swift schema parity, defaults, validation errors, snapshot behavior, and compatibility tests."
    },
    {
      "issue_id": "ARCH-R4-02",
      "status": "addressed",
      "resolution": "Specified Command::OverrideLegacyDiscoveryPolicy, GraphQL/MCP operator entry points, PrincipalClass::Operator authorization, legacy_discovery_overrides storage, idempotency key, transaction boundary, recovery load point, consume-before-prompt semantics, and readback."
    },
    {
      "issue_id": "ARCH-R4-03",
      "status": "addressed",
      "resolution": "Moved durable discovery diagnostics to a dedicated agent_execution_discovery_diagnostics table with discovery_diagnostics_v1 JSON and indexed summaries."
    },
    {
      "issue_id": "ARCH-R4-04",
      "status": "addressed",
      "resolution": "Added uniform per-output max_bytes enforcement for provider envelopes, exact paths, and control-plane outputs, plus 64 MiB aggregate cap and 10 second exact-output acceptance timeout."
    },
    {
      "issue_id": "ARCH-R4-05",
      "status": "addressed",
      "resolution": "Defined changed-files manifest as working-tree-current and added porcelain status normalization for staged, unstaged, deleted, renamed, untracked, and conflicted files."
    },
    {
      "issue_id": "PO-R2-01",
      "status": "addressed",
      "resolution": "Added Delivery Phases with Phase 0 cap validation, Phase 1 core latency gate, Phase 2 integrity/readback hardening, and Phase 3 UI surfaces."
    },
    {
      "issue_id": "PO-R2-02",
      "status": "addressed",
      "resolution": "Changed byte caps from asserted fixed values to candidate defaults gated by Phase 0 production sampling across at least 20 representative executions, with a required tune-or-workflow-override decision if p90 exceeds the candidates."
    },
    {
      "issue_id": "PO-R2-03",
      "status": "addressed",
      "resolution": "Kept the full audited override model for safety, moved it to Phase 2, made the Phase 1 gate independent of fallback, and added a migration-window retirement note through risks and phased delivery."
    },
    {
      "issue_id": "PO-R2-04",
      "status": "addressed",
      "resolution": "Separated macOS UI surfaces into Phase 3 with a tracked acceptance subset; the core p053 gate remains Rust control-plane behavior."
    },
    {
      "issue_id": "PO-R2-05",
      "status": "addressed",
      "resolution": "Added a non-deterministic production observability target of acp_pre_initialize_local_latency_ms below 1000 ms on the reference large workspace while retaining behavioral no-traversal as the deterministic gate."
    },
    {
      "issue_id": "ARCH-R6-01",
      "status": "addressed",
      "resolution": "Added a field-complete OutputDiscoveryDecision schema and a single CapturedOutput builder that validation and declared artifact persistence must consume."
    },
    {
      "issue_id": "ARCH-R6-02",
      "status": "addressed",
      "resolution": "Made legacy override identity race-free by binding overrides to a target_stage_execution_id allocated by the retry command transaction, with standalone overrides allowed only for pending not-yet-started retry attempts."
    },
    {
      "issue_id": "ARCH-R6-03",
      "status": "addressed",
      "resolution": "Replaced soft 'where practical' persistence wording with a transaction-or-reconciliation invariant and reader behavior for reconciliation_pending partial writes."
    },
    {
      "issue_id": "ARCH-R6-04",
      "status": "addressed",
      "resolution": "Specified legacy broad discovery fallback caps: 5 second timeout, 1000 visited files, 1 MiB per file, 10 MiB total, cancellation, and truncation diagnostics."
    },
    {
      "issue_id": "ARCH-R6-05",
      "status": "addressed",
      "resolution": "Added compiler/engine validation for duplicate declared output names or target paths within multi-task stages, especially control-plane outputs such as changed_files_manifest."
    },
    {
      "issue_id": "UX-ISS-01",
      "status": "deferred",
      "resolution": "Declare as Output remains a high-priority follow-up because it edits workflow YAML and needs separate permission and frozen-run semantics."
    },
    {
      "issue_id": "UX-ISS-02",
      "status": "addressed",
      "resolution": "Kept segmented bar for wide layouts and compact list for narrow layouts, requiring dynamic switching by width."
    },
    {
      "issue_id": "UI-R2-NB-01",
      "status": "addressed",
      "resolution": "Defined Open Location behavior for missing files: open an existing file or nearest existing parent directory, otherwise disable with a reason tooltip."
    },
    {
      "issue_id": "UI-R2-NB-02",
      "status": "addressed",
      "resolution": "Limited narrow sidebar missing-artifact rows to five by default with disclosure or scroll behavior for larger sets."
    }
  ],
  "acceptance_criteria": [
    "Phase 0 samples at least 20 representative production agent executions and reports p50, p90, and p99 per-output and aggregate declared-output bytes before cap defaults are finalized.",
    "If Phase 0 p90 output size exceeds 10 MiB or p90 aggregate exceeds 64 MiB, P053 tunes defaults or adds workflow-level discovery.output_size_policy before Phase 1 implementation proceeds.",
    "Fresh ACP startup sends initialize without any recursive repository, worktree, or workspace-root scan.",
    "The proposal-053|p053 gate verifies no traversal under workspace_root or effective worktree root before initialize through deterministic instrumentation.",
    "DiscoveryFilesystem is the instrumentation seam for traversal, metadata, canonicalization, and file reads, and the gate fails on pre-initialize traversal under workspace or worktree roots.",
    "ExpectedOutputSpec is built for declared machine and companion outputs with labels, requiredness, reuse_policy, default max bytes, and authorized roots.",
    "OutputDiscoveryDecision includes the complete validation handoff fields and accepted decisions are converted to CapturedOutput by one builder.",
    "Per-output reuse policy uses a backward-compatible sibling output_policies map while existing outputs lists continue to compile unchanged in Rust and Swift schema mirrors.",
    "Duplicate declared output names or target paths within a multi-task stage fail compilation or produce deterministic wrong-owner diagnostics.",
    "ExpectedOutputSpec authorized_roots are derived per output role and reject wrong-run meta-root paths and original workspace paths for write-enabled worktree outputs.",
    "PrePromptExpectedOutputMetadata is captured only for expected output paths after session/new and before session/prompt, including content digests for regular files within cap.",
    "mtime and size alone never prove freshness; same-size rewrites and coarse-timestamp stale files are detected by digest tests.",
    "changed_files_manifest is generated by the engine after prompt completion and before exact-path acceptance when declared.",
    "Declared output validation and persistence consume accepted OutputDiscoveryDecision records, not raw target_path disk reads.",
    "Stale pre-existing required outputs settle as missing_required_outputs unless output_policies.<output_name>.reuse_policy is allow_unchanged_existing or the output is accepted from provider/control-plane current-invocation provenance.",
    "Exact expected paths enforce canonical root, symlink, regular-file, and validated per-output size checks.",
    "Ignored per-run meta artifacts are imported when they are exact expected paths or bounded supplemental evidence.",
    "Additional supplemental discovery is bounded to current run chainworks_meta_root with symlink protection and file/byte caps.",
    "Absent chainworks_meta_root skips bounded discovery and emits a diagnostic warning without falling back to workspace scanning.",
    "Legacy broad discovery is disabled by default, compiled into new-run snapshots only when workflow_opt_in is set, and available to frozen resumes only through an audited one-shot per-run override.",
    "Legacy broad discovery fallback enforces 5 second timeout, 1000 visited files, 1 MiB per file, 10 MiB total, cancellation, and truncation diagnostics.",
    "Audited per-run discovery overrides are created atomically with retry scheduling or attach only to pending not-yet-started retry attempts; they use legacy_discovery_overrides storage, command-journal transactionality, target_stage_execution_id scope, fail-closed stale handling, and consume-before-prompt semantics.",
    "Repo-backed changed-file manifests use GitManifestRunner after prompt completion with typed nonfatal statuses.",
    "Changed-file manifests are working-tree-current and normalize staged, unstaged, deleted, renamed, untracked, and conflicted porcelain statuses.",
    "Agent-authored changed_files_manifest conflicts are preserved at changed_files_manifest.agent.json while the control-plane manifest is canonical.",
    "Provider envelopes, exact-path files, and control-plane generated declared outputs enforce validated per-output max_bytes, validated aggregate accepted declared-output cap, and a 10 second exact-output acceptance timeout.",
    "Discovery diagnostics are persisted in agent_execution_discovery_diagnostics as a schema-versioned discovery_diagnostics_v1 payload plus compact indexed summaries, with ExecutionResult.discovery_diagnostics as a compatibility projection.",
    "Diagnostics, source-generation claims, runtime settlement, and active contract import either commit together or reconcile after restart to reconciliation_pending without validating required outputs.",
    "P037 idle watchdog does not classify post-prompt discovery phases as provider idle time.",
    "Missing required outputs include artifact labels, output ids, expected paths, statuses, and diagnostic reasons.",
    "RunDetailPanel renders compact Missing Artifacts rows at sidebar widths, limits default visible rows, defines Open Location behavior for missing files, and shows Discovery Mode in a diagnostics/runtime metadata grouping while promoting legacy/resume/override warning modes near run status.",
    "FailedStageEvidencePanel renders a dedicated Missing Required Outputs section.",
    "Stage Detail and Evidence panels render Startup Performance with Forge overhead separated from Provider latency.",
    "Discovery truncation renders a Capped warning indicator with the specific cap reason.",
    "ArtifactInspectorView has a Source Changes renderer for changed_files_manifest with raw JSON fallback.",
    "scripts/test-gate.sh and docs/reference/test-gates.md register and document proposal-053|p053."
  ]
}
