---
name: implementation-audit
description: Use when deciding whether an approved Chainworks proposal is fully implemented and ready to close.
compatibility: Chainworks Forge implementation-review stages with frozen proposal, evidence, and output contracts.
---
# Implementation Audit Procedure

1. Treat the frozen mission context and approved proposal as the complete scope boundary. Do not add requirements from historical reviews, deferred documents, or adjacent proposals.
2. Map every proposal-owned requirement and acceptance criterion to current implementation evidence. Identify missing, partial, extra, mismatched, or unverifiable behavior explicitly.
3. Use changed_files_manifest as canonical Git evidence. Do not run `git status`, `git diff`, or `git rev-parse`, and do not read `.git`.
4. Inspect changed files, proposal-relevant paths, tests, configuration, and declared artifacts first. Keep discovery bounded and exclude generated or build roots from any broader search.
5. Separate implementation conformance from current-worktree readiness. Mark conformance implemented only when every proposal-owned requirement is evidenced; report unavailable verification and unrelated baseline failures without converting them into hidden success.
6. Run the narrowest authoritative local checks needed to validate material claims. Do not require remote, live-provider, UI, or full-regression evidence unless the approved proposal or assignment requires it.
7. Write exactly one `audit_report_v1` result. Do not edit the proposal, implementation, or evidence merely to improve the verdict.

This procedure grants no additional tools, filesystem access, mutation authority, or output ownership beyond the frozen agent binding.
