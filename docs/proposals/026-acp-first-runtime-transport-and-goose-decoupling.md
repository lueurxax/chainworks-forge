# Proposal 026: ACP-First Runtime Transport And Goose Decoupling

| Field | Value |
|---|---|
| Date | 2026-04-05 |
| Status | Draft |
| Author | Codex |
| Depends on | [015-skill-resolution-and-runtime-injection.md](015-skill-resolution-and-runtime-injection.md), [025-per-agent-mcp-policy-and-runtime-validation.md](025-per-agent-mcp-policy-and-runtime-validation.md), [026-acp-runtime-plan-additive-profiles.md](026-acp-runtime-plan-additive-profiles.md), [../reference/goose-server-transport.md](../reference/goose-server-transport.md), [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../research/goose_swiftui_agent_architecture_research.md](../research/goose_swiftui_agent_architecture_research.md), [../evidence/goose-acp-compatibility-probe.md](../evidence/goose-acp-compatibility-probe.md), [../evidence/opencode-acp-research.md](../evidence/opencode-acp-research.md), [../evidence/gemini-cli-acp-research.md](../evidence/gemini-cli-acp-research.md), [../evidence/claude-agent-acp-research.md](../evidence/claude-agent-acp-research.md), [../evidence/junie-cli-acp-research.md](../evidence/junie-cli-acp-research.md), [../evidence/cline-cli-acp-research.md](../evidence/cline-cli-acp-research.md), [../evidence/auggie-cli-acp-research.md](../evidence/auggie-cli-acp-research.md), [../evidence/acp-runtime-candidate-comparison.md](../evidence/acp-runtime-candidate-comparison.md) |
| Scope | Introduce an ACP-shaped canonical runtime vocabulary, add catalog-backed runtime profiles and first-wave ACP runtimes, and preserve Goose as the current default runtime during the first proposal wave. |
| Goal | Make `Chainworks Forge` transport-neutral in its core runtime vocabulary while adding ACP-backed runtime options without regressing current execution truth, report truth, recovery truth, or MCP policy truth. |

---

## 1. Context and Motivation

`Chainworks Forge` currently has a transport seam, but the seam is still Goose-shaped.

Core execution code still assumes Goose concepts and Goose operational behavior:

- session creation through Goose server endpoints
- provider binding through Goose-specific update calls
- prompt submission through Goose-specific reply semantics
- runtime state reads through Goose session inspection
- MCP realization through Goose extension mutation APIs
- operational readiness through `goosed` process management and `/status` probing

That creates a structural lock-in:

1. The runtime transport contract in core code inherits Goose endpoint semantics.
2. MCP policy is conceptually generic, but its runtime realization path is still Goose-specific.
3. Recovery and execution truth are increasingly canonical, yet the transport beneath them is still framed around Goose behavior.
4. Adding ACP later as “just another transport” would preserve Goose as the hidden system model rather than truly decoupling the product.

The earlier framing of `P026` pushed too hard toward a fast cutover.
The current research basis does **not** support a destructive first wave:

- Goose still carries current production/default behavior
- the best ACP candidates are strong, but not uniform
- operator-grade parity exists only for part of the field
- some ACP runtimes remain better as second-wave targets

So this proposal intentionally does **not** create a new Forge-owned universal execution protocol, and it also does **not** require a first-wave Goose replacement.
Instead, it adopts **ACP as the canonical transport vocabulary in core** while keeping Forge ownership only where product semantics actually belong:

- workflow orchestration
- run/stage/agent execution truth
- preflight policy
- MCP requested/predicted/actual truth
- recovery and report truth
- operator UX

Goose then becomes, in Proposal 026:

- the current default runtime path,
- an ACP-shaped compatibility adapter for the new seam,
- and optionally a managed local runtime provider for operator convenience,

but no longer the canonical shape of the system itself.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can core orchestration execute runs without importing Goose endpoint semantics?
2. Can ACP become the canonical execution vocabulary without forcing an immediate hard cutover away from Goose?
3. Can Goose remain functional as the default runtime while no longer defining core transport semantics?
4. Can runtime selection move into catalog/runtime-profile truth instead of ad hoc per-agent transport fields?
5. Can MCP policy remain product-owned while its realization becomes runtime-specific?
6. Can first-wave ACP support land additively for a constrained set of runtimes without regressing report, recovery, or execution truth?

