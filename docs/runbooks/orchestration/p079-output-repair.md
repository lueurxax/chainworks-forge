# P079: Output Contract Repair and Provider Fallback Runbook

## Overview

This runbook provides guidance for operators on understanding and observing the implemented P079 functionality: Contract-Aware Output Repair and Provider Fallback. Today, P079 records durable evidence for output-contract failures, can run deterministic fixture same-session repair, accepts bounded transport-attributed provider-envelope recovery when enabled, and fails closed for advisory-only production providers. Controlled provider fallback dispatch remains deferred.

## Key Concepts

*   **Output Contract Repair**: A mechanism to attempt to fix invalid or missing outputs from an agent within the same session. In production provider lanes this is currently fail-closed unless enforceable permission restrictions exist.
*   **Provider Fallback**: The schema and readback contract exist, but controlled fallback dispatch from frozen policy is not wired today.
*   **Leasing**: A transactional mechanism to ensure single-flight execution of repair or fallback attempts.
*   **Evidence**: Detailed records of repair and fallback attempts, including status, failure classifications, and outcomes, are stored as durable evidence.
*   **Hold Conditions**: Strict rules that, if violated, block repair or fallback actions to prevent unintended behavior or data integrity issues.

## Observability

P079 functionality is exposed through various observability channels, allowing operators to monitor its behavior and diagnose issues.

### Metrics

The P079 rollout contract defines the following metric names for adoption and operational monitoring:

*   `p079_eligible_output_failures_recovered_percent`: Percentage of eligible output failures successfully recovered.
*   `p079_output_repair_attempt_total{role,provider_family,failure_class,result}`: Total output repair attempts, broken down by role, provider family, failure class, and result.
*   `p079_transcript_recovery_total{role,recovery_source,result}`: Total transcript recovery attempts, broken down by role, recovery source, and result.
*   `p079_provider_fallback_attempt_total{role,failed_provider_family,fallback_provider_family,result}`: Total provider fallback attempts.
*   `p079_repair_budget_exhausted_total{role}`: Total times the repair budget was exhausted.
*   `p079_fallback_budget_exhausted_total{role}`: Total times the fallback budget was exhausted.

Core repair and recovery metric emission is implemented in the DB metrics inventory. Dashboards and live provider-fallback dispatch readback remain deferred with the fallback lane. Use run report, MCP, GraphQL readback, and logs for current diagnostics.

### Run Report and MCP

The `output_contract_repair` object is available in run reports and through the MCP (Model Context Protocol) server for detailed inspection of specific agent execution outcomes.

*   **Run Report**: Look for the `output_contract_repair` section within the agent execution details. This section will contain the `OutputContractRepairEvidence` which details the `status`, `initial_failure_class`, `final_output_settlement`, `recommended_next_action`, and other relevant information.
*   **MCP**: Use MCP commands (e.g., `mcp reports.get <run_id>`) to retrieve run details and examine the `output_contract_repair` field.

### GraphQL

The GraphQL API exposes `OutputContractRepairEvidence` on `AgentExecution` types. This allows for programmatic access to P079-related data for custom dashboards or analysis.

*   **Field**: `AgentExecution.outputContractRepair`
*   **Details**: Provides structured evidence including `schemaVersion`, `repairAttemptId`, `status`, `finalOutputSettlement`, `recommendedNextAction`, etc.

## Troubleshooting and Actions

### Interpreting `OutputContractRepairEvidence`

When reviewing `OutputContractRepairEvidence`, pay attention to the following fields:

*   **`status`**: Indicates the overall outcome of the repair/fallback process.
    *   `recovered`: Success! The output issue was resolved.
    *   `blocked`, `failed`, `cancelled`: The repair/fallback attempt did not succeed. Investigate `initial_failure_class`, `initial_failure_subtype`, and `recommended_next_action` for more details.
*   **`initial_failure_class` / `initial_failure_subtype`**: Provides insight into *why* the original output failed.
*   **`final_output_settlement`**: Describes how the output was ultimately handled (for example, `blocked_missing_required_outputs`; fallback settlement values are schema-defined but not produced by controlled fallback dispatch today).
*   **`recommended_next_action`**: Offers explicit guidance for the next steps.

### Common Scenarios and Recommended Actions

*   **`status: blocked` / `recommended_next_action: manual_investigation`**:
    *   **Cause**: A repair or fallback attempt was blocked, potentially due to a violation of P079 hold conditions or an unrecoverable error.
    *   **Action**: Review the `initial_failure_class`, `initial_failure_subtype`, and any associated logs. Check the `Hold Conditions` section in the P079 proposal document for potential causes. Escalate if necessary.
*   **`status: failed` / `recommended_next_action: inspect_repair_evidence`**:
    *   **Cause**: A repair or fallback attempt failed, possibly due to a logical error or persistent issue.
    *   **Action**: Examine the full `OutputContractRepairEvidence` payload for details. Look for specific error messages in logs if available. Consider re-running the agent with additional debugging if the issue is reproducible.
*   **`status: recovered`**:
    *   **Cause**: The system successfully repaired output in an implemented lane. Today that primarily means deterministic fixture repair; production provider repair remains fail-closed.
    *   **Action**: No immediate action required. This indicates successful self-healing. Monitor metrics to ensure this behavior is consistent and not masking deeper issues.
*   **Repeated `skipped` / `manual_investigation` evidence**:
    *   **Cause**: Production provider repair is advisory-only, Junie plan-mode output is ineligible, or another hold condition blocked recovery.
    *   **Action**: Inspect `initial_failure_class`, `initial_failure_subtype`, `permission_decisions`, and logs. Treat the original output failure as unresolved unless a valid repaired output was accepted.

## Rollback / Disablement

In case of critical issues or during controlled experiments, P079 can be rolled back or disabled.

*   **Procedure**:
    1.  Disable the feature flags: `CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED`, `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED`, and `CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED`.
    2.  No database schema rollback is required; existing evidence remains readable.
    3.  Monitor system behavior to ensure a smooth return to pre-P079 output handling.
*   **Impact**: Disabling P079 will revert output failure handling to the pre-P079 behavior, where most output contract failures will result in immediate stage blockages.

## Related References

*   [Output Contracts, Failure Evidence, and Narrow Recovery](../../reference/output-contracts-failure-evidence-and-recovery.md): Detailed technical reference for P079 schema and contracts.
*   [P079 Repair Prompt Template](../../reference/p079-repair-prompt-template.md): Pinned repair prompt version and sanitization rules.
*   [P079 Recovery Attribution](../../reference/p079-recovery-attribution.md): Transcript/provider-envelope recovery attribution and bounds.
*   [P079 Adapter Idempotency](../../reference/p079-adapter-idempotency.md): Lease, restart, and adapter idempotency rules.
*   [Executable Rollout Gate Template](../../reference/executable-rollout-gate-template.md): General guidance on the rollout-contract preflight, gate aliases, and operator readback used by P079.
*   [P079 Proposal Document](../../proposals/079-contract-aware-output-repair-and-provider-fallback.md): The original proposal document for full context.
