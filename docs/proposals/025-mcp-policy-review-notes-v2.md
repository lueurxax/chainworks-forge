# Proposal 025 review notes (revised MCP allocation)

## Main verdict

Proposal 025 is still directionally correct: per-agent MCP policy must be explicit, runtime-applied before prompt submission, and fail closed when the requested extension set cannot be honored.

The first minimal MCP pass was too austere for the actual product shape.

A better policy is:

- still default-deny,
- but allow more MCP on agents where the tool meaningfully changes outcome quality or reduces shell/tool churn,
- while continuing to keep the broadest stateful or side-effect-heavy servers off by default.

## What changed in this revised recommendation

### Add Xcode to proposal-side agents that benefit from visual grounding
Recommended:
- proposal_writer -> `xcode`
- proposal_reviewer_product_owner -> `xcode`
- proposal_reviewer_ux -> `xcode`, optional `autovisualiser`
- proposal_reviewer_ui -> `xcode`, optional `autovisualiser`
- proposal_reviewer_architect -> `xcode`, optional `context7`

Reason:
for proposal quality, current-screen truth and previewability are often worth more than another abstract paragraph.

### Add developer lane to code and verification agents
Recommended:
- code_writer -> `developer`, `xcode`, optional `context7`, optional `todo`, optional `summon`
- proposal_implementation_auditor -> `developer`, optional `xcode`, optional `analyze`
- prepush_code_reviewer -> `developer`, optional `xcode`
- security_checker -> `developer`, optional `analyze`, optional `context7`

Reason:
these agents already live close to code and build/test truth. Giving them a developer-oriented MCP lane is likely higher ROI than keeping them shell-only.

### Allow summon only on bounded coordination / build lanes
Recommended:
- lead_orchestrator -> optional `summon`
- code_writer -> optional `summon`

Not recommended broadly for:
- reviewers
- security
- docs
- release
- steward retrospectives

Reason:
`summon` can reduce wall-clock time, but it can also multiply burn if granted to many agents at once.

## What still stays off by default

The following servers should still remain unassigned in normal workflow agents until a dedicated use case exists:

- `memory`
- `chatrecall`
- `computercontroller`
- `apps`
- `chromedevtools`
- `extensionmanager`
- `tutorial`
- `tom`
- `orchestrator`

Reason:
they either broaden hidden state, increase non-determinism, or introduce wide side-effect surfaces that do not currently justify the burn and debugging cost.

## Additional proposal remarks

1. Proposal 025 should add an explicit installed-server registry / capability map.
   The current catalog still uses conceptual `permission_profiles.*.mcp.allow` values that do not match the real installed-server namespace.

2. Proposal 025 should distinguish:
   - `required_extensions`
   - `optional_extensions`
   - and `fallback_policy`
   so that nice-to-have MCPs do not block a run unnecessarily.

3. Proposal 025 should add telemetry:
   - requested extension count
   - effective extension count
   - startup latency by extension set
   - tool-call count by extension
   - bytes returned by extension
   - run-level burn delta before/after MCP tightening
