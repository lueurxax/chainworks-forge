---
name: security-review
description: Use when reviewing a Chainworks implementation for security and privacy release blockers.
compatibility: Chainworks Forge security review stages with frozen mission, evidence, permission, and output contracts.
---
# Security Review Procedure

1. Stay within the frozen proposal, mission, and evidence declared by the compiled task. Inspect `tests_result` directly only when it is declared by the compiled task; otherwise do not invent or fetch it.
2. Inspect authentication, authorization, secrets, unsafe defaults, injection, serialization, filesystem and symlink boundaries, network boundaries, data leakage, and dependency risk when implicated by the change.
3. Use read-only scanner results as evidence rather than as a substitute for reasoning.
4. Accept only the declared control-plane-generated `changed_files_manifest` as canonical Git evidence; do not substitute a caller- or provider-authored manifest. Never invoke `git status`, `git diff`, or `git rev-parse`, and never read `.git`.
5. Keep discovery bounded to changed and implicated paths. Exclude generated and build roots and cap output before any broader search permitted by the frozen assignment.
6. Return `pass` only when no blocking security issue remains and the required evidence is sufficient.
7. Publish only the logical output `security_report` under `security_report_v1`; do not mutate source, proposal, approval, release, or external state.

This procedure grants no additional tools, filesystem access, permission, or transition authority beyond the frozen agent binding.