---

## 3. Scope

This proposal includes:

- an ACP-shaped canonical runtime vocabulary in core execution code
- replacement of Goose-shaped transport abstractions in core execution code
- a Goose compatibility adapter that implements the canonical runtime contract
- additive `runtime_profiles` and backend-profile runtime binding
- first-wave ACP runtime selection for a constrained candidate set
- explicit separation between product semantics and vendor-specific operational mechanics
- ACP-shaped MCP intent in core runtime code
- run-snapshot freezing of runtime selection
- capability-gated rollout criteria for ACP runtimes

This proposal does **not** include:

- inventing a Forge-owned replacement for ACP
- removing Goose as the default runtime in the first wave
- a forced global cutover to ACP runtimes
- per-agent raw ACP command selection as the canonical model
- preserving every current Goose optimization during the bridge
- guaranteeing backward-perfect behavior for every diagnostic path during migration
- treating every ACP runtime as equally capable

---

## 4. Design Principles

1. ACP is canonical at the runtime-vocabulary layer.
2. Forge remains canonical at the product-semantics layer.
3. Proposal 026 is additive, not destructive.
4. Goose may remain the default runtime path in the first wave.
5. Core runtime code must stop importing Goose endpoint semantics even if Goose remains operational.
6. Runtime selection belongs to catalog/runtime-profile truth, not ad hoc agent-local transport fields.
7. MCP intent is transport-neutral in core and runtime-specific only in realization.
8. Capability classes gate rollout; ACP runtimes are not assumed equal.

---

## 5. Architecture

### 5.1 ACP becomes the canonical runtime vocabulary

Core runtime execution should be expressed in ACP-shaped terms:

- create or load a session
- submit prompt/context
- receive normalized stream events
- cancel/close
- update session when supported
- inspect runtime/session state when supported

Core code should no longer reason in terms of:

- `POST /agent/start`
- `POST /agent/update_provider`
- `POST /reply`
- `POST /agent/add_extension`
- `POST /agent/remove_extension`
- `GET /sessions/{id}`
- `DELETE /sessions/{id}`

Those become implementation details of specific adapters, including Goose.

### 5.2 Runtime profiles become the selection unit

Proposal 026 should introduce catalog-owned `runtime_profiles`.

`runtime_profile` is the place where Forge records **repo-owned runtime intent**, not machine-local launch authority.

`runtime_profile` should record runtime execution shape such as:

- transport kind
- adapter family
- capability class
- required capabilities
- compatibility expectations relevant to run truth and operator surfaces

This is the right authority boundary because runtime choice is not just “which model to use.”
It is a bundle of:

- transport
- session semantics
- mutation expectations
- MCP realization path
- operator-surface viability

`runtime_profile` should **not** become the place where Forge stores machine-local runtime authority such as:

- concrete launch command for the current machine
- local managed-runtime process settings
- auth material
- bootstrap-specific filesystem paths
- machine-local provider/runtime installation details

Those remain owned by the existing machine-local provider/platform/bootstrap layer.
So the split is:

- repo catalog owns:
  - runtime profile identity
  - adapter family
  - capability class
  - required capabilities
  - compatibility expectations
- machine-local platform/bootstrap owns:
  - whether a runtime is installed on this machine
  - how it is launched on this machine
  - auth/bootstrap details
  - local health/readiness checks

### 5.3 Backend profiles bind to runtime profiles

Agents should continue selecting `backend_profile`.
`backend_profile` should then freeze a single runtime bundle:

- provider
- model
- effort
- structured-output posture
- runtime profile

This avoids drifting authority between:

- agent role
- provider/model
- transport
- MCP capability expectations
- run/report truth

### 5.4 Goose remains the default runtime in Proposal 026

Proposal 026 should preserve Goose as the current default runtime path.

That means:

- existing Goose-backed backend profiles continue to work
- no product-wide cutover is required
- ACP support is introduced as additive runtime enablement

The first proposal wave is successful if core code stops being Goose-shaped while Goose still works.

### 5.5 Forge-owned semantics stay above ACP

Forge still owns:

