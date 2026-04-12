# Goose Provider Remediation

Status: **Historical / Legacy**

This document is retained only as historical evidence for the removed Goose remediation path.
It is not part of the current canonical ACP-backed provider platform.

Stable reference for the Goose-backed provider remediation owner path that was previously carried by Proposal 010's provider-troubleshooting slice.

## Purpose

For `codex` and `claude_code`, the product must diagnose the exact Goose-backed path the runtime uses.
It is not enough for a nearby terminal flow to work.

This document defines the implemented operator journey for Goose-backed provider verification and remediation.

## Scope

This reference covers:

- Goose-first troubleshooting for `codex` and `claude_code`,
- the remediation owner path,
- handshake probe phases,
- state progression from unverified to verified/failing,
- evidence disclosure shown to the operator.

`gemini` remains part of the provider platform, but does not gain a fake Goose-specific remediation journey unless the runtime actually depends on one.

## Owner path

One provider source of truth remains in place:

- `ConfiguredProvider` is the durable provider record,
- `ProviderSettingsStore` remains the only persisted owner of provider edits,
- `ProviderRegistry` remains the owner of derived health,
- `PreflightService` and `PilotReadinessView` consume that same derived truth.

Canonical operator journey:

1. unhealthy provider row or first-run bootstrap step,
2. `GooseProviderConnectionAssistant`,
3. `Save and Verify`,
4. evidence disclosure if needed,
5. return to provider summary or `PilotReadinessView` with refreshed health.

This flow does not create a parallel configuration store.

## Primary integration rule

For `codex` and `claude_code`, Goose-backed setup is the primary operator path.

That means:

- the app verifies what live runtime actually uses,
- direct CLI checks are secondary diagnostics, not the primary story,
- failures are attributed to Goose endpoint/provider/model/policy steps first,
- run-start readiness consumes the same Goose-backed truth.

## Troubleshooting states

The remediation journey exposes these operator states:

1. `not_configured`
2. `configured_unverified`
3. `probing`
4. `verified`
5. `degraded`
6. `failing`

These states sit underneath the top-level `ProviderHealthSnapshot`.

Suggested mapping:

- `not_configured` / `configured_unverified` -> top-level health `unknown`
- `probing` -> transient `unknown` or `degraded`
- `verified` -> top-level health `healthy`
- `degraded` -> top-level health `degraded`
- `failing` -> top-level health `degraded` or `unavailable`

## Failure attribution

When remediation fails, the failing layer must be explicit.

Current failure attribution vocabulary:

- `binary_or_runtime_missing`
- `endpoint_unreachable`
- `goose_provider_not_available`
- `auth_failed`
- `model_resolution_failed`
- `capability_mismatch`
- `policy_mismatch`
- `unknown`

## Handshake probe contract

The probe path must test the real dependency chain the app needs:

1. provider selection,
2. Goose transport selection,
3. endpoint/runtime discovery,
4. auth expectations,
5. Goose provider identifier resolution,
6. model resolution,
7. live handshake probe.

The product should not report a generic connection failure when a more specific failing layer is known.

## Evidence panel

The operator must be able to inspect the last remediation result without leaving the app.

`ProviderSetupEvidencePanel` surfaces:

- family,
- transport,
- auth mode,
- endpoint,
- Goose provider identifier,
- selected/default model,
- latest checked time,
- latest verification result,
- handshake step results,
- actionable remediation guidance,
- raw probe details for advanced debugging.

## Relationship to provider health and preflight

Goose remediation does not replace the broader provider platform.

It extends it by supplying:

- Goose-specific derived diagnostics,
- fail-closed preflight checks for Goose reachability,
- clearer ownership for Codex/Claude setup,
- evidence the operator can inspect before run start.

## Boundaries

This reference does not define:

- new provider families,
- provider/model provenance truth across frozen runs,
- stop/cancel behavior,
- working-directory ownership.

Those boundaries remain outside this document.
