# Proposal 060: Lead-Driven Reviewer Routing and Expanded Reviewer Catalog

| Field | Value |
|---|---|
| Date | 2026-04-19 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md](017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md), [047-yaml-validation-and-definition-inspection-api.md](047-yaml-validation-and-definition-inspection-api.md) |
| Scope | Replace the fixed 4-reviewer fan-out in `state_4_proposal_reviewed` with lead-driven reviewer routing: the lead reads the proposal, fingerprints it, and selects 2–4 reviewers from an expanded catalog. |
| Goal | Every proposal gets the right specialist lenses (no more, no less), saving review budget on targeted changes and adding missing specialist coverage (reliability, performance, security, api-contract) that the current fixed set lacks. |

---

## 1. Context and Motivation

The current workflow `state_4_proposal_reviewed` fans out to a **fixed set of 4 reviewers**: product_owner, UX, UI, architect. Every proposal gets the same four lenses regardless of what it actually changes.

This has three concrete problems:

1. **Wasted review budget on mismatched proposals.** A pure UI polish proposal invokes `architect` + `product_owner` unnecessarily. A backend algorithm proposal invokes `ux` + `ui` unnecessarily. Each review costs provider tokens, wall-clock time, and adds noise to the aggregate summary.

2. **Missing specialist coverage.** The current catalog has no reviewer for reliability (retry/idempotency/shutdown), performance (latency/throughput/allocation), security (auth/trust boundary/unsafe), or API-contract (schema/protobuf/OpenAPI compatibility). Proposals touching these areas get only generic architectural review.

3. **No evidence-backed selection.** The 4-reviewer set is a historical default. There's no fingerprinting of the proposal to justify which lenses are appropriate.

The user already has codex skills — `proposal-review-router` and `proposal-implementation-audit` — that solve exactly this problem for single-operator reviews. They define:

- a reviewer registry with stacks, surfaces, risks, keywords, and repo signals;
- a fingerprint-first algorithm (build stack/surface/risk tags with evidence IDs before routing);
- a scoring function (`stack_match + surface_match + risk_match + repo_signal_match - overlap_penalty`);
- a target of 2–4 reviewers, hard cap 5.

This proposal brings the same discipline to the Chainworks lead orchestrator, so the multi-agent workflow gains selective, evidence-backed review.

---

## 2. Product Questions This Proposal Must Answer

1. Can the lead read a proposal and produce a fingerprint with evidence-backed stack/surface/risk tags?
2. Can the lead select 2–4 reviewers from an expanded catalog based on the fingerprint?
3. Can the workflow DSL express "route to these dynamically-selected agents" instead of a static fan-out list?
4. Can the system record which reviewers were selected, which were rejected, and why — for audit?
5. Does the expanded reviewer catalog cover reliability, performance, security, and API-contract lenses?
6. Does the selection result remain deterministic given the same proposal (reproducibility for testing and drift detection)?
7. Does the aggregator work with variable reviewer count (not assume exactly 4)?

---

## 3. Scope

This proposal includes:

- Expanded `agents.yaml` reviewer catalog: 4 existing (PO, UX, UI, architect) + up to 8 new specialist reviewers.
- A reviewer routing metadata block per reviewer agent (stacks, surfaces, risks, keywords, repo signals, pairing rules).
- A new reviewer-router agent (`proposal_review_router`) that runs before the fan-out and emits a `reviewer_selection_plan` artifact.
- Workflow DSL extension: dynamic fan-out — tasks list a pool of candidate agents filtered by a selection-plan artifact.
- Aggregator changes to handle variable reviewer count.
- Deterministic scoring algorithm with evidence IDs.
- Selection audit artifact (`reviewer_selection_plan`) recording selected agents, rejected alternatives, and fingerprint evidence.
- Tests covering: UI-only proposal → UI/UX routing, backend proposal → arch/reliability routing, security-sensitive proposal → security routing, fixed/legacy behavior via explicit flag.

This proposal does **not** include:

- Removing the existing 4 reviewers. They remain as candidates in the expanded catalog.
- Changing the review contract (`proposal_review_v1`). Same JSON output shape.
- LLM-based reviewer selection. Routing is deterministic (scoring + rules).
- Dynamic reviewer selection for other stages (e.g., `state_9_implementation_reviewed`). That's a follow-up.
- Cross-run reviewer learning (e.g., "these reviewers missed issues before"). Future steward work.

