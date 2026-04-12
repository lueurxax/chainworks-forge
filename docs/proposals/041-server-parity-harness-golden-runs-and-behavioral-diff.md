# Proposal 041: Server Parity Harness, Golden Runs, and Behavioral Diff

| Field | Value |
|---|---|
| Date | 2026-04-11 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 027 |
| Goal | Prove that the server-side Rust + SQLite control plane behaves equivalently to the current client-owned logic before any user-visible cutover. |

## 1. Why this proposal exists

Proposal 027 builds a server-side parity replica of the current client logic.
That alone is not enough.

Without a parity harness, the repo will quickly end up in the worst possible state:

- the server exists,
- the client still exists,
- both appear to implement the same semantics,
- but nobody can say with confidence whether they actually agree on real workflows.

This proposal creates the proof layer that sits between server extraction and thin-client cutover.

## 2. Outcome

After Proposal 041:

- the repo has golden run fixtures that represent canonical workflow behavior,
- the server can replay those scenarios deterministically,
- the system can diff client vs server behavior explicitly,
- divergence is reported in a structured, operator-readable form,
- thin-client cutover no longer depends on trust or manual spot checks.

## 3. Scope

This proposal includes:

- golden run fixture capture
- deterministic replay inputs
- behavioral diff tooling
- shadow execution mode
- divergence reports
- repo-owned parity proof gates

This proposal does **not** include:

- changing product semantics
- replacing the client
- redesigning MCP
- production multi-host deployment

## 4. Parity questions this proposal must answer

The system must be able to answer:

1. Does the server choose the same next stage as the client?
2. Does it preserve the same approval, retry, and recovery semantics?
3. Does it produce the same run terminality and report truth?
4. If it diverges, can the repo show exactly where and how?

## 5. Golden run model

The parity harness should use a set of stable golden scenarios, including:

- canonical proposal loop
- implementation loop with refinement and review
- approval gate pauses and resumes
- retry and recovery flows
- terminal reporting

Each golden scenario should freeze:

- workflow snapshot
- catalog snapshot
- required runtime evidence or stubbed runtime outputs
- expected run/stage settlement summary
- expected report/recovery outputs

## 6. Behavioral diff contract

The diff should compare at least:

- run status
- current and settled stage IDs
- approval state
- retry disposition
- recovery suggestions
- report outputs
- transition lineage

The output should be a structured divergence report, not a generic test failure blob.

## 7. Shadow execution mode

Before thin-client cutover, the system should support a shadow mode where:

- the client remains the active owner,
- the server evaluates the same scenario in parallel,
- results are compared,
- divergence is recorded without changing user-visible behavior.

This is the main safety bridge between Proposal 027 and Proposal 031.

## 8. Risks

### 8.1 False parity confidence

Risk:
- the harness proves only toy scenarios.

Mitigation:
- require same-tree golden runs that represent real proposal/implementation/recovery slices.

### 8.2 Non-deterministic comparisons

Risk:
- runtime timing and unordered evidence make comparisons noisy.

Mitigation:
- freeze inputs,
- compare normalized semantic outputs,
- exclude transport noise from the parity surface.

## 9. Acceptance criteria

Proposal 041 is complete when:

1. golden run fixtures exist for the canonical workflow slices,
2. server replay can evaluate them deterministically,
3. client vs server divergence is reported in a structured diff,
4. shadow execution mode exists for same-tree validation,
5. the repo has a parity-focused proof gate that must stay green before cutover.

## 10. Final recommendation

Proposal 041 should be treated as mandatory.

Without it, Proposal 027 becomes an unproven rewrite and Proposal 031 becomes a risky leap of faith.
