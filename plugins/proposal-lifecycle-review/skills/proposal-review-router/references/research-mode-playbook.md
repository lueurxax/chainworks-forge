# Research Mode Playbook

Research mode answers narrow external questions that local evidence could not settle.

## Preconditions

Do not research until the evidence pack includes:

- proposal inventory
- adjacent-doc inventory or explicit absence
- baseline status
- proposal integration-context status
- current manifest or code-path map
- fingerprint tags with evidence IDs
- research triggers with local evidence IDs

If these are missing, continue local review or return an evidence gap.

## Good research triggers

Good triggers cite local evidence and ask a narrow question:

- `RES-01` from `MAP-03`: current `tonic` streaming cancellation behavior for the exact Rust pattern proposed.
- `RES-02` from `DATA-02`: protobuf backward-compatibility rule for adding a field consumed by an Apple client and Go service.
- `RES-03` from `DOC-04`: current Apple platform guidance for a macOS settings-window pattern.

Bad triggers are broad:

- Research Rust performance.
- Find articles about microservices.
- What is good UX?

## Source preference

Use primary or authoritative sources first:

1. Official platform docs, RFCs, standards, vendor docs.
2. Primary library/framework docs and release notes.
3. Security advisories and protocol specifications.
4. High-quality engineering writeups only when primary sources are insufficient.

## Research pack requirements

Write `<proposal>.review/research-pack.md` when producing artifacts.

Include:

- local trigger IDs
- why local evidence was insufficient
- source ledger with freshness risk
- applicability notes for this repo
- what research changed or confirmed
- remaining unknowns

## Synthesis rules

The final review must distinguish:

- local-evidence findings
- research-confirmed findings
- time-sensitive recommendations
- unresolved questions

Do not let research override current repo facts unless the proposal explicitly depends on current external behavior.
