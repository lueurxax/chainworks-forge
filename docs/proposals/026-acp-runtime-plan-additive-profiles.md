# ACP Runtime Plan: Additive Runtime Profiles, First-Wave Candidates, and Goose Preservation

## Status

Decision note captured after the current ACP runtime research round.

This note records the currently agreed direction for the ACP migration path in Chainworks Forge.

---

## 1. Core decision

Chainworks Forge should **not** do a hard cutover away from Goose in Proposal 026.

Proposal 026 should instead become an **additive runtime-enablement slice**:

- ACP-shaped transport and lifecycle semantics become the canonical direction in core architecture.
- Goose remains intact as the current default runtime path.
- The system gains the ability to select an ACP-backed runtime through catalog/runtime configuration.
- No existing live functionality is intentionally broken just to “prove” ACP migration speed.

The immediate goal is not “remove Goose.”
The immediate goal is:

> add ACP-backed runtime options without regressing current execution truth, recovery truth, report truth, or MCP policy truth.

---

## 2. Candidate ranking from the current research round

Based on the current comparison snapshot, the practical ranking is:

1. **Claude Agent ACP**
2. **Gemini CLI ACP**
3. **Auggie CLI ACP**
4. **Junie CLI ACP**
5. **Cline CLI ACP**
6. **OpenCode ACP**
7. **Goose ACP**

This ranking is evidence-driven, not aspirational.

### Why Claude Agent ACP currently leads

Claude Agent ACP is currently the strongest near-term candidate for Forge because the live probes already show:

- real `session/update` streaming
- real `usage_update`
- persisted `set_model` and `set_mode` truth through `loadSession`
- real permission callbacks
- rich edit diff lifecycle
- real MCP attach and tool execution

This makes it the strongest current candidate for **operator-grade runtime parity**.

### Why Gemini CLI ACP remains critical

Gemini CLI ACP is the strongest current **native-style ACP runtime** candidate because the live probes show:

- real `session/update`
- real `session/load` replay
- real tool-call visibility
- real permission callbacks
- real ACP file-read proof
- real MCP attach and tool execution
- useful quota/model usage telemetry

Its current weakness is weaker persisted session-config truth after mutation compared with Claude.

### Why Auggie and Junie are still worth keeping

Auggie and Junie are now both real second-wave candidates rather than speculative watchlist entries.

- **Auggie** proved authenticated execution, replay via `session/load`, permission callbacks, edit settlement, and real MCP tool execution.
- **Junie** proved authenticated execution, thought/message streaming, permission callbacks, MCP execution, and real edit flows.

They are strong enough to justify a second proposal wave after the first ACP lane lands.

### Why OpenCode is not the first ACP target anymore

OpenCode still has the strongest broader product/runtime story for:

- agents
- skills
- MCP
- permissions
- model/mode mutation truth

But its ACP layer still lacks the observability Forge needs for live operator surfaces:

- zero meaningful `session/update` in probes
- no ACP permission callback proof
- weak transcript/history replay
- no proven ACP-level MCP tool-use visibility

This makes OpenCode strategically interesting, but not the best immediate target for Proposal 026.

### Why Goose ACP is not a migration target

Goose ACP remains useful as:

- a bridge reference
- a compatibility probe
- a signal that ACP-first architecture still makes sense

But it is not currently strong enough to become the target runtime because it still lacks:

- useful `session/update` streaming
- trustworthy persisted model truth
- safe MCP attach behavior
- operator-grade replay/observability

---

## 3. Proposal split

### Proposal 026 (first wave)

Proposal 026 should support exactly two ACP runtimes:

1. **Claude Agent ACP**
2. **Gemini CLI ACP**

Reasoning:

- Claude gives the best near-term chance at operator-grade parity.
- Gemini gives the best native ACP-first path.
- Together they test both the pragmatic and strategic sides of the migration.

### Next proposal (second wave)

The next ACP runtime proposal should add:

3. **Auggie CLI ACP**
4. **Junie CLI ACP**

These should be treated as second-wave adapters once the core ACP runtime seam is working and the first-wave capability model is proven.

---

## 4. What Proposal 026 should become

Proposal 026 should no longer be framed as:

- a fast cutover away from Goose,
- or a near-immediate runtime replacement.

It should be reframed as:

> **ACP-capable additive runtime transport and runtime-profile introduction**

### Practical meaning

Proposal 026 should do all of the following:

- introduce an ACP-shaped runtime seam in core code
- keep Goose working as the existing default runtime
- add runtime selection through catalog/runtime configuration
- allow backend profiles to bind to different runtime profiles
- freeze runtime selection into the run snapshot
- preserve current report/recovery/execution-truth semantics
- avoid any product-wide cutover requirement in this proposal

---

## 5. Runtime-selection model

### Do not bind ACP directly on each agent

A simple per-agent field like:

```yaml
acp_server: claude-agent-acp
```

is too weak and will create authority drift between:

- agent role
- backend profile
- provider/model
- runtime transport
- MCP capability
- report/recovery truth

### Preferred model

#### 5.1 Add `runtime_profiles` to the catalog

`runtime_profile` should be understood as **repo-owned runtime intent**, not
machine-local launch/bootstrap authority.

So catalog-owned `runtime_profiles` should carry:

- runtime profile identity
- transport family / adapter family
- capability class
- required capabilities
- compatibility expectations relevant to run truth and operator surfaces

They should **not** carry machine-local launch authority such as:

- concrete command lines for the current machine
- local managed-runtime process settings
- auth/bootstrap material
- machine-local install paths

Those stay in provider-platform settings / configured providers / app bootstrap
for the current machine.

Example direction:

```yaml
runtime_profiles:
  goose_rest_sse:
    kind: goose_rest_sse
    adapter_family: goose_rest_sse
    capability_class: legacy_operator_grade

  claude_agent_acp:
    kind: acp
    adapter_family: claude_agent_acp
    capability_class: operator_grade
    requires:
      - session_load
      - session_update
      - permission_callbacks
      - mcp_attach

  gemini_cli_acp:
    kind: acp
    adapter_family: gemini_cli_acp
    capability_class: native_candidate
    requires:
      - session_load
      - session_update
      - permission_callbacks
      - mcp_attach
```

#### 5.2 Bind `runtime_profile` to `backend_profiles`

Example direction:

```yaml
backend_profiles:
  claude_orchestrator_high:
    provider: claude_code
    model: claude-opus-4.6
    effort: high
    temperature: 0.1
    max_turns: 20
    structured_output: required
    runtime_profile: goose_rest_sse

  claude_orchestrator_high_acp:
    provider: claude_code
    model: claude-opus-4.6
    effort: high
    temperature: 0.1
    max_turns: 20
    structured_output: required
    runtime_profile: claude_agent_acp

  gemini_reasoning_pro_high_acp:
    provider: gemini
    model: gemini-2.5-pro
    effort: high
    temperature: 0.1
    max_turns: 16
    structured_output: required
    runtime_profile: gemini_cli_acp
```

#### 5.3 Agents continue to select backend profiles

Agents should continue to choose only `backend_profile`.
That keeps one canonical bundle for:

- provider
- model
- runtime
- transport choice

---

## 6. Required capability model

Proposal 026 should not treat all ACP servers as equal.
It should define capability classes.

### Class A — lifecycle-capable

Must support:

- initialize
- session/new
- session/prompt
- cancel/close

### Class B — control-capable

Must additionally support:

- trustworthy session-load or equivalent state inspection
- runtime model/mode mutation truth good enough for reports
- session-scoped MCP realization
- requested-to-effective runtime capability check

### Class C — operator-grade

Must additionally support enough evidence for Forge live surfaces:

- useful `session/update`
- permission callbacks or equally strong permission truth
- tool-call visibility
- replay/history enough for reports/recovery/live timeline
- stable runtime truth for provider/model/session behavior

### Mapping for current candidates

- **Claude Agent ACP** -> current best **operator-grade** candidate
- **Gemini CLI ACP** -> strong **control-capable** and near-operator-grade native candidate
- **Auggie** -> strong second-wave control candidate
- **Junie** -> strong second-wave control candidate
- **OpenCode** -> strong broader runtime, but ACP still below operator-grade for Forge
- **Goose ACP** -> reference/bridge only

---

## 7. Required invariants

Proposal 026 must preserve these invariants:

1. Goose stays functional during the first ACP proposal.
2. Current execution truth is not downgraded just to accelerate migration.
3. MCP requested→effective truth remains product-owned and transport-neutral.
4. Runtime choice is frozen into run snapshot truth.
5. Reports and recovery surfaces continue to read canonical Forge truth, not adapter-specific heuristics.
6. ACP support is additive in Proposal 026, not destructive.

---

## 8. What Proposal 026 must not do

Proposal 026 must **not**:

- remove Goose as the default runtime
- require global cutover
- encode raw ACP command selection directly on each agent
- assume all ACP servers have identical observability
- degrade persisted report/recovery truth for the sake of architectural neatness
- treat OpenCode ACP as the primary target just because the broader runtime product is strong
- treat Goose ACP as a serious long-term target

---

## 9. Success criteria for Proposal 026

Proposal 026 is successful when:

1. core runtime code is no longer Goose-shaped in its canonical transport vocabulary,
2. Goose still works,
3. runtime selection exists through catalog/runtime-profile configuration,
4. at least two ACP runtimes can be selected via backend profiles,
5. run snapshot truth freezes runtime selection,
6. ACP-backed runs do not break canonical execution truth, report truth, or MCP truth.

Minimum first-wave runtimes:

- Claude Agent ACP
- Gemini CLI ACP

---

## 10. Follow-up proposal

The next ACP runtime expansion proposal should introduce:

- Auggie CLI ACP
- Junie CLI ACP

That second-wave proposal should assume Proposal 026 already delivered:

- ACP seam extraction
- runtime-profile schema
- first-wave adapters
- capability-gated runtime selection

---

## 11. Final recommendation

The current best path is:

- keep Goose alive,
- make ACP the long-term transport direction,
- add ACP support through runtime profiles,
- ship first-wave adapters for Claude Agent ACP and Gemini CLI ACP,
- then expand to Auggie and Junie in the next proposal.

This path is additive, realistic, and consistent with the current evidence.

It avoids two bad outcomes:

1. a premature hard cutover away from Goose,
2. and a fake “runtime neutrality” claim that still hides Goose semantics in core code.