- workflow orchestration
- run model
- stage execution model
- `AgentExecution` truth
- retry and resume policy
- report generation
- approval and recovery semantics
- preflight policy
- frozen run snapshot
- requested/predicted/actual MCP truth

ACP does **not** define:

- what counts as a blocked run
- what counts as a retryable failure
- what report fields are canonical
- how improvement loops work
- how approvals are modeled

Those stay in Forge.

### 5.6 Capability classes gate ACP rollout

Proposal 026 should not treat all ACP runtimes as equal.
It should classify them by evidence-backed capability level.

Suggested classes:

- `lifecycle_capable`
  Requires:
  - `initialize`
  - `session/new`
  - `session/prompt`
  - cancel/close
- `control_capable`
  Requires:
  - trustworthy load/state inspection
  - runtime model/mode mutation truth good enough for reports
  - session-scoped MCP realization
  - requested-to-effective runtime capability checks
- `operator_grade`
  Requires:
  - useful `session/update`
  - permission callbacks or equally strong permission truth
  - tool-call visibility
  - replay/history good enough for reports and recovery
  - stable runtime truth for provider/model/session behavior

Current rollout posture:

- first-wave targets in Proposal 026:
  - Claude Agent ACP
  - Gemini CLI ACP
- second-wave candidates for follow-up proposal:
  - Auggie CLI ACP
  - Junie CLI ACP

### 5.7 Adapter-specific operations remain below ACP

The following may remain vendor-specific even after cutover:

- managed local `goosed` bootstrapping
- Goose `/status` probing
- Goose auth and localhost TLS quirks
- Goose provider identifier normalization
- Goose extension mutation calls
- Goose config-file and registry parsing
- Goose-specific health and diagnostics

These are runtime operations, not core transport semantics.

---

## 6. Canonical Ownership Boundary

The system should be split into three layers.

### 6.1 ACP core

Owns the canonical execution transport vocabulary:

- session lifecycle
- prompt/context submission
- stream events
- cancellation
- capability discovery
- session state/load/update where available

### 6.2 Forge-owned execution semantics

Owns the product truth:

- run, stage, and agent execution semantics
- receipts, reports, and comparison surfaces
- retry/resume/recovery rules
- run-start freezing
- MCP policy intent and validation
- operator-facing diagnostics

### 6.3 Adapter-specific mechanics

Own vendor/runtime mechanics:

- process launch
- runtime health checks
- vendor auth
- vendor-side MCP mutation mechanics
- vendor registry discovery
- vendor-specific transient diagnostics

This split is the heart of the proposal.
It allows Forge to stop being Goose-shaped without forcing Forge to invent a new protocol.

---

## 7. Current Goose Method Inventory And Migration Matrix

### 7.1 Core transport methods

| Current method / behavior | Current owner | ACP analogue | Migration action |
|---|---|---|---|
| `createSession(request:)` | `GooseTransportProtocol` / Goose server | ACP session creation/setup | Migrate to canonical ACP transport |
| `submitPrompt(sessionID:prompt:)` | Goose reply path | ACP prompt/message submission | Migrate to canonical ACP transport |
| `closeSession(sessionID:)` | Goose session delete | ACP cancel/close | Migrate to canonical ACP transport |
| `readSessionRuntimeState(sessionID:)` | Goose session inspection | ACP session state/load/capabilities where supported | Migrate to canonical ACP transport, with capability fallback |
| stream event normalization | Goose SSE mapper | ACP event normalization | Migrate to canonical ACP transport |

### 7.2 Goose-specific compatibility behaviors

| Current Goose behavior | Why it exists | Destination |
|---|---|---|
| `POST /agent/start` then `POST /agent/update_provider` | Goose startup shape | Goose adapter only |
| provider ID normalization for Goose | vendor naming mismatch | Goose adapter only |
| Goose-specific request body assembly | vendor API shape | Goose adapter only |
| Goose session extension add/remove | vendor-side MCP realization | Goose adapter only |
| Goose session extension enumeration | vendor runtime introspection | Goose adapter only |
| Goose SSE event mapping | vendor event shape | Goose adapter only |

### 7.3 Vendor-specific operational mechanics

