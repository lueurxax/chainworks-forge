# Schema-Version Evolution Policy

Status: implemented execution-truth contract.

Execution-truth JSON, MCP, GraphQL readback, rollout, and evidence payloads that expose a
`schema_version` field follow append-only version semantics.

## Same-Version Changes

Same-version changes may add optional fields only when all existing consumers can
ignore unknown optional fields or when the envelope already declares
`additionalProperties: true` for that extension object. A same-version change may
not remove fields, rename fields, narrow enum values, change scalar types, change
required fields, or change the meaning of an existing field.

## Version Bumps

A schema version must be bumped when a change adds a required field, removes a
field, renames a field, narrows or renames enum values, changes scalar type,
changes redaction semantics, changes idempotency/hash inputs, or changes command
admission/denial meaning. The new version must remain readable beside the prior
version until all durable rows and evidence fixtures that use the old version are
outside the supported retention window.

## Prior-Version Readability

Readers must keep prior supported versions readable and map them into the current
operator readback shape without mutating the stored payload. Missing fields from a
prior version are represented as explicit null/absent readback values, not
synthetic success. If a prior version cannot provide a field required for a
mutation or command decision, that path fails closed before mutation.

## Unknown Versions

Unknown `schema_version` values are diagnostics, not best-effort input. Runtime
mutation and command paths reject unknown versions before side effects with a
typed schema/diagnostic failure. Readback surfaces preserve the fact that the
version is unknown and redact payload details according to the caller class.

## Evidence

The fixture
`docs/evidence/083/api/schema-version-evolution-policy.fixture.json` records the
machine-checkable cases for same-version additive fields, required version bumps,
prior-version readability, and unknown-version diagnostics.