---

## 4. Problem Statement

### 4.1 Fixed fan-out wastes budget and adds noise

`state_4_proposal_reviewed` currently runs:

```yaml
tasks:
  - agent: proposal_reviewer_product_owner
    task: review_proposal_as_product_owner
  - agent: proposal_reviewer_ux
    task: review_proposal_as_ux_designer
  - agent: proposal_reviewer_ui
    task: review_proposal_as_ui_designer
  - agent: proposal_reviewer_architect
    task: review_proposal_as_architect
  - agent: lead_orchestrator
    task: aggregate_proposal_reviews
```

For a proposal that only changes retry logic in a Rust crate, running UX and UI reviewers produces filler output ("no UI impact detected"), consumes tokens, and dilutes the aggregated score.

### 4.2 Critical specialist lenses are missing

The catalog has no reviewer for:

- **Reliability**: retry, idempotency, cancellation, shutdown, backpressure, queue coordination.
- **Performance**: latency, throughput, allocation, lock contention, serialization hot path.
- **Security**: auth, secrets, unsafe, FFI, deserialization, public boundary, rate limits.
- **API contract**: protobuf/OpenAPI/GraphQL schema compatibility, version migration.
- **Observability/rollout**: feature flags, telemetry, migration, rollback.

Proposals like P032 (atomic transition settlement) or P045 (run recovery) touch reliability deeply but get only generic architectural review.

### 4.3 No evidence trail for reviewer selection

Today, the reviewer set is chosen at workflow-authoring time and frozen. There's no record of "why these four reviewers for this proposal." If a review misses an issue, there's no way to attribute the gap to a reviewer-selection mistake vs. a reviewer quality issue.

### 4.4 The DSL can't express dynamic fan-out

The workflow YAML `tasks:` list is static — every task runs. There's no way to say "run tasks from this pool, filtered by this plan artifact."

---

## 5. Core Product Behavior

### 5.1 Expanded reviewer catalog

Add the following reviewers to `agents.yaml`, each with routing metadata:

| Agent ID | Role | Stacks | Primary Surfaces | Primary Risks |
|---|---|---|---|---|
| `proposal_reviewer_product_owner` | Product | any | ux, rollout | user-trust |
| `proposal_reviewer_ux` | UX | apple-client, cross-stack | ux, navigation, accessibility | user-trust, privacy-sensitive |
| `proposal_reviewer_ui` | UI | ios, macos, apple-client | ui, navigation | platform-mismatch |
| `proposal_reviewer_architect` | Architecture (generic) | any | architecture, state-management | availability-sensitive |
| `proposal_reviewer_apple_architect` **(new)** | Apple architecture | ios, macos, apple-client | architecture, state-management, concurrency, persistence | platform-mismatch, data-loss |
| `proposal_reviewer_rust_architect` **(new)** | Rust architecture | rust-backend | architecture, concurrency, persistence, api-contract | availability-sensitive, backward-compatibility |
| `proposal_reviewer_reliability` **(new)** | Reliability | any | background-work, concurrency, persistence | idempotency, availability-sensitive, data-loss |
| `proposal_reviewer_performance` **(new)** | Performance | any | performance-hot-path, concurrency | latency-sensitive, availability-sensitive |
| `proposal_reviewer_security` **(new)** | Security | any | auth, security-boundary | security-sensitive, privacy-sensitive |
| `proposal_reviewer_api_contract` **(new)** | API contract | shared-api, cross-stack | api-contract, migration, persistence | backward-compatibility, multi-service-coordination |
| `proposal_reviewer_observability_rollout` **(new)** | Observability & rollout | any | telemetry, feature-flag, rollout, migration | operability-sensitive |

### 5.2 Reviewer routing metadata block

Each reviewer agent gains a `routing:` block in `agents.yaml`:

```yaml
- id: proposal_reviewer_reliability
  title: Proposal Reviewer / Reliability
  mode: proposal_review.reliability
  backend_profile: claude_reviewer_high
  permission_profile: RO_REVIEW
  skill_ref: proposal_review_triad
  skill_role: reliability_engineer
  inputs:
    - proposal_current
    - reviewer_scope_plan
  outputs:
    - proposal_review_reliability
  output_contract: proposal_review_v1
  requires_human_approval: false
  routing:
    stacks: [rust-backend, go-backend, microservice, cross-stack]
    surfaces: [background-work, concurrency, persistence]
    risks: [idempotency, availability-sensitive, operability-sensitive, data-loss]
    mandatory_when:
      any: [background-work, idempotency, availability-sensitive]
    strong_proposal_keywords:
      - retry
      - idempotency
      - timeout
      - deadline
      - cancellation
      - backpressure
      - queue
      - worker
      - recovery
      - resume
      - duplicate
      - shutdown
    strong_repo_files:
      - "**/*.rs"
      - "**/*.go"
    strong_repo_symbols:
      - WorkQueue
      - claim_next
      - tokio::select!
      - errgroup
      - goroutine
    close_alternatives: [proposal_reviewer_rust_architect, proposal_reviewer_architect]
    usually_pair_with: [proposal_reviewer_rust_architect, proposal_reviewer_observability_rollout]
  prompt: "Review the proposal as a reliability engineer. Focus on retry semantics, idempotency, cancellation, deadlines, backpressure, shutdown, and recovery paths. Mark blocking issues when the proposal risks data loss, duplicate work, or stuck states. Output the structured review contract."
```

### 5.3 New reviewer-router agent

Add a new agent `proposal_review_router` that runs **before** the fan-out:

```yaml
- id: proposal_review_router
  title: Proposal Review Router
  mode: proposal_review.routing
  backend_profile: claude_reviewer_high
  permission_profile: RO_REVIEW
  skill_ref: proposal_review_router_skill
  inputs:
    - proposal_current
    - idea_brief
    - reviewer_catalog_snapshot
  outputs:
    - reviewer_selection_plan
  output_contract: reviewer_selection_plan_v1
  requires_human_approval: false
  prompt: "You are the proposal review router. Read the proposal, build an evidence-backed fingerprint (stack_tags, surface_tags, risk_tags), and select 2-4 reviewers from the reviewer catalog that best match the fingerprint. Use the scoring algorithm from the proposal-review-router skill. Record selected reviewers, rejected close alternatives with rationale, and fingerprint evidence IDs. Target 3 reviewers, hard cap 5. Never select more than the cap."
```

The router produces a structured `reviewer_selection_plan` artifact:

```json
{
  "schema_version": 1,
  "proposal_md5": "abc123...",
  "fingerprint": {
    "stack_tags": [
      {"tag": "rust-backend", "evidence_ids": ["proposal:SS5.1", "repo:control-plane/crates/engine/Cargo.toml"]}
    ],
    "surface_tags": [
      {"tag": "background-work", "evidence_ids": ["proposal:SS4.2"]},
      {"tag": "persistence", "evidence_ids": ["proposal:SS6.2:migration"]}
    ],
    "risk_tags": [
      {"tag": "idempotency", "evidence_ids": ["proposal:SS5.1:claim_id"]},
      {"tag": "backward-compatibility", "evidence_ids": ["proposal:SS6"]}
    ]
  },
  "selected_reviewers": [
    {"agent_id": "proposal_reviewer_rust_architect", "score": 12, "rationale": "stack=rust-backend + surface=persistence + surface=concurrency"},
    {"agent_id": "proposal_reviewer_reliability", "score": 11, "rationale": "mandatory: risk=idempotency + surface=background-work"},
    {"agent_id": "proposal_reviewer_api_contract", "score": 7, "rationale": "surface=api-contract + risk=backward-compatibility"}
  ],
  "rejected_alternatives": [
    {"agent_id": "proposal_reviewer_ux", "reason": "no ux evidence; proposal is backend-internal"},
    {"agent_id": "proposal_reviewer_ui", "reason": "no ui evidence"},
    {"agent_id": "proposal_reviewer_product_owner", "reason": "no central KPI, experiment, or adoption metric evidence"},
    {"agent_id": "proposal_reviewer_architect", "reason": "superseded by proposal_reviewer_rust_architect (more specific stack match)"}
  ]
}
```

### 5.4 Scoring algorithm (deterministic)

For each candidate reviewer in the catalog:

```
score = 0
IF user_explicit_request → score += 5
FOR each matched stack_tag → score += 4
FOR each matched surface_tag → score += 3
FOR each matched risk_tag → score += 3
FOR each matched strong_keyword in proposal → score += 2
FOR each matched repo_file or repo_symbol in proposal evidence → score += 2
IF mandatory_when condition met → add reviewer regardless of score
IF another selected reviewer covers same risks with higher score → score -= 3 (overlap penalty)
```

Selection:

1. All reviewers meeting `mandatory_when` are auto-selected.
2. Remaining slots filled by top-scoring candidates above threshold (default: score ≥ 6).
3. Target: 3 reviewers. Cap: 5.
4. If no reviewer scores above threshold (rare) and no mandatory triggers: fall back to `[product_owner, architect]` as a minimal safety net.

### 5.5 Workflow DSL extension — dynamic fan-out

Extend workflow YAML with a new task form:

```yaml
state_4_proposal_reviewed:
  label: Proposal reviewed
  approval: not_required
  loop:
    type: revision
    max_iterations: 3
    counter: proposal_revision_counter
  tasks:
    # Step 1: Router agent selects reviewers
    - agent: proposal_review_router
      task: select_reviewers
      inputs:
        - proposal_current
        - idea_brief
      outputs:
        - reviewer_selection_plan

    # Step 2: Dynamic fan-out filtered by selection plan
    - dynamic_fan_out:
        selector_artifact: reviewer_selection_plan
        selector_field: selected_reviewers[*].agent_id
        candidate_agents:
          - proposal_reviewer_product_owner
          - proposal_reviewer_ux
          - proposal_reviewer_ui
          - proposal_reviewer_architect
          - proposal_reviewer_apple_architect
          - proposal_reviewer_rust_architect
          - proposal_reviewer_reliability
          - proposal_reviewer_performance
          - proposal_reviewer_security
          - proposal_reviewer_api_contract
          - proposal_reviewer_observability_rollout
        per_agent_task_template:
          task: review_proposal
          inputs:
            - proposal_current
            - reviewer_scope_plan
          output_contract: proposal_review_v1

    # Step 3: Aggregator handles variable reviewer count
    - agent: lead_orchestrator
      task: aggregate_proposal_reviews
      inputs:
        - proposal_current
        - reviewer_selection_plan
        - "*:proposal_review_*"  # Glob: all emitted review artifacts
      outputs:
        - proposal_review_summary
        - review_corpus_bundle
```

The `dynamic_fan_out` task type:

- Reads the selector artifact at runtime.
- Filters `candidate_agents` to only those whose `agent_id` appears in the selector field.
- Emits one task per selected agent, reusing the `per_agent_task_template`.
- Fails the stage if no reviewers are selected (the router must always produce at least 1).

### 5.6 Aggregator changes

`lead_orchestrator.aggregate_proposal_reviews` currently assumes 4 review artifacts. Update the aggregator to:

1. Read `reviewer_selection_plan` to know which reviewers were selected.
2. Read exactly those review artifacts (no more, no less).
3. Compute `average_score`, `min_individual_score`, `blocker_count` over the variable set.
4. Attach the selection plan to `review_corpus_bundle` for audit trail.

### 5.7 Selection determinism

Given the same proposal (MD5 match) and same reviewer catalog (content hash), the router must produce identical selection. This is required for:

- Proposal drift detection (if the same proposal gets re-reviewed, should pick same reviewers).
- Test reproducibility.
- Steward analysis (comparing selection quality across runs).

The router agent uses low temperature (0.0-0.1) and deterministic scoring. No randomness in selection tie-breaking (use `agent_id` alphabetical order as tiebreaker).

### 5.8 Improvements over the codex router skill

Beyond parity with the codex `proposal-review-router` skill, this proposal adds:

1. **Multi-run learning hook (future).** The `reviewer_selection_plan` artifact is recorded with each run. Steward analysis (P048) can later correlate "which reviewer sets catch which issue classes" and tune the scoring function.

2. **Workflow-level audit.** The selection plan is part of the run's frozen snapshot. A rerun against a different reviewer catalog will show a drift warning.

3. **Per-proposal reviewer override.** Operators can add a `reviewer_override:` hint to the idea brief:
   ```yaml
   reviewer_override:
     force_include: [proposal_reviewer_security]
     force_exclude: [proposal_reviewer_ui]
     reason: "This is a security-sensitive internal API; no UI surface."
   ```
   The router respects overrides, records them in the plan, and still reports rejected alternatives.

4. **Legacy compat flag.** `idea.review_mode: legacy_fixed` forces the old 4-reviewer behavior for regression testing. Default is `legacy_fixed: false` (dynamic routing).

---

## 6. Migration

### 6.1 YAML schema additions

Add to `agents.yaml` schema:
- Optional `routing:` block on each reviewer agent.
- New agent entries for the 7 new reviewers.
- New agent entry for `proposal_review_router`.