| Current Goose behavior | Destination |
|---|---|
| local `goosed` process launch / stop | optional Goose managed-runtime layer |
| `/status` health probe | optional Goose managed-runtime layer |
| localhost TLS bypass / trust delegate | Goose managed-runtime layer |
| `X-Secret-Key` handling | Goose managed-runtime layer |
| machine-local Goose config parsing | Goose adapter / diagnostics layer |

The important point is not whether Goose still exists.
The important point is that none of these items remain the canonical transport truth of the app.

---

## 8. MCP Policy Under ACP

Proposal 025 already makes MCP policy explicit and product-owned.
Proposal 026 changes **where** that policy is realized.

### 8.1 Core MCP truth becomes ACP-shaped intent

In core runtime code, MCP should no longer be modeled as Goose extension IDs.

Core should instead own:

- requested MCP servers or capabilities
- required vs optional semantics
- fallback policy
- predicted compatibility truth
- actual runtime-settled truth

That truth must be split across concrete owners:

- catalog / backend profile selection owns:
  - declared runtime intent
  - requested runtime profile identity
- preflight owns:
  - predicted effective runtime capability truth for this machine
  - predicted MCP/runtime compatibility assessment
  - it does **not** own durable runtime settlement
- `RunStartSnapshot` owns:
  - frozen requested runtime binding for the run
  - frozen backend-profile/runtime-profile identity used to start execution
- `AgentExecution` owns:
  - actual runtime settlement for a concrete execution attempt
  - actual provider/model/runtime facts that materially occurred
  - actual MCP requested-to-effective settlement for that attempt
- shell-owned reports and recovery readers own:
  - reading and presenting persisted Forge truth
  - they do **not** reconstruct truth from adapter heuristics

This is the required requested-vs-predicted-vs-actual split.

### 8.2 Goose becomes one realization strategy

If Goose stays during the bridge, the Goose adapter translates ACP-shaped MCP intent into:

- Goose extension IDs
- Goose add/remove calls
- Goose enabled-extension reads

This means Goose naming is no longer canonical in core code.
It becomes just one adapter-specific mapping.

### 8.3 ACP-native runtime replaces the realization layer

When an ACP-native runtime is added, it should consume the same Forge-owned MCP intent directly, without changing:

- preflight authority
- run snapshot shape
- `AgentExecution` truth model
- report and comparison readers

---

## 9. Migration Strategy

This proposal intentionally favors a short bridge over a long coexistence period.

### 9.1 Phase 1: ACP vocabulary becomes canonical in core

Replace Goose-shaped transport interfaces in core runtime code with ACP-shaped ones.

Required outcome:

- orchestration and executor code above transport stop depending on Goose endpoint semantics

### 9.2 Phase 2: Goose becomes the default compatibility adapter

Implement the canonical runtime contract on top of the existing Goose backend and keep it as the default runtime profile.

Allowed bridge degradations are scoped **only** to non-default early ACP lanes and temporary capability-gated adapters.
They are **not** allowed to weaken the preserved default Goose path.

Allowed temporary degradations outside the default Goose path:

- weaker session reuse
- reduced live diagnostics fidelity
- temporary MCP feature narrowing
- rougher resume/load behavior for some edge cases

Required preservation for the default Goose path:

- current default Goose-backed runs remain functional
- default Goose report/recovery behavior may not be intentionally weakened by bridge concessions
- any conservative downgrade must be confined to newly introduced ACP-backed runtime lanes or explicitly capability-gated non-default paths

### 9.3 Phase 3: Runtime profiles and first-wave ACP runtimes land

Add additive ACP runtime selection through runtime profiles and backend profiles.

Minimum first-wave runtimes:

- Claude Agent ACP
- Gemini CLI ACP

Required proof focus:

- run snapshot freezes selected runtime profile
- canonical execution truth remains Forge-owned
- reports and recovery surfaces remain grounded in persisted Forge truth through the current shell-owned readers:
  - `RunReportView`
  - `RunComparisonView`
  - `RecoverySheet`
  - `BlockedRunRecoveryView`
- at least one canonical proposal loop and one implementation path complete on ACP-backed runtimes

Proposal 026 does **not** authorize a parallel runtime-diagnostics truth lane outside those persisted-truth readers.

### 9.4 Phase 4: Follow-up runtime expansion

After the first-wave seam is proven:

- add second-wave runtimes through a follow-up proposal
- likely next candidates:
  - Auggie CLI ACP
  - Junie CLI ACP
- defer any Goose-default removal or broader cutover decision until after that evidence exists

---

## 10. Rollout Constraints

Proposal 026 is additive and may not regress canonical truth just to accelerate migration.

### 10.1 Acceptable temporary regressions

- some existing live diagnostics temporarily degrade
- some report fields temporarily become less detailed
- some recovery paths temporarily become conservative
- some legacy runs may not resume perfectly

### 10.2 Unacceptable regressions

- two canonical transport contracts in core
- Goose endpoint semantics leaking back into orchestration
- MCP truth becoming vendor-named again in core
- report truth being reconstructed from adapter heuristics instead of persisted execution truth
- Goose becoming broken as the default runtime during the first wave
- runtime selection drifting outside catalog/backend-profile truth
- shipping ACP support that downgrades current execution truth just to claim cutover speed

---

## 11. Success Criteria

Proposal 026 is complete only when all of the following are true:

1. Core orchestration code does not import Goose transport types or Goose endpoint semantics.
2. The canonical runtime abstraction in core is ACP-shaped.
3. MCP intent in core is not expressed in Goose extension IDs.
4. `RunStartSnapshot` and `AgentExecution` persist transport-neutral truth.
5. Runtime selection exists through catalog/runtime-profile and backend-profile truth.
6. Goose still works as the default runtime path after the seam extraction.
7. At least two ACP runtimes can be selected through backend profiles:
   - Claude Agent ACP
   - Gemini CLI ACP
8. ACP-backed runs complete at least one canonical proposal loop and one implementation path without downgrading canonical execution truth, report truth, or MCP truth.

---

## 12. Risks And Open Questions

### 12.0 Current candidate posture

Current ranking from the research round in
[../evidence/acp-runtime-candidate-comparison.md](../evidence/acp-runtime-candidate-comparison.md):

- Claude Agent ACP
- Gemini CLI ACP
- Auggie CLI ACP
- Junie CLI ACP
- Cline CLI ACP
- OpenCode ACP
- Goose ACP

Implications for Proposal 026:

- Claude and Gemini are strong enough for the first additive wave
- Auggie and Junie are strong enough to justify a second-wave proposal
- OpenCode remains strategically interesting, but not the right first target for operator-grade parity
- Goose ACP remains a bridge/reference, not a long-term target runtime

### 12.1 ACP coverage mismatch

ACP may not expose every convenience that current Goose operations provide.
The system must therefore distinguish:

- canonical transport features
- optional runtime capabilities
- adapter-specific conveniences

Core code should degrade through capability checks rather than reintroducing Goose semantics.

### 12.2 Resume/load fidelity

Some existing resume and session inspection behavior may currently depend on Goose-specific reads.
The bridge must decide whether to:

- degrade gracefully,
- provide ACP capability-based fallbacks,
- or temporarily fail closed.

### 12.3 MCP realization differences

Different ACP runtimes may represent MCP/server attachment differently.
Forge should standardize only the requested/predicted/actual policy truth, not a vendor-shaped mutation sequence.

### 12.4 Managed local runtime remains a partial lock-in

If the product still ships a managed local Goose runtime, that remains a convenience lock-in at the operational layer.
This is acceptable only if:

- core execution no longer depends on it,
- runtime selection truth is no longer Goose-shaped,
- and the operator can eventually replace it with another ACP-capable runtime.

---

## 13. Recommendation

Adopt an **ACP-first** core runtime vocabulary now, but make Proposal 026 an additive runtime-enablement slice rather than a hard cutover.

Concretely:

- keep Goose alive and functional as the current default runtime,
- extract Goose-shaped semantics out of core,
- introduce runtime profiles and backend-profile runtime binding,
- ship first-wave ACP support for:
  - Claude Agent ACP
  - Gemini CLI ACP
- preserve execution truth, report truth, recovery truth, and MCP truth as Forge-owned surfaces,
- defer Auggie and Junie to the next expansion proposal once the first-wave seam is proven.

This path is lower-risk, more honest to the current evidence, and avoids two bad outcomes:

1. a premature cutover away from Goose,
2. a fake runtime-neutrality claim that still hides Goose semantics in core.
