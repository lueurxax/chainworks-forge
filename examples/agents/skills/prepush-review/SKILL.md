---
name: prepush-review
description: Use for the final Chainworks code-quality review before an approved implementation may proceed to release Git actions.
compatibility: Chainworks Forge pre-push review stages with frozen audit, security, test, and output contracts.
---
# Pre-Push Review Procedure

1. Treat the approved proposal and frozen mission as the complete scope boundary.
2. Evaluate correctness, maintainability, regression risk, surprising side effects, and missing tests without adding unrelated improvements.
3. Consume exactly the canonical changed-files, implementation-audit, and security evidence declared by the compiled task. Inspect `tests_result` directly only when it is declared by the compiled task; otherwise do not invent or fetch it.
4. Accept only the declared control-plane-generated `changed_files_manifest` as canonical Git evidence; do not substitute a caller- or provider-authored manifest. Never invoke `git status`, `git diff`, or `git rev-parse`, and never read `.git`.
5. Keep discovery bounded to changed and implicated paths. Exclude generated and build roots and cap output before any broader search permitted by the frozen assignment.
6. Return `block` when required evidence is missing, invalid, red, or contains an unresolved blocking finding. Never reinterpret a blocking security or audit result as `pass`.
7. Publish only the logical output `prepush_review_report` under `prepush_review_v1`; do not edit source, commit, push, approve, release, or cause external effects.

This procedure grants no additional tools, filesystem access, permission, or transition authority beyond the frozen agent binding.