Add to `workflow.yaml` schema:
- New task form `dynamic_fan_out` with `selector_artifact`, `selector_field`, `candidate_agents`, `per_agent_task_template`.

### 6.2 Control-plane work

- Workflow compiler (`workflow/src/compiler.rs`): parse `dynamic_fan_out` tasks; emit a RunPlan marker so the orchestrator knows to resolve reviewers at runtime.
- Orchestrator (`engine/src/orchestrator.rs`): when a `dynamic_fan_out` task is dispatched, read the selector artifact, filter candidates, and enqueue per-agent work items.
- Artifact contract registry: register `reviewer_selection_plan_v1` schema.

### 6.3 Swift app work

Minimal. The Swift app renders whichever artifacts the workflow produces. The new `reviewer_selection_plan` artifact renders as JSON via `ArtifactContentRenderer`. No new UI required.

### 6.4 Existing proposals and workflows

- `state_4_proposal_reviewed` in `workflow.yaml` changes from static fan-out to the new form.
- Existing test workflows (`examples/workflows/proposal-loop-live.yaml`) use the new structure or opt into `legacy_fixed` compat.

### 6.5 Rollout

1. Ship router agent + expanded catalog + `dynamic_fan_out` support without removing the old static form (both supported).
2. One canonical workflow (`workflow.yaml`) migrates to dynamic routing. `examples/workflows/proposal-loop-live.yaml` stays on legacy for compat testing.
3. Steward analysis (P048) tracks reviewer quality for 2 weeks.
4. Remove `legacy_fixed` compat flag in a follow-up proposal.

---

## 7. Verification

### 7.1 Router output contract

- Router emits `reviewer_selection_plan` matching schema v1.
- Plan includes `proposal_md5`, `fingerprint`, `selected_reviewers` (1–5), `rejected_alternatives`.
- Every selected reviewer has a `rationale` citing matched tags.
- Every rejected alternative has a `reason`.

### 7.2 Selection correctness

Test scenarios:

| Proposal type | Expected selected reviewers |
|---|---|
| Pure UI polish (Swift/macOS) | `macos_ui_reviewer`, `apple_ux_reviewer`, (no architect) |
| Backend retry/idempotency (Rust) | `rust_architect`, `reliability`, (optional: `observability_rollout`) |
| Auth/secrets change (any stack) | `security`, `api_contract`, one architect |
| Cross-stack API + UI | `ios_ui` or `macos_ui`, one backend architect, `api_contract` |
| Proposal with explicit `force_include: [security]` | Includes `security` regardless of score |

### 7.3 Determinism

- Same proposal + catalog → identical `selected_reviewers` list (order and composition).
- MD5 of `reviewer_selection_plan` is stable across repeated runs (excluding timestamps).

### 7.4 Aggregator handles variable count

- Aggregator processes 2 reviews correctly (computes avg, min, blockers).
- Aggregator processes 5 reviews correctly.
- Aggregator fails cleanly if router selected 0 reviewers.

### 7.5 Cap enforcement

- If 6 reviewers score above threshold, only top 5 are selected (matching codex skill's hard cap).
- `mandatory_when` reviewers are never dropped by cap — they displace lowest-scoring optional selections.

### 7.6 Legacy compat

- Setting `idea.review_mode: legacy_fixed` invokes all 4 original reviewers; skips router.
- Plan artifact still emitted (for audit) but marked `mode: legacy_fixed`.

### 7.7 Workflow drift detection

- If reviewer catalog changes between run compile and resume, drift is detected via catalog snapshot hash (existing P049 mechanism).
- Run either continues with frozen catalog (safe) or the operator is prompted for re-selection.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Router picks wrong reviewers and misses issues | Medium | Rejected alternatives list lets the operator override. Steward analysis tracks missed-issue correlation over time. `mandatory_when` rules prevent the router from dropping critical lenses. |
| Scoring function ossifies into the wrong priorities | Medium | Scoring weights are in `agents.yaml` (not hardcoded). Can be tuned per project. Steward analysis can propose weight changes (P048 experiment track). |
| Adding 7 new reviewer agents bloats the catalog | Low | Reviewers are defined by text prompts; no runtime cost unless selected. Grouping in UI (proposal 036) handles catalog size. |
| Dynamic fan-out breaks existing workflow tooling | Medium | Existing static `tasks:` form stays supported. New form is additive. Workflow validation (P047) explicitly validates `dynamic_fan_out` structure. |
| Router adds latency to every proposal review | Low | Router is one fast LLM call (~5-10s). Saves far more time by cutting unnecessary reviewers. |
| Router agent itself fails or times out | Medium | Fallback to `[product_owner, architect]` minimal safety net. Router failure is surfaced as a blocker the operator can resolve via `agents.retry` (P045). |
| Fingerprint tags drift out of sync between router and reviewer metadata | Medium | Router reads reviewer catalog at runtime; both reference the same `routing:` metadata. Catalog validation (P047) checks tag consistency. |
| Deterministic scoring produces same mistake repeatedly | Low | Override flags (`force_include`, `force_exclude`) give operator escape hatch. Steward analysis detects repeated failures and can propose scoring changes. |
| Legacy workflows silently degrade when catalog metadata is missing | Low | Validation (P047) rejects router config that references reviewers without `routing:` block. Fail-closed startup. |
| Override flag can be misused to pass reviews | Medium | `force_exclude` decisions are logged with the operator-supplied `reason`. Steward flags proposals with exclusions for periodic audit. |

---

## 9. Example: Backend-Only Proposal

Proposal: "Add `runs.resume` MCP tool for on-demand run resume" (like P045).

**Router fingerprint:**

```json
{
  "stack_tags": [{"tag": "rust-backend", "evidence_ids": ["proposal:SS5", "repo:control-plane/crates/engine/Cargo.toml"]}],
  "surface_tags": [
    {"tag": "api-contract", "evidence_ids": ["proposal:SS5.1:runs.resume JSON schema"]},
    {"tag": "persistence", "evidence_ids": ["proposal:SS5.1:runs.transition_cursor_json"]},
    {"tag": "background-work", "evidence_ids": ["proposal:SS5.1:AdvanceRun work item"]}
  ],
  "risk_tags": [
    {"tag": "idempotency", "evidence_ids": ["proposal:SS5.1:resume_claim_id"]},
    {"tag": "availability-sensitive", "evidence_ids": ["proposal:SS4.1:interrupted run"]}
  ]
}
```

**Selected reviewers:**

1. `proposal_reviewer_rust_architect` — score 12 (stack=rust-backend + surface=persistence + surface=api-contract)
2. `proposal_reviewer_reliability` — score 11 (mandatory: surface=background-work + risk=idempotency)
3. `proposal_reviewer_api_contract` — score 8 (surface=api-contract + risk=backward-compatibility via new tool)

**Rejected alternatives:**

- `proposal_reviewer_ui` — no UI evidence
- `proposal_reviewer_ux` — no UX evidence
- `proposal_reviewer_product_owner` — internal tool, no user-facing KPI
- `proposal_reviewer_architect` (generic) — superseded by `rust_architect`
- `proposal_reviewer_security` — score 4, below threshold (no auth/unsafe evidence)
- `proposal_reviewer_performance` — score 3, below threshold (no hot-path evidence)

**Result:** Review runs 3 specialists instead of the current 4 generalists. Budget saved: ~25%. Coverage improved: reliability lens added, irrelevant UX/UI lenses removed.

---

## 10. Example: Pure UI Polish Proposal

Proposal: "Rework Live Timeline card rendering with tool merging and 2s batched updates" (like part of P036).

**Router fingerprint:**

```json
{
  "stack_tags": [{"tag": "macos", "evidence_ids": ["repo:Chainworks Forge/Views/RunTimelineInspectorView.swift"]}],
  "surface_tags": [{"tag": "ui", "evidence_ids": ["proposal:SS5.11:card rendering"]}],
  "risk_tags": [{"tag": "platform-mismatch", "evidence_ids": ["proposal:SS5.11.4:animation"]}]
}
```

**Selected reviewers:**

1. `proposal_reviewer_macos_ui_reviewer` — score 10 (stack=macos + surface=ui)
2. `proposal_reviewer_apple_ux_reviewer` — score 7 (surface=ui + pair-with rule)

**Rejected alternatives:**

- `proposal_reviewer_architect` — score 3, overlap penalty (apple_arch covers same)
- `proposal_reviewer_product_owner` — no user KPI evidence
- All backend reviewers — no Rust/Go evidence

**Result:** 2 focused UI reviewers instead of 4 generalists. 50% budget reduction, higher review quality.
