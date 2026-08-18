use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;
use serde_json::json;

use crate::protocol::McpTool;
use crate::tools::scanner::{
    assemble_scan_dto, scan_diagnostic_test_root, scan_multi_root, ScanPermitError, ScanRootTarget,
};
use domain::temp_artifact_inventory::{
    validate_inventory_limit, validate_inventory_timeout_ms, validate_run_id,
    validate_test_root_override, EnabledState, InventoryErrorCode, InventoryMode, InventoryStatus,
    MutationGuardStatus, RootKind, TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
};

/// Scope selector parsed from the request. Exactly one must be present for enabled modes.
#[derive(Clone)]
enum ScopeSelector {
    RunId(String),
    WorkspaceContext(PathBuf),
}

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "temp_artifacts.inventory.preview".to_string(),
        description:
            "Read-only advisory temporary artifact inventory preview. Returns disabled readback \
             when CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE=disabled (the default). Filesystem \
             scanning occurs in hidden_readback and operator_visible modes. No deletion, cleanup, \
             mutation, or persistence occurs in any mode."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "Run ID to scope the inventory preview."
                },
                "workspace_context": {
                    "type": "object",
                    "description": "Workspace-scoped inventory preview. Exactly one of run_id or workspace_context is required in enabled modes.",
                    "properties": {
                        "workspace_root": {
                            "type": "string",
                            "description": "Absolute workspace root. Only known Chainworks-managed descendants are inventoried."
                        }
                    },
                    "required": ["workspace_root"],
                    "additionalProperties": false
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum rows to return (0-500). Default 500.",
                    "minimum": 0,
                    "maximum": 500,
                    "default": 500
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Total request deadline in milliseconds including queue wait (1-5000). Default 5000.",
                    "minimum": 1,
                    "maximum": 5000,
                    "default": 5000
                },
                "include_dry_run": {
                    "type": "boolean",
                    "description": "Include advisory dry-run recommendation fields. Default true.",
                    "default": true
                },
                "test_root_override": {
                    "type": "string",
                    "description": "Diagnostic test root override. Accepted only when diagnostic test-root mode is enabled and caller is authorized. Absolute path only; no traversal, no tilde, maximum 4096 UTF-8 bytes."
                }
            },
            "oneOf": [
                {"required": ["run_id"], "not": {"required": ["workspace_context"]}},
                {"required": ["workspace_context"], "not": {"required": ["run_id"]}}
            ],
            "additionalProperties": false
        }),
        output_schema: Some(json!({
            "type": "object",
            "properties": {
                "schema_version": {"type": "string"},
                "status": {"type": "string"},
                "enabled_state": {"type": "string"},
                "mode": {"type": "string"},
                "disabled_reason_code": {"type": ["string", "null"]},
                "generated_at": {"type": "string"},
                "limits_applied": {
                    "type": "object",
                    "properties": {
                        "limit": {"type": "integer"},
                        "timeout_ms": {"type": "integer"},
                        "scan_deadline_at": {"type": ["string", "null"]},
                        "queue_wait_ms": {"type": "integer"}
                    },
                    "required": ["limit", "timeout_ms", "scan_deadline_at", "queue_wait_ms"],
                    "additionalProperties": false
                },
                "summary": {
                    "type": "object",
                    "properties": {
                        "artifact_tree_count": {"type": "integer"},
                        "estimated_bytes": {"type": "string", "pattern": "^(0|[1-9][0-9]*)$"},
                        "active_or_recent_count": {"type": "integer"},
                        "terminal_candidate_count": {"type": "integer"},
                        "orphan_candidate_count": {"type": "integer"},
                        "legacy_unmanaged_count": {"type": "integer"},
                        "scan_error_count": {"type": "integer"},
                        "dry_run_candidate_count": {"type": "integer"},
                        "truncated": {"type": "boolean"},
                        "queue_wait_ms": {"type": "integer"}
                    },
                    "required": [
                        "artifact_tree_count", "estimated_bytes", "active_or_recent_count",
                        "terminal_candidate_count", "orphan_candidate_count",
                        "legacy_unmanaged_count", "scan_error_count",
                        "dry_run_candidate_count", "truncated", "queue_wait_ms"
                    ]
                },
                "rows": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path_display": {"type": "string"},
                            "path_hash": {"type": "string"},
                            "path_hash_short": {"type": "string"},
                            "correlation_key": {"type": "string"},
                            "root_kind": {"type": "string"},
                            "artifact_kind": {"type": "string"},
                            "manifest_state": {"type": "string"},
                            "lifecycle_classification": {"type": "string"},
                            "dry_run_recommendation": {"type": ["string", "null"]},
                            "estimated_size_bytes": {"type": "string", "pattern": "^(0|[1-9][0-9]*)$"},
                            "last_touched_at": {"type": ["string", "null"]},
                            "active_process_evidence": {"type": ["string", "null"]},
                            "owner": {"type": ["string", "null"]},
                            "owner_inference": {"type": ["string", "null"]},
                            "status_token": {"type": "string"},
                            "generated_at": {"type": "string"},
                            "partial_errors": {"type": "array", "items": {"type": "string"}}
                        },
                        "required": [
                            "path_display", "path_hash", "path_hash_short", "correlation_key",
                            "root_kind", "artifact_kind", "lifecycle_classification",
                            "estimated_size_bytes", "status_token", "generated_at"
                        ]
                    }
                },
                "errors": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "code": {"type": "string"},
                            "message": {"type": "string"},
                            "root_kind": {"type": ["string", "null"]}
                        },
                        "required": ["code", "message"]
                    }
                },
                "dry_run": {"type": ["object", "null"]},
                "mutation_guard": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string"},
                        "checked_at": {"type": "string"},
                        "no_delete": {"type": "boolean"},
                        "no_prune": {"type": "boolean"},
                        "no_chmod": {"type": "boolean"},
                        "no_persist": {"type": "boolean"},
                        "no_retry": {"type": "boolean"}
                    },
                    "required": ["status", "checked_at", "no_delete", "no_prune", "no_chmod", "no_persist", "no_retry"]
                }
            },
            "required": [
                "schema_version", "status", "enabled_state", "mode", "disabled_reason_code",
                "generated_at", "limits_applied", "summary", "rows", "errors",
                "dry_run", "mutation_guard"
            ],
            "additionalProperties": false
        })),
    }]
}

/// Returns the current inventory mode from the daemon process-start environment.
/// Defaults to Disabled when the env var is absent or unrecognized.
pub fn current_inventory_mode() -> InventoryMode {
    std::env::var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE")
        .ok()
        .and_then(|s| InventoryMode::from_env_str(&s))
        .unwrap_or(InventoryMode::Disabled)
}

/// Builds a request-error canonical DTO with a redacted error message.
fn error_payload(error_code: &str, include_dry_run: bool) -> serde_json::Value {
    let now = Utc::now().to_rfc3339();
    json!({
        "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
        "status": InventoryStatus::Error.as_str(),
        "enabled_state": EnabledState::Unknown.as_str(),
        "mode": current_inventory_mode().as_str(),
        "disabled_reason_code": null,
        "generated_at": now,
        "limits_applied": {
            "limit": 0,
            "timeout_ms": 0,
            "scan_deadline_at": null,
            "queue_wait_ms": 0
        },
        "summary": {
            "artifact_tree_count": 0,
            "estimated_bytes": "0",
            "active_or_recent_count": 0,
            "terminal_candidate_count": 0,
            "orphan_candidate_count": 0,
            "legacy_unmanaged_count": 0,
            "scan_error_count": 0,
            "dry_run_candidate_count": 0,
            "truncated": false,
            "queue_wait_ms": 0
        },
        "rows": [],
        "errors": [{
            "code": error_code,
            "message": "<redacted>",
            "root_kind": null,
            "phase": null
        }],
        "dry_run": if include_dry_run {
            json!({
                "schema_version": "temp_artifact_dry_run_v1",
                "generated_at": now,
                "recommendation_counts": {},
                "mutation_guard": {
                    "status": MutationGuardStatus::Skipped.as_str(),
                    "checked_at": now
                }
            })
        } else {
            json!(null)
        },
        "mutation_guard": {
            "status": MutationGuardStatus::Skipped.as_str(),
            "checked_at": now,
            "no_delete": false,
            "no_prune": false,
            "no_chmod": false,
            "no_persist": false,
            "no_retry": false
        }
    })
}

/// Builds the disabled-mode canonical DTO.
fn disabled_payload(include_dry_run: bool) -> serde_json::Value {
    let now = Utc::now().to_rfc3339();
    json!({
        "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
        "status": InventoryStatus::Disabled.as_str(),
        "enabled_state": EnabledState::Disabled.as_str(),
        "mode": current_inventory_mode().as_str(),
        "disabled_reason_code": "mode_disabled",
        "generated_at": now,
        "limits_applied": {
            "limit": 0,
            "timeout_ms": 0,
            "scan_deadline_at": null,
            "queue_wait_ms": 0
        },
        "summary": {
            "artifact_tree_count": 0,
            "estimated_bytes": "0",
            "active_or_recent_count": 0,
            "terminal_candidate_count": 0,
            "orphan_candidate_count": 0,
            "legacy_unmanaged_count": 0,
            "scan_error_count": 0,
            "dry_run_candidate_count": 0,
            "truncated": false,
            "queue_wait_ms": 0
        },
        "rows": [],
        "errors": [],
        "dry_run": if include_dry_run {
            json!({
                "schema_version": "temp_artifact_dry_run_v1",
                "generated_at": now,
                "recommendation_counts": {},
                "mutation_guard": {
                    "status": MutationGuardStatus::Skipped.as_str(),
                    "checked_at": now
                }
            })
        } else {
            json!(null)
        },
        "mutation_guard": {
            "status": MutationGuardStatus::Skipped.as_str(),
            "checked_at": now,
            "no_delete": true,
            "no_prune": true,
            "no_chmod": true,
            "no_persist": true,
            "no_retry": true
        }
    })
}

/// Builds a deadline-exceeded canonical DTO for when the pre-scan scope/override
/// resolution (canonicalize, containment, is_dir) itself exceeds `timeout_ms`
/// before the scan phase is ever reached (SR-MEDIUM-001).
fn timeout_payload(include_dry_run: bool) -> serde_json::Value {
    let now = Utc::now().to_rfc3339();
    json!({
        "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
        "status": InventoryStatus::Timeout.as_str(),
        "enabled_state": EnabledState::Unknown.as_str(),
        "mode": current_inventory_mode().as_str(),
        "disabled_reason_code": null,
        "generated_at": now,
        "limits_applied": {
            "limit": 0,
            "timeout_ms": 0,
            "scan_deadline_at": null,
            "queue_wait_ms": 0
        },
        "summary": {
            "artifact_tree_count": 0,
            "estimated_bytes": "0",
            "active_or_recent_count": 0,
            "terminal_candidate_count": 0,
            "orphan_candidate_count": 0,
            "legacy_unmanaged_count": 0,
            "scan_error_count": 0,
            "dry_run_candidate_count": 0,
            "truncated": false,
            "queue_wait_ms": 0
        },
        "rows": [],
        "errors": [{
            "code": InventoryErrorCode::DeadlineExceeded.as_str(),
            "message": "<redacted>",
            "root_kind": null,
            "phase": null
        }],
        "dry_run": if include_dry_run {
            json!({
                "schema_version": "temp_artifact_dry_run_v1",
                "generated_at": now,
                "recommendation_counts": {},
                "mutation_guard": {
                    "status": MutationGuardStatus::Skipped.as_str(),
                    "checked_at": now
                }
            })
        } else {
            json!(null)
        },
        "mutation_guard": {
            "status": MutationGuardStatus::Skipped.as_str(),
            "checked_at": now,
            "no_delete": false,
            "no_prune": false,
            "no_chmod": false,
            "no_persist": false,
            "no_retry": false
        }
    })
}

/// Returns the configured diagnostic test-root allowlist from the daemon environment.
/// Reads `CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS` as a colon-separated list
/// of absolute paths (like PATH). Empty list means the diagnostic test-root is disabled.
pub fn load_diagnostic_test_roots() -> Vec<std::path::PathBuf> {
    std::env::var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let p = std::path::PathBuf::from(s);
            if p.is_absolute() {
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

/// Resolves `override_path` to a canonical contained `PathBuf` within the diagnostic allowlist.
///
/// Performs containment in two phases:
/// 1. Lexical: the raw path must start with an allowlisted root (fast reject).
/// 2. Realpath: if the path exists, its canonicalized form must also be within the
///    canonicalized allowlist root. This catches intermediate symlink escapes (SEC-P089-002).
///    If the path does not exist, the lexically-normalized path is returned so the scanner
///    can emit root_unreadable rather than an authorization error.
///
/// Returns `None` if the allowlist is empty, lexical containment fails, or the
/// realpath-canonicalized path escapes the allowlist (symlink escape).
///
/// IMPORTANT: callers must pass the RETURNED PathBuf to the scanner — never the raw input —
/// so the scanner operates on the canonical path that passed containment checks.
fn resolve_contained_test_root(override_path: &str) -> Option<PathBuf> {
    let allowlist = load_diagnostic_test_roots();
    if allowlist.is_empty() {
        return None;
    }
    let override_pb = PathBuf::from(override_path);
    // Lexical containment: the raw path must start with a *specific* allowlisted root.
    // The resolved-path containment check below is bound to this same matched root
    // (not "any" allowlist root) so a symlink lexically inside allowlist root A that
    // resolves into a different allowlist root B is rejected as an escape rather than
    // accepted (audit: same-root containment, not "any root" containment).
    let Some(matched_root) = allowlist.iter().find(|root| override_pb.starts_with(root)) else {
        return None;
    };
    // Realpath containment: if the path exists, its canonicalized form must also be within
    // the *same* canonicalized allowlist root. This prevents intermediate-symlink escape,
    // including escape into a different allowlist entry (SEC-P089-002).
    match std::fs::canonicalize(&override_pb) {
        Ok(canonical) => {
            let canonical_root =
                std::fs::canonicalize(matched_root).unwrap_or_else(|_| matched_root.clone());
            if canonical.starts_with(&canonical_root) {
                Some(canonical) // Use canonical path so the scanner never follows the raw input
            } else {
                None // Symlink escape: canonical path left the matched allowlist root
            }
        }
        Err(_) => {
            // Path does not exist; lexical check passed.
            // Return the lexically-normalized path — scanner will emit root_unreadable.
            Some(override_pb)
        }
    }
}

/// Scopes the scan to the specific run's meta directory only.
///
/// Defense-in-depth: even after `validate_run_id`, canonicalize both the expected
/// `runs/` parent and the candidate run directory, then verify the run dir is contained
/// under `runs/`. This catches residual edge cases such as unexpected symlinks (SEC-P089-005).
fn discover_run_scoped_roots(run_id: &str) -> Vec<ScanRootTarget> {
    let mut roots = Vec::new();

    if let Some(meta_root) = resolve_meta_root() {
        let runs_root = meta_root.join("runs");

        // Canonicalize runs/ first; if it doesn't exist on disk there are no roots to scan.
        if let Ok(canonical_runs_root) = std::fs::canonicalize(&runs_root) {
            // Build and canonicalize the candidate run directory.
            let run_dir_candidate = canonical_runs_root.join(run_id);
            if let Ok(canonical_run_dir) = std::fs::canonicalize(&run_dir_candidate) {
                // Verify the canonical run directory is contained under the canonical runs/ root.
                if canonical_run_dir.starts_with(&canonical_runs_root) && canonical_run_dir.is_dir()
                {
                    roots.push(ScanRootTarget {
                        path: canonical_run_dir,
                        root_kind: RootKind::RunMetaRoot,
                    });
                }
            }
        }

        // A normal Chainworks meta root is `<workspace>/.chainworks`. Include the
        // other managed descendants of that same workspace so run-scoped previews
        // inventory per-workspace provider-home copies and caches as required,
        // without broadening the run-meta portion to every run in the workspace.
        if meta_root.file_name().and_then(|name| name.to_str()) == Some(".chainworks") {
            if let Some(workspace_root) = meta_root.parent().and_then(canonicalize_managed_root) {
                roots.extend(discover_workspace_common_roots(&workspace_root));
            }
        }
    }

    roots.extend(discover_managed_and_legacy_roots());
    roots
}

/// Discovers only known Chainworks-owned descendants of a canonical workspace.
/// The workspace root itself and unrelated project files are never enumerated.
fn discover_workspace_scoped_roots(workspace_root: &Path) -> Vec<ScanRootTarget> {
    let mut roots = canonicalize_managed_root(&workspace_root.join(".chainworks").join("runs"))
        .filter(|canonical| canonical.starts_with(workspace_root))
        .map(|path| {
            vec![ScanRootTarget {
                path,
                root_kind: RootKind::RunMetaRoot,
            }]
        })
        .unwrap_or_default();
    roots.extend(discover_workspace_common_roots(workspace_root));
    roots.extend(discover_managed_and_legacy_roots());
    roots
}

/// Discovers managed roots that are siblings of the run-meta tree inside one
/// canonical workspace. Kept separate so a run-scoped request can include its
/// workspace's provider homes/caches without scanning every other run.
fn discover_workspace_common_roots(workspace_root: &Path) -> Vec<ScanRootTarget> {
    let candidates = [
        (
            workspace_root.join(".chainworks").join("cargo-target"),
            RootKind::ControlPlaneCache,
        ),
        (
            workspace_root.join(".forge-codex-acp"),
            RootKind::ProviderHomeCopy,
        ),
        (
            workspace_root.join(".chainworks").join("tmp"),
            RootKind::LegacyChainworksTmp,
        ),
    ];
    candidates
        .into_iter()
        .filter_map(|(path, root_kind)| {
            canonicalize_managed_root(&path).and_then(|canonical| {
                if canonical.starts_with(workspace_root) {
                    Some(ScanRootTarget {
                        path: canonical,
                        root_kind,
                    })
                } else {
                    None
                }
            })
        })
        .collect()
}

/// Discovers the process-wide managed and legacy roots (control_plane_cache,
/// provider_home_copy, legacy_chainworks_tmp). These are not scoped to a single
/// run — they are shared/global directories the control plane and its ACP
/// provider adapters write into across every run — so every enabled-mode scan
/// includes them alongside the requested run's meta root, bounded by the same
/// limit/deadline/permit machinery as any other root.
///
/// Each root path is overridable via env var so tests (and operators diagnosing a
/// specific cache) can point at an isolated directory instead of the real shared
/// system path; production leaves these unset and gets the real defaults.
/// These roots are part of the approved inventory coverage in every enabled mode.
/// Their cost is bounded by the same row cap, total deadline, cooperative
/// cancellation, and worker-capacity lease as the run/workspace-scoped roots.

fn discover_managed_and_legacy_roots() -> Vec<ScanRootTarget> {
    let mut roots = Vec::new();

    let cache_root = std::env::var_os("CHAINWORKS_TEMP_ARTIFACT_CONTROL_PLANE_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(acp::adapters::chainworks_cache_root);
    if let Some(canonical) = canonicalize_managed_root(&cache_root) {
        roots.push(ScanRootTarget {
            path: canonical,
            root_kind: RootKind::ControlPlaneCache,
        });
    }

    let provider_home_fallback_root =
        std::env::var_os("CHAINWORKS_TEMP_ARTIFACT_PROVIDER_HOME_FALLBACK_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("forge-codex-acp"));
    if let Some(canonical) = canonicalize_managed_root(&provider_home_fallback_root) {
        roots.push(ScanRootTarget {
            path: canonical,
            root_kind: RootKind::ProviderHomeCopy,
        });
    }

    let legacy_root = std::env::var_os("CHAINWORKS_TEMP_ARTIFACT_LEGACY_TMP_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".chainworks").join("tmp"))
        });
    if let Some(legacy_root) = legacy_root.as_deref().and_then(canonicalize_managed_root) {
        roots.push(ScanRootTarget {
            path: legacy_root,
            root_kind: RootKind::LegacyChainworksTmp,
        });
    }

    roots
}

/// Canonicalizes a managed/legacy root before handing it to the descriptor-relative
/// no-follow scanner. These roots frequently sit under a symlinked ancestor on macOS
/// (`/tmp` -> `/private/tmp`, `/var` -> `/private/var`, and `tempfile::TempDir` paths
/// under `/var/folders/...` in tests), and the scanner's no-follow containment checks
/// treat an unresolved symlinked ancestor as an escape. Canonicalizing here — once,
/// against a config/env-resolved path rather than caller input — resolves the same way
/// `resolve_contained_test_root` already does for the operator-supplied override.
/// Returns `None` if the root doesn't exist or can't be resolved, in which case it is
/// simply omitted from this scan (never surfaced as a root_unreadable error) since a
/// managed root not existing yet (e.g. no cache has been written) is expected, not
/// exceptional.
fn canonicalize_managed_root(path: &Path) -> Option<PathBuf> {
    if !path.is_dir() {
        return None;
    }
    std::fs::canonicalize(path).ok()
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "temp_artifacts.inventory.preview" => {
            let dto = execute_inventory_preview(params, principal).await?;
            Ok(enforce_lane_parity_and_redaction("mcp", dto))
        }
        _ => Err(anyhow::anyhow!("Unknown temp_artifacts tool: {tool_name}")),
    }
}

/// Returns the canonical P089 inventory DTO for the run-scoped MCP resource lane.
///
/// This intentionally routes through the same request parser and scanner path as
/// the MCP tool so mode handling, validation, redaction, limits, dry-run flags,
/// permit guards, and mutation guards stay identical across readback lanes.
pub async fn inventory_preview_for_run_resource(
    run_id: &str,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let dto = execute_inventory_preview(
        serde_json::json!({
            "run_id": run_id,
            "include_dry_run": true
        }),
        principal,
    )
    .await?;
    Ok(enforce_lane_parity_and_redaction("mcp", dto))
}

/// Public entry point for other in-process lanes (GraphQL) to reuse the exact
/// mode-check, validation, redaction, permit-guard, and scanner path as the MCP
/// tool, so readback stays at parity across lanes. `params` uses the same
/// snake_case shape as the `temp_artifacts.inventory.preview` MCP tool input.
pub async fn inventory_preview(
    params: serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let dto = execute_inventory_preview(params, principal).await?;
    Ok(enforce_lane_parity_and_redaction("graphql", dto))
}

/// Same scan path as `inventory_preview`, but without recording a lane-parity
/// verdict — the caller is a lane other than `graphql` (e.g. `run_report`,
/// `release_receipt`) and records its own single verdict on the returned DTO via
/// `record_and_enforce_lane_parity`. Calling `inventory_preview` from those lanes
/// would spuriously also record a `graphql` parity data point for a request that
/// never actually went through the GraphQL resolver, misattributing readback
/// parity telemetry across lanes.
pub(crate) async fn inventory_preview_raw(
    params: serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    execute_inventory_preview(params, principal).await
}

/// Adapts `inventory_preview` to `graphql_server::types::temp_artifact_inventory::
/// TempArtifactInventoryBackend` so the daemon can install it into the GraphQL
/// schema at startup without graphql-server depending on mcp-server (mcp-server
/// already depends on graphql-server; this inverts the dependency cleanly).
pub struct McpTempArtifactInventoryBackend;

#[async_trait::async_trait]
impl graphql_server::types::temp_artifact_inventory::TempArtifactInventoryBackend
    for McpTempArtifactInventoryBackend
{
    async fn inventory_preview(
        &self,
        params: serde_json::Value,
        principal: &auth::Principal,
    ) -> Result<serde_json::Value> {
        inventory_preview(params, principal).await
    }
}

async fn execute_inventory_preview(
    params: serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    // Strict boolean parse for include_dry_run — reject non-bool values (SEC-P089-003).
    let include_dry_run = match params.get("include_dry_run") {
        None | Some(serde_json::Value::Null) => true,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(_) => return Ok(error_payload(InventoryErrorCode::Unknown.as_str(), true)),
    };

    // Strict integer parse for limit — reject non-integer values (SEC-P089-003).
    let limit: i32 = match params.get("limit") {
        None | Some(serde_json::Value::Null) => 500,
        Some(v) => match v.as_i64() {
            Some(n) if n >= i32::MIN as i64 && n <= i32::MAX as i64 => n as i32,
            _ => {
                return Ok(error_payload(
                    InventoryErrorCode::Unknown.as_str(),
                    include_dry_run,
                ))
            }
        },
    };

    // Strict integer parse for timeout_ms — reject non-integer values (SEC-P089-003).
    let timeout_ms: i32 = match params.get("timeout_ms") {
        None | Some(serde_json::Value::Null) => 5000,
        Some(v) => match v.as_i64() {
            Some(n) if n >= i32::MIN as i64 && n <= i32::MAX as i64 => n as i32,
            _ => {
                return Ok(error_payload(
                    InventoryErrorCode::Unknown.as_str(),
                    include_dry_run,
                ))
            }
        },
    };

    // Validate limit range (0..=500).
    if validate_inventory_limit(limit).is_err() {
        return Ok(error_payload(
            InventoryErrorCode::Unknown.as_str(),
            include_dry_run,
        ));
    }

    // Validate timeout_ms range (1..=5000).
    if validate_inventory_timeout_ms(timeout_ms).is_err() {
        return Ok(error_payload(
            InventoryErrorCode::Unknown.as_str(),
            include_dry_run,
        ));
    }

    // Strict string parse for test_root_override — reject non-string non-null values (SEC-P089-003).
    let test_root_override_str: Option<String> = match params.get("test_root_override") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(_) => {
            return Ok(error_payload(
                InventoryErrorCode::Unknown.as_str(),
                include_dry_run,
            ))
        }
    };

    // Mode check: return disabled_payload BEFORE any override format validation, caller-class
    // derivation, canonicalization, or containment resolution (SEC-P089-010). The disabled
    // kill switch must not touch the filesystem, authorize, or probe — regardless of override value.
    let mode = current_inventory_mode();
    if mode == InventoryMode::Disabled {
        return Ok(disabled_payload(include_dry_run));
    }

    // The request deadline starts here, before any canonicalize/is_dir filesystem work,
    // so a stalled mount cannot silently consume the advertised timeout_ms budget before
    // the timer starts running (SR-MEDIUM-001). `caller_class` derivation is pure/sync and
    // safe to compute on the async thread.
    let request_started = Instant::now();
    let scan_deadline_at =
        (Utc::now() + chrono::Duration::milliseconds(timeout_ms as i64)).to_rfc3339();
    let deadline = request_started + Duration::from_millis(timeout_ms as u64);
    let caller_class = auth::derive_caller_class_for_mcp(principal);

    // SEC-P089-HIGH-001: validate the override's format/authorization and the scope's
    // *shape* here — both pure, synchronous, I/O-free checks — so a permit can be
    // admitted before any filesystem call (canonicalize/is_dir) runs at all. Previously,
    // both `test_root_override` containment resolution and `workspace_context`
    // canonicalization ran unadmitted, ahead of the permit check that gates the scan
    // itself: a burst of requests against a stalled mount could pin blocking-pool
    // threads in that phase without ever being subject to `resource_exhausted`
    // admission control.
    let test_root_override_validated: Option<String> = match test_root_override_str.as_deref() {
        Some(override_str) => {
            // Format validation: absolute path, no NUL, no traversal, ≤4096 bytes.
            if validate_test_root_override(override_str).is_err() {
                return Ok(error_payload(
                    InventoryErrorCode::InvalidRootOverride.as_str(),
                    include_dry_run,
                ));
            }
            // Authorization: test_root_override requires automation or developer_break_glass caller class.
            if !matches!(
                caller_class,
                auth::CallerClass::Automation | auth::CallerClass::DeveloperBreakGlass
            ) {
                return Ok(error_payload(
                    InventoryErrorCode::InvalidRootOverride.as_str(),
                    include_dry_run,
                ));
            }
            Some(override_str.to_string())
        }
        None => None,
    };

    // For enabled modes: require a canonical run_id/workspace scope shape
    // (SEC-P089-001/005/006). Pure/I/O-free — see `parse_scope_shape`.
    let scope_shape = match parse_scope_shape(&params) {
        Ok(shape) => shape,
        Err(()) => {
            return Ok(error_payload(
                InventoryErrorCode::Unknown.as_str(),
                include_dry_run,
            ))
        }
    };

    // Use a scope-specific permit key so concurrent scans for different runs
    // each get their own context permit slot (SEC-P089-001). Derived from the
    // pre-canonicalization shape: for `workspace_context` this hashes the raw
    // requested path rather than its canonical form, which is fine here since
    // this key only buckets concurrency admission, not a security/redaction
    // boundary.
    let context_key = match &scope_shape {
        ScopeShape::RunId(id) => format!("run:{id}"),
        ScopeShape::WorkspaceContext(raw_workspace_root) => {
            let hash = domain::temp_artifact_inventory::compute_path_hash(
                raw_workspace_root.as_bytes(),
                RootKind::Unknown,
            );
            format!("workspace:{}", &hash[..12])
        }
    };

    // Acquire permit guard (resource_exhausted if permits are full) before any
    // filesystem work — including override/scope canonicalization below — is
    // admitted (SEC-P089-HIGH-001).
    let permit = match crate::tools::scanner::ScanPermitGuard::try_acquire(&context_key) {
        Ok(g) => g,
        Err(ScanPermitError::ResourceExhausted) => {
            db::metrics::record_p089_scan_rejected(
                "resource_exhausted",
                current_inventory_mode().as_str(),
            );
            return Ok(resource_exhausted_payload(include_dry_run));
        }
    };
    // Measured from request start to permit admission only — now that scope/override
    // canonicalization runs strictly after this point, this no longer conflates
    // resolution time with actual queueing/admission time.
    let queue_wait_ms = request_started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    db::metrics::record_p089_queue_wait(current_inventory_mode().as_str(), queue_wait_ms);

    // SEC-P089-HIGH-001: the permit moves INTO the blocking closure so capacity is
    // held for the true lifetime of the actual filesystem worker, not reclaimed the
    // moment the *request* gives up waiting on it (timeout/cancel/disconnect). A
    // stalled mount can pin at most one real worker per admitted permit — a
    // replacement can never be admitted for the same context/global slot while the
    // original worker is still running, because capacity is returned only when the
    // closure holding the permit actually returns (success, deliberate reject inside
    // the deadline, or panic-unwind), never by the async side racing it with a
    // deadline and reclaiming early.
    let remaining_before_resolve = deadline.saturating_duration_since(Instant::now());
    let mut resolve_handle = tokio::task::spawn_blocking(move || {
        let resolved = resolve_scope_and_override(scope_shape, test_root_override_validated);
        (permit, resolved)
    });
    let (permit, resolved) = tokio::select! {
        joined = &mut resolve_handle => {
            match joined {
                Ok(pair) => pair,
                Err(join_err) => {
                    // The panicking closure's stack unwound with the permit as a
                    // local variable, so it was already released there during
                    // unwind — nothing to reclaim on this side.
                    return Err(anyhow::anyhow!(
                        "scope/override resolution task panicked: {join_err}"
                    ));
                }
            }
        }
        _ = tokio::time::sleep(remaining_before_resolve) => {
            // The request gives up here, but the permit travels with the still-
            // running detached blocking closure and is released only when that
            // closure actually returns. Spawn a detached awaiter solely to record
            // when the real release happens — this does not affect admission
            // control, which already depends only on the closure's own Drop.
            tokio::spawn(async move {
                let _ = resolve_handle.await;
                db::metrics::record_p089_permit_reclaimed("context", "timeout");
                db::metrics::record_p089_permit_reclaimed("global", "timeout");
            });
            return Ok(timeout_payload(include_dry_run));
        }
    };
    let (scope, test_root_override_path) = match resolved {
        Ok(pair) => pair,
        Err(code) => {
            // Deliberate rejection of an already shape-validated request (e.g. a
            // workspace_context path that doesn't canonicalize to an existing
            // directory), observed within the deadline. The closure already handed
            // the permit back to us here, so release it promptly — a normal exit,
            // not a stuck/orphaned-resource reclaim.
            drop(permit);
            return Ok(error_payload(code.as_str(), include_dry_run));
        }
    };

    run_hidden_readback_scan(
        scope,
        test_root_override_path,
        limit,
        include_dry_run,
        request_started,
        deadline,
        scan_deadline_at,
        timeout_ms,
        permit,
        queue_wait_ms,
    )
    .await
}

/// Cheap, synchronous, I/O-free pre-parse of the request's scope *shape* — enough to
/// validate run_id/workspace_context exclusivity and format and to derive a stable
/// permit `context_key` — without any `canonicalize`/`is_dir` filesystem call. See
/// `canonicalize_scope` for the remaining filesystem-touching resolution, which now
/// runs only after a permit is admitted (SEC-P089-HIGH-001).
enum ScopeShape {
    RunId(String),
    WorkspaceContext(String),
}

/// Parses the enabled-mode scope shape from request params.
///
/// Workspace scope never scans the caller-provided root itself. It admits only
/// known Chainworks-managed descendants after the workspace root is validated
/// and canonicalized (in `canonicalize_scope`), preventing workspace_context from
/// becoming an arbitrary filesystem inventory primitive.
fn parse_scope_shape(params: &serde_json::Value) -> Result<ScopeShape, ()> {
    let has_run_id = params.get("run_id").map(|v| !v.is_null()).unwrap_or(false);
    let has_workspace = params
        .get("workspace_context")
        .map(|v| !v.is_null())
        .unwrap_or(false);

    match (has_run_id, has_workspace) {
        (true, true) => Err(()),
        (false, false) => Err(()),
        (true, false) => {
            match params["run_id"].as_str() {
                Some(id) if !id.is_empty() => {
                    // Validate run_id for path safety before accepting (SEC-P089-005):
                    // reject path separators, traversal, NUL bytes, and overlong values.
                    if validate_run_id(id).is_err() {
                        return Err(());
                    }
                    Ok(ScopeShape::RunId(id.to_string()))
                }
                _ => Err(()), // run_id not a non-empty string
            }
        }
        (false, true) => {
            let workspace_root = params["workspace_context"]
                .as_object()
                .filter(|object| object.len() == 1)
                .and_then(|object| object.get("workspace_root"))
                .and_then(serde_json::Value::as_str)
                .ok_or(())?;
            validate_test_root_override(workspace_root).map_err(|_| ())?;
            Ok(ScopeShape::WorkspaceContext(workspace_root.to_string()))
        }
    }
}

/// Resolves the test-root override and scope selector, including the synchronous
/// `canonicalize`/`is_dir` filesystem calls each requires. Called inside
/// `spawn_blocking`, bounded by the same request deadline via the caller's outer
/// `tokio::time::timeout` (SR-MEDIUM-001) — and, as of SEC-P089-HIGH-001, only
/// after a scan permit for this request's `context_key` is already held, so this
/// filesystem work is itself subject to admission control rather than running
/// unbounded ahead of it.
fn resolve_scope_and_override(
    scope_shape: ScopeShape,
    test_root_override_validated: Option<String>,
) -> Result<(ScopeSelector, Option<PathBuf>), InventoryErrorCode> {
    // Containment + canonical path resolution: path must be within the allowlist
    // (SEC-P089-002). Returns the canonical PathBuf to eliminate TOCTOU/
    // intermediate-symlink risk. Format validation and caller-class authorization
    // already ran, pure/I/O-free, before the permit was admitted.
    let test_root_override_path: Option<PathBuf> = match test_root_override_validated {
        Some(override_str) => match resolve_contained_test_root(&override_str) {
            Some(p) => Some(p),
            None => return Err(InventoryErrorCode::InvalidRootOverride),
        },
        None => None,
    };

    let scope = canonicalize_scope(scope_shape).map_err(|_| InventoryErrorCode::Unknown)?;

    Ok((scope, test_root_override_path))
}

/// Completes scope resolution from an already shape-validated `ScopeShape`,
/// performing the `canonicalize`/`is_dir` filesystem call `workspace_context`
/// requires (`run_id` needs none).
fn canonicalize_scope(shape: ScopeShape) -> Result<ScopeSelector, ()> {
    match shape {
        ScopeShape::RunId(id) => Ok(ScopeSelector::RunId(id)),
        ScopeShape::WorkspaceContext(workspace_root) => {
            let canonical = std::fs::canonicalize(&workspace_root).map_err(|_| ())?;
            if !canonical.is_dir() {
                return Err(());
            }
            Ok(ScopeSelector::WorkspaceContext(canonical))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_hidden_readback_scan(
    scope: ScopeSelector,
    test_root_override: Option<PathBuf>,
    limit: i32,
    include_dry_run: bool,
    request_started: Instant,
    deadline: Instant,
    scan_deadline_at: String,
    timeout_ms: i32,
    permit: crate::tools::scanner::ScanPermitGuard,
    queue_wait_ms: u64,
) -> Result<serde_json::Value> {
    let cancelled: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // SEC-P089-MED-001: wire cancellation to connection/request drop.
    // If the outer async future is dropped (HTTP disconnect, refresh supersede,
    // explicit cancel), this guard sets the flag so the blocking scan stops at its
    // next cooperative check interval (~128 entries or 100 ms). This is purely a
    // cooperative hint for the walker to exit sooner — it is independent of permit
    // lifetime, which is governed solely by when the blocking closure holding the
    // permit actually returns (see the SEC-P089-HIGH-001 note below).
    struct AbortScanOnDrop {
        cancelled: Arc<AtomicBool>,
        armed: bool,
    }
    impl AbortScanOnDrop {
        fn new(cancelled: Arc<AtomicBool>) -> Self {
            Self {
                cancelled,
                armed: true,
            }
        }

        fn disarm(&mut self) {
            self.armed = false;
        }

        fn cancel_without_transport_metric(&mut self) {
            self.cancelled.store(true, Ordering::Release);
            self.armed = false;
        }
    }
    impl Drop for AbortScanOnDrop {
        fn drop(&mut self) {
            if self.armed {
                self.cancelled.store(true, Ordering::Release);
                db::metrics::record_p089_cancel("transport_close", "cancelled");
            }
        }
    }

    if let Some(override_path) = test_root_override {
        // Diagnostic test root scan. The path is already canonical (SEC-P089-002).
        let target = ScanRootTarget {
            path: override_path,
            root_kind: RootKind::DiagnosticTestRoot,
        };
        let cancelled_clone = Arc::clone(&cancelled);
        // SEC-P089-HIGH-001: the permit moves INTO this closure so capacity is held
        // for the true duration of the blocking scan. If the request times out or is
        // dropped (disconnect/cancel/supersede) below, the permit is NOT reclaimed
        // early — it travels with the still-running detached closure and is
        // released only when that closure actually returns, so a stalled mount can
        // never let a replacement request get admitted for this slot while the
        // original worker is still alive.
        let mut handle = tokio::task::spawn_blocking(move || {
            let result =
                scan_diagnostic_test_root(&target, limit as usize, deadline, &cancelled_clone);
            (permit, result)
        });
        let mut abort_guard = AbortScanOnDrop::new(Arc::clone(&cancelled));
        // Bound the await itself with an outer wall-clock timeout tied to the request
        // deadline (SR-MEDIUM-001): cooperative checks inside the blocking task cannot
        // fire if a single filesystem syscall blocks indefinitely (e.g. a stalled mount).
        let remaining = deadline.saturating_duration_since(Instant::now());
        let result = tokio::select! {
            joined = &mut handle => {
                match joined {
                    Ok((_permit, result)) => result,
                    Err(join_err) => {
                        abort_guard.disarm();
                        return Err(anyhow::anyhow!("scan task panicked: {join_err}"));
                    }
                }
            }
            _ = tokio::time::sleep(remaining) => {
                abort_guard.cancel_without_transport_metric();
                // Spawn a detached awaiter solely to record when the real worker-lease
                // release happens, distinguishing it from this request's own timeout
                // in metrics. Admission control itself never depends on this — only
                // on the closure's own Drop of `permit` when it actually returns.
                tokio::spawn(async move {
                    let _ = handle.await;
                    db::metrics::record_p089_permit_reclaimed("context", "timeout");
                    db::metrics::record_p089_permit_reclaimed("global", "timeout");
                });
                let dto = timeout_payload(include_dry_run);
                record_scan_metrics(
                    &dto,
                    current_inventory_mode(),
                    request_started.elapsed(),
                );
                return Ok(dto);
            }
        };
        abort_guard.disarm();

        let dto = assemble_scan_dto(
            result,
            include_dry_run,
            limit,
            timeout_ms,
            queue_wait_ms,
            Some(scan_deadline_at),
            current_inventory_mode(),
        );
        record_scan_metrics(&dto, current_inventory_mode(), request_started.elapsed());
        Ok(dto)
    } else {
        // Root discovery (`discover_run_scoped_roots`/`discover_workspace_scoped_roots`)
        // performs canonicalize/is_dir filesystem calls; run it inside the same
        // `spawn_blocking` as the scan itself so it never blocks an async runtime worker
        // thread (SR-MEDIUM-001), bounded by the same outer request deadline as the scan.
        let scope_for_blocking = scope.clone();
        let cancelled_clone = Arc::clone(&cancelled);
        // SEC-P089-HIGH-001: see the diagnostic-test-root branch above — the permit
        // moves into this closure for the same reason.
        let mut handle = tokio::task::spawn_blocking(move || {
            let roots = match &scope_for_blocking {
                ScopeSelector::RunId(run_id) => discover_run_scoped_roots(run_id),
                ScopeSelector::WorkspaceContext(workspace_root) => {
                    discover_workspace_scoped_roots(workspace_root)
                }
            };
            let result = scan_multi_root(&roots, limit as usize, deadline, &cancelled_clone);
            (permit, result)
        });
        let mut abort_guard = AbortScanOnDrop::new(Arc::clone(&cancelled));
        // See the diagnostic-test-root branch above for why this outer timeout, and its
        // detached-completion/permit-release reasoning, is safe (SR-MEDIUM-001).
        let remaining = deadline.saturating_duration_since(Instant::now());
        let result = tokio::select! {
            joined = &mut handle => {
                match joined {
                    Ok((_permit, result)) => result,
                    Err(join_err) => {
                        abort_guard.disarm();
                        return Err(anyhow::anyhow!(
                            "production root scan task panicked: {join_err}"
                        ));
                    }
                }
            }
            _ = tokio::time::sleep(remaining) => {
                abort_guard.cancel_without_transport_metric();
                tokio::spawn(async move {
                    let _ = handle.await;
                    db::metrics::record_p089_permit_reclaimed("context", "timeout");
                    db::metrics::record_p089_permit_reclaimed("global", "timeout");
                });
                let dto = timeout_payload(include_dry_run);
                record_scan_metrics(
                    &dto,
                    current_inventory_mode(),
                    request_started.elapsed(),
                );
                return Ok(dto);
            }
        };
        abort_guard.disarm();
        let dto = assemble_scan_dto(
            result,
            include_dry_run,
            limit,
            timeout_ms,
            queue_wait_ms,
            Some(scan_deadline_at),
            current_inventory_mode(),
        );
        record_scan_metrics(&dto, current_inventory_mode(), request_started.elapsed());
        Ok(dto)
    }
}

fn record_scan_metrics(dto: &serde_json::Value, mode: InventoryMode, duration: Duration) {
    let status = dto
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let mut root_kinds = std::collections::BTreeSet::new();
    if let Some(rows) = dto.get("rows").and_then(serde_json::Value::as_array) {
        for row in rows {
            let root_kind = row
                .get("root_kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            root_kinds.insert(root_kind);
            let estimated_bytes = row
                .get("estimated_size_bytes")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            db::metrics::record_p089_inventory_row(
                root_kind,
                row.get("manifest_state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown"),
                row.get("lifecycle_classification")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown"),
                row.get("artifact_kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown"),
                estimated_bytes,
            );
            if let Some(recommendation) = row
                .get("dry_run_recommendation")
                .and_then(serde_json::Value::as_str)
            {
                db::metrics::record_p089_dry_run_recommendation(recommendation);
            }
        }
    }
    if let Some(errors) = dto.get("errors").and_then(serde_json::Value::as_array) {
        for error in errors {
            let root_kind = error
                .get("root_kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            root_kinds.insert(root_kind);
            db::metrics::record_p089_scan_error(
                error
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown"),
                root_kind,
                mode.as_str(),
            );
        }
    }
    if root_kinds.is_empty() {
        root_kinds.insert("unknown");
    }
    let duration_ms = duration.as_millis().min(u64::MAX as u128) as u64;
    for root_kind in root_kinds {
        db::metrics::record_p089_inventory_scan(status, root_kind, mode.as_str(), duration_ms);
    }
    if status == InventoryStatus::Timeout.as_str() {
        db::metrics::record_p089_deadline_exceeded("enumeration", mode.as_str());
    } else if status == InventoryStatus::Cancelled.as_str() {
        // Distinguish the one cancellation source this call site can actually
        // observe (graceful daemon shutdown) from the rest, which collapse to
        // transport_close until explicit-cancel/supersede signals are threaded
        // in from the GraphQL/MCP caller layer.
        let source = if crate::tools::scanner::is_global_shutdown_requested() {
            "daemon_shutdown"
        } else {
            "transport_close"
        };
        db::metrics::record_p089_cancel(source, status);
    }
    db::metrics::record_p089_mutation_guard(
        dto.get("mutation_guard")
            .and_then(|guard| guard.get("status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown"),
    );
    db::metrics::record_p089_metric_health("pass");
}

/// Returns whether every row's `path_display` in `dto` is redacted text. Shared by
/// `enforce_lane_parity_and_redaction` (mcp/graphql lanes) and the run_report/
/// release_receipt projections in `reports.rs`, so every readback lane derives its
/// parity/redaction verdict from the same check on the actual DTO rather than each
/// lane independently deciding (or hardcoding) its own answer.
pub(crate) fn dto_redaction_is_safe(dto: &serde_json::Value) -> bool {
    // A missing or non-array `rows`/`errors` is not "vacuously safe" — it means the
    // DTO's shape could not be confirmed, so it must fail closed rather than pass
    // through (this is the single shared check all four lanes derive their verdict
    // from). Validates the complete canonical row shape — path_display, path_hash,
    // path_hash_short, and correlation_key — rather than path_display alone
    // (SEC-P089-MED-001): a malformed DTO with a valid-looking path_display but
    // sensitive data in another projected field must still fail closed, since
    // GraphQL/reports expose every one of these fields and Swift copies path_hash
    // to the system pasteboard.
    let Some(rows) = dto.get("rows").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let Some(errors) = dto.get("errors").and_then(serde_json::Value::as_array) else {
        return false;
    };
    rows.iter().all(is_row_redaction_safe) && errors.iter().all(is_error_redaction_safe)
}

/// Validates every projected string field on one row, not only `path_display`.
fn is_row_redaction_safe(row: &serde_json::Value) -> bool {
    let Some(path_display) = row.get("path_display").and_then(serde_json::Value::as_str) else {
        return false;
    };
    if !is_valid_redacted_path_display(path_display) {
        return false;
    }
    let Some(path_hash) = row.get("path_hash").and_then(serde_json::Value::as_str) else {
        return false;
    };
    if !is_valid_full_path_hash(path_hash) {
        return false;
    }
    let Some(path_hash_short) = row
        .get("path_hash_short")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    if !is_valid_path_hash_short(path_hash_short, path_hash) {
        return false;
    }
    // correlation_key must be exactly path_hash whenever a row carries a valid hash
    // (path_hash_contract.correlation_key: "Use path_hash when present"). A
    // divergent correlation_key on a hashed row would mean some other, unvalidated
    // value — potentially raw or partially raw — is being projected instead.
    match row
        .get("correlation_key")
        .and_then(serde_json::Value::as_str)
    {
        Some(key) => key == path_hash,
        None => false,
    }
}

/// Validates a row-level or top-level error entry's `message` is the exact
/// redacted literal every scanner/DTO-builder call site emits. Any deviation
/// indicates unredacted content reached this boundary.
fn is_error_redaction_safe(error: &serde_json::Value) -> bool {
    matches!(
        error.get("message").and_then(serde_json::Value::as_str),
        Some("<redacted>")
    )
}

/// Validates a full `path_hash`: exactly `PATH_HASH_HEX_LEN` lowercase hex characters.
fn is_valid_full_path_hash(hash: &str) -> bool {
    use domain::temp_artifact_inventory::PATH_HASH_HEX_LEN;
    hash.len() == PATH_HASH_HEX_LEN && is_lowercase_hex(hash)
}

/// Validates `path_hash_short`: bounded length, lowercase hex, and a genuine
/// prefix of the row's full `path_hash` (per the collision-resolution derivation
/// in `path_hash_contract.path_hash_short_derivation`) rather than an unrelated
/// value that merely happens to look like hex.
fn is_valid_path_hash_short(short: &str, full: &str) -> bool {
    use domain::temp_artifact_inventory::{PATH_HASH_SHORT_MAX_LEN, PATH_HASH_SHORT_MIN_LEN};
    if short.len() < PATH_HASH_SHORT_MIN_LEN || short.len() > PATH_HASH_SHORT_MAX_LEN {
        return false;
    }
    is_lowercase_hex(short) && full.starts_with(short)
}

fn is_lowercase_hex(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Validates the exact `path_display` redaction syntax: `<redacted:` + a
/// `PATH_HASH_SHORT_MIN_LEN..=PATH_HASH_SHORT_MAX_LEN`-char lowercase-hex path hash
/// + `>`, with nothing before, after, or interleaved. A prefix-only check (formerly
/// `starts_with("<redacted")`) would accept a string like `"<redacted:ab12> /Users/
/// user/secret/path"` — the trailing raw path passes a prefix check but leaks the
/// real filesystem path.
fn is_valid_redacted_path_display(display: &str) -> bool {
    use domain::temp_artifact_inventory::{PATH_HASH_SHORT_MAX_LEN, PATH_HASH_SHORT_MIN_LEN};
    let Some(rest) = display.strip_prefix("<redacted:") else {
        return false;
    };
    let Some(hash) = rest.strip_suffix('>') else {
        return false;
    };
    if hash.len() < PATH_HASH_SHORT_MIN_LEN || hash.len() > PATH_HASH_SHORT_MAX_LEN {
        return false;
    }
    hash.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Builds a safe `internal_error` payload to substitute for a DTO whose redaction
/// check failed, preserving whether dry_run info was requested.
pub(crate) fn redaction_failure_error_payload(dto: &serde_json::Value) -> serde_json::Value {
    let include_dry_run = !dto
        .get("dry_run")
        .map(serde_json::Value::is_null)
        .unwrap_or(true);
    error_payload(InventoryErrorCode::InternalError.as_str(), include_dry_run)
}

/// Records `lane`'s parity verdict from the actual `dto` content and returns either
/// the DTO unchanged (safe) or a safe substitute (unsafe) — the single fail-closed
/// rule every readback lane (mcp, graphql, run_report, release_receipt) applies to
/// a DTO it is about to hand to a caller, whether freshly scanned or reused from an
/// already-computed sibling-lane result. No caller may hardcode a "pass" verdict:
/// that would keep recording success even if the reused/passed-in DTO were unsafe.
pub(crate) fn record_and_enforce_lane_parity(
    lane: &str,
    dto: &serde_json::Value,
) -> serde_json::Value {
    let safe = dto_redaction_is_safe(dto);
    db::metrics::record_p089_readback_parity(lane, if safe { "pass" } else { "fail" });
    if safe {
        dto.clone()
    } else {
        db::metrics::record_p089_redaction_failure(lane);
        redaction_failure_error_payload(dto)
    }
}

/// Validates that every row's `path_display` is redacted text before the DTO is allowed
/// to leave this process boundary on the given lane. Unlike a detect-only check, this
/// fails closed: a redaction failure discards the (potentially unsafe) DTO entirely and
/// substitutes a safe `internal_error` payload, rather than returning unredacted content
/// with only a metric recorded (audit/security-review fail-open redaction defect).
fn enforce_lane_parity_and_redaction(lane: &str, dto: serde_json::Value) -> serde_json::Value {
    let redaction_is_safe = dto_redaction_is_safe(&dto);
    db::metrics::record_p089_readback_parity(lane, if redaction_is_safe { "pass" } else { "fail" });
    if redaction_is_safe {
        dto
    } else {
        db::metrics::record_p089_redaction_failure(lane);
        redaction_failure_error_payload(&dto)
    }
}

/// Resolves the Chainworks meta root directory from env vars or known path conventions.
fn resolve_meta_root() -> Option<std::path::PathBuf> {
    // 1. Explicit env override
    if let Ok(val) = std::env::var("CHAINWORKS_META_ROOT") {
        let p = std::path::Path::new(&val);
        if p.is_absolute() {
            return Some(p.to_path_buf());
        }
    }

    // 2. Derive from DATABASE_URL (sqlite:///path/to/file.db?...)
    if let Some(root) = derive_meta_root_from_database_url() {
        return Some(root);
    }

    // 3. $HOME/.chainworks
    if let Some(home) = std::env::var("HOME").ok() {
        let p = std::path::PathBuf::from(home).join(".chainworks");
        if p.is_dir() {
            return Some(p);
        }
    }

    None
}

/// Parses the parent directory of the sqlite DB path from DATABASE_URL.
fn derive_meta_root_from_database_url() -> Option<std::path::PathBuf> {
    let db_url = std::env::var("DATABASE_URL").ok()?;
    // Format: sqlite:///absolute/path/file.db or sqlite:///absolute/path/file.db?options
    let path_part = db_url.strip_prefix("sqlite://")?;
    let path_str = path_part.split('?').next()?;
    let p = std::path::Path::new(path_str);
    if p.is_absolute() {
        p.parent().map(|parent| parent.to_path_buf())
    } else {
        None
    }
}

fn resource_exhausted_payload(include_dry_run: bool) -> serde_json::Value {
    let now = Utc::now().to_rfc3339();
    json!({
        "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
        "status": InventoryStatus::ResourceExhausted.as_str(),
        "enabled_state": EnabledState::Unknown.as_str(),
        "mode": current_inventory_mode().as_str(),
        "disabled_reason_code": null,
        "generated_at": now,
        "limits_applied": {
            "limit": 0,
            "timeout_ms": 0,
            "scan_deadline_at": null,
            "queue_wait_ms": 0
        },
        "summary": {
            "artifact_tree_count": 0,
            "estimated_bytes": "0",
            "active_or_recent_count": 0,
            "terminal_candidate_count": 0,
            "orphan_candidate_count": 0,
            "legacy_unmanaged_count": 0,
            "scan_error_count": 0,
            "dry_run_candidate_count": 0,
            "truncated": false,
            "queue_wait_ms": 0
        },
        "rows": [],
        "errors": [{
            "code": InventoryErrorCode::ResourceExhausted.as_str(),
            "message": "<redacted>",
            "root_kind": null,
            "phase": null
        }],
        "dry_run": if include_dry_run {
            json!({
                "schema_version": "temp_artifact_dry_run_v1",
                "generated_at": now,
                "recommendation_counts": {},
                "mutation_guard": {
                    "status": MutationGuardStatus::Skipped.as_str(),
                    "checked_at": now
                }
            })
        } else {
            json!(null)
        },
        "mutation_guard": {
            "status": MutationGuardStatus::Skipped.as_str(),
            "checked_at": now,
            "no_delete": true,
            "no_prune": true,
            "no_chmod": true,
            "no_persist": true,
            "no_retry": true
        }
    })
}

/// Serializes tests anywhere in this crate that touch
/// `CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE` (a process-global env var), so
/// concurrent tests in different modules — e.g. `tools::reports` tests that now
/// depend on this crate's live inventory path — can't observe each other's
/// transient mode changes. `unwrap_or_else` recovers from poisoning rather than
/// letting one panicking test cascade-fail every other test sharing this lock.
#[cfg(test)]
pub(crate) fn temp_artifact_inventory_env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_RUN_ID: &str = "2c74aef7-739c-4ac6-baa0-f67ca36cc7ef";
    const OTHER_RUN_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const MISSING_RUN_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    fn operator_principal() -> auth::Principal {
        auth::Principal::new("test-operator", domain::PrincipalClass::Operator)
    }

    fn automation_principal() -> auth::Principal {
        let mut p = auth::Principal::new("test-automation", domain::PrincipalClass::Operator);
        p.caller_class_override = Some(auth::CallerClass::Automation);
        p
    }

    struct IsolatedManagedRoots {
        _cache: tempfile::TempDir,
        _provider_home: tempfile::TempDir,
        _legacy: tempfile::TempDir,
    }

    impl Drop for IsolatedManagedRoots {
        fn drop(&mut self) {
            std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_CONTROL_PLANE_CACHE_ROOT");
            std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_PROVIDER_HOME_FALLBACK_ROOT");
            std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_LEGACY_TMP_ROOT");
        }
    }

    fn isolate_managed_roots() -> IsolatedManagedRoots {
        let roots = IsolatedManagedRoots {
            _cache: tempfile::TempDir::new().expect("isolated cache root"),
            _provider_home: tempfile::TempDir::new().expect("isolated provider-home root"),
            _legacy: tempfile::TempDir::new().expect("isolated legacy root"),
        };
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_CONTROL_PLANE_CACHE_ROOT",
            roots._cache.path(),
        );
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_PROVIDER_HOME_FALLBACK_ROOT",
            roots._provider_home.path(),
        );
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_LEGACY_TMP_ROOT",
            roots._legacy.path(),
        );
        roots
    }

    #[test]
    fn p089_temp_artifacts_tool_spec_has_correct_name() {
        let specs = tool_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "temp_artifacts.inventory.preview");
    }

    #[test]
    fn p089_temp_artifacts_tool_spec_input_schema_is_object() {
        let specs = tool_specs();
        assert_eq!(specs[0].input_schema["type"], "object");
    }

    #[test]
    fn p089_temp_artifacts_tool_spec_input_schema_requires_exactly_one_selector() {
        // Regression: the schema must express "exactly one of run_id / workspace_context"
        // declaratively via oneOf, not leave the top-level `required` empty and defer
        // entirely to runtime validation.
        let specs = tool_specs();
        let one_of = specs[0].input_schema["oneOf"]
            .as_array()
            .expect("oneOf must be present and an array");
        assert_eq!(one_of.len(), 2);
        assert!(one_of
            .iter()
            .any(|branch| branch["required"] == serde_json::json!(["run_id"])));
        assert!(one_of
            .iter()
            .any(|branch| branch["required"] == serde_json::json!(["workspace_context"])));
    }

    #[test]
    fn p089_temp_artifacts_tool_spec_declares_output_schema() {
        // Regression: the MCP tool must publish its result shape rather than
        // advertising `output_schema: None`, so callers/fixtures can validate
        // against the exact canonical DTO contract.
        let specs = tool_specs();
        let output_schema = specs[0]
            .output_schema
            .as_ref()
            .expect("output_schema must be declared");
        let required = output_schema["required"]
            .as_array()
            .expect("output_schema.required must be an array")
            .iter()
            .map(|v| v.as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        for field in [
            "schema_version",
            "status",
            "enabled_state",
            "generated_at",
            "limits_applied",
            "summary",
            "rows",
            "errors",
            "mutation_guard",
        ] {
            assert!(
                required.contains(&field),
                "output_schema.required must include {field}"
            );
        }
    }

    #[tokio::test]
    async fn p089_disabled_mode_payload_has_correct_status() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("disabled mode must not error");
        assert_eq!(result["status"], "disabled");
        assert_eq!(result["enabled_state"], "disabled");
        assert_eq!(
            result["schema_version"],
            TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION
        );
    }

    #[tokio::test]
    async fn p089_disabled_mode_payload_rows_is_empty() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert!(result["rows"]
            .as_array()
            .expect("rows must be array")
            .is_empty());
        assert!(result["errors"]
            .as_array()
            .expect("errors must be array")
            .is_empty());
    }

    #[tokio::test]
    async fn p089_disabled_mode_mutation_guard_no_delete() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        let guard = &result["mutation_guard"];
        assert_eq!(guard["no_delete"], true);
        assert_eq!(guard["no_prune"], true);
        assert_eq!(guard["no_chmod"], true);
        assert_eq!(guard["no_persist"], true);
        assert_eq!(guard["no_retry"], true);
    }

    #[tokio::test]
    async fn p089_include_dry_run_false_makes_dry_run_null() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"include_dry_run": false});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert!(result["dry_run"].is_null());
    }

    #[tokio::test]
    async fn p089_include_dry_run_true_includes_dry_run() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"include_dry_run": true});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert!(!result["dry_run"].is_null());
        assert_eq!(
            result["dry_run"]["schema_version"],
            "temp_artifact_dry_run_v1"
        );
    }

    #[test]
    fn p089_current_inventory_mode_defaults_to_disabled() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        assert_eq!(current_inventory_mode(), InventoryMode::Disabled);
    }

    #[test]
    fn p089_current_inventory_mode_reads_env_var() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        let mode = current_inventory_mode();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        assert_eq!(mode, InventoryMode::HiddenReadback);
    }

    #[test]
    fn p089_current_inventory_mode_unknown_value_defaults_to_disabled() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "bogus_value");
        let mode = current_inventory_mode();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        assert_eq!(mode, InventoryMode::Disabled);
    }

    #[tokio::test]
    async fn p089_disabled_payload_limits_applied_has_required_fields() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        let limits = &result["limits_applied"];
        assert!(
            limits.get("limit").is_some(),
            "limits_applied.limit required"
        );
        assert!(
            limits.get("timeout_ms").is_some(),
            "limits_applied.timeout_ms required"
        );
        assert!(
            limits.get("queue_wait_ms").is_some(),
            "limits_applied.queue_wait_ms required"
        );
    }

    #[tokio::test]
    async fn p089_disabled_payload_schema_version_matches_constant() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(
            result["schema_version"].as_str().unwrap_or(""),
            TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION
        );
    }

    #[tokio::test]
    async fn p089_invalid_limit_above_max_returns_error_status() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"limit": 501});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "error");
        let errors = result["errors"].as_array().expect("errors array");
        assert!(!errors.is_empty());
        // Uses canonical unknown code — limit_out_of_range is not in contract error_code enum.
        assert_eq!(errors[0]["code"], "unknown");
        assert!(errors[0]["phase"].is_null());
    }

    #[tokio::test]
    async fn p089_invalid_limit_negative_returns_error_status() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"limit": -1});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "unknown");
    }

    #[tokio::test]
    async fn p089_invalid_timeout_ms_zero_returns_error_status() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"timeout_ms": 0});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "error");
        // Uses canonical unknown code — timeout_ms_out_of_range is not in contract error_code enum.
        assert_eq!(result["errors"][0]["code"], "unknown");
    }

    #[tokio::test]
    async fn p089_invalid_timeout_ms_above_max_returns_error_status() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"timeout_ms": 5001});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "unknown");
    }

    // These two tests now prove the SEC-P089-010 fix: disabled mode fires BEFORE override
    // format validation, so even a string override that would fail path validation in enabled
    // mode returns disabled (not error) in disabled mode — no filesystem access occurs.
    #[tokio::test]
    async fn p089_disabled_mode_with_relative_override_returns_disabled_not_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"test_root_override": "relative/path"});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        assert_eq!(
            result["status"], "disabled",
            "SEC-P089-010: disabled mode must not run format validation on override"
        );
        assert_eq!(result["enabled_state"], "disabled");
    }

    #[tokio::test]
    async fn p089_disabled_mode_with_traversal_override_returns_disabled_not_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"test_root_override": "/tmp/foo/../etc"});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        assert_eq!(
            result["status"], "disabled",
            "SEC-P089-010: disabled mode must not run traversal check on override"
        );
        assert_eq!(result["enabled_state"], "disabled");
    }

    // SEC-P089-010 regression tests: disabled mode with various override values must never
    // call canonicalize, derive caller class, or check containment.
    #[tokio::test]
    async fn p089_disabled_mode_with_absolute_override_returns_disabled_without_fs_access() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"test_root_override": "/tmp/chainworks-test-disabled"});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        assert_eq!(
            result["status"], "disabled",
            "SEC-P089-010: disabled mode must return disabled before any filesystem access"
        );
        assert_eq!(result["enabled_state"], "disabled");
    }

    #[tokio::test]
    async fn p089_disabled_mode_with_allowlisted_override_returns_disabled() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let dir_path = dir.path().to_str().expect("valid path").to_string();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", &dir_path);
        let params = serde_json::json!({"test_root_override": dir_path});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");
        assert_eq!(result["status"], "disabled",
            "SEC-P089-010: disabled mode must return disabled even when override would be valid in enabled mode");
    }

    #[tokio::test]
    async fn p089_disabled_mode_with_outside_allowlist_override_returns_disabled() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let allowlist_dir = tempfile::TempDir::new().expect("allowlist dir");
        let outside_dir = tempfile::TempDir::new().expect("outside dir");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS",
            allowlist_dir.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"test_root_override": outside_dir.path().to_str().expect("valid path")});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");
        assert_eq!(
            result["status"], "disabled",
            "SEC-P089-010: disabled mode must return disabled without running containment checks"
        );
    }

    #[tokio::test]
    async fn p089_valid_inputs_still_return_disabled_payload() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"limit": 500, "timeout_ms": 5000, "include_dry_run": true});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "disabled");
    }

    #[tokio::test]
    async fn p089_error_payload_mutation_guard_never_attests_pass() {
        // Regression: an error payload must not claim the mutation guard ran and
        // passed. The guard never executed for a request that errored before the
        // scan phase, so its status must be `skipped` and its `no_*` evidence must
        // default closed (`false`), never `true` — a `true` default would let a
        // durable run_report/release_receipt record "mutation protections passed"
        // for a request that never checked them.
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"limit": 999});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "error");
        let guard = &result["mutation_guard"];
        assert_eq!(guard["status"], "skipped");
        assert_eq!(guard["no_delete"], false);
        assert_eq!(guard["no_prune"], false);
        assert_eq!(guard["no_chmod"], false);
        assert_eq!(guard["no_persist"], false);
        assert_eq!(guard["no_retry"], false);
    }

    #[test]
    fn p089_enforce_lane_parity_and_redaction_fails_closed_on_unsafe_path_display() {
        // Regression for the fail-open redaction defect: a malformed/unsafe path_display
        // must not be returned to the caller with only a metric recorded — the DTO itself
        // must be replaced with a safe error payload.
        let unsafe_dto = serde_json::json!({
            "status": "complete",
            "dry_run": {"schema_version": "temp_artifact_dry_run_v1"},
            "rows": [
                {"path_display": "/Users/someone/not-redacted/path"}
            ]
        });
        let result = enforce_lane_parity_and_redaction("mcp", unsafe_dto);
        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "internal_error");
        assert_eq!(
            result["rows"].as_array().map(Vec::len),
            Some(0),
            "unsafe rows must not be forwarded, even inside an error payload"
        );
        // include_dry_run was derived from the (unsafe) input DTO having a non-null dry_run.
        assert!(!result["dry_run"].is_null());
    }

    #[test]
    fn p089_enforce_lane_parity_and_redaction_passes_through_safe_dto() {
        let safe_dto = serde_json::json!({
            "status": "complete",
            "dry_run": serde_json::Value::Null,
            "errors": [],
            "rows": [
                {
                    "path_display": "<redacted:abababababab>",
                    "path_hash": "ab".repeat(32),
                    "path_hash_short": "abababababab",
                    "correlation_key": "ab".repeat(32)
                }
            ]
        });
        let result = enforce_lane_parity_and_redaction("graphql", safe_dto.clone());
        assert_eq!(result, safe_dto, "safe DTOs must pass through unchanged");
    }

    #[test]
    fn p089_dto_redaction_is_safe_rejects_missing_or_non_array_rows() {
        // Regression: a missing/non-array `rows` must fail closed, not be treated as
        // vacuously safe. `is_none_or` previously accepted both cases.
        assert!(!dto_redaction_is_safe(
            &serde_json::json!({"status": "complete"})
        ));
        assert!(!dto_redaction_is_safe(
            &serde_json::json!({"status": "complete", "rows": "not-an-array"})
        ));
        assert!(!dto_redaction_is_safe(
            &serde_json::json!({"status": "complete", "rows": null})
        ));
    }

    #[test]
    fn p089_dto_redaction_is_safe_rejects_trailing_raw_path_after_valid_prefix() {
        // Regression: a bare prefix check (`starts_with("<redacted")`) would accept
        // a redacted-looking prefix followed by a raw, unredacted path. The exact
        // syntax check must reject anything after the closing '>'.
        assert!(!dto_redaction_is_safe(&serde_json::json!({
            "status": "complete",
            "rows": [{"path_display": "<redacted:abc123abc123> /Users/user/secret/path"}]
        })));
    }

    #[test]
    fn p089_dto_redaction_is_safe_rejects_short_and_non_hex_hashes() {
        // A hash shorter than PATH_HASH_SHORT_MIN_LEN (12) or containing non-hex
        // characters must not pass — both would have passed the old prefix check.
        assert!(!dto_redaction_is_safe(&serde_json::json!({
            "status": "complete",
            "rows": [{"path_display": "<redacted:abc>"}]
        })));
        assert!(!dto_redaction_is_safe(&serde_json::json!({
            "status": "complete",
            "rows": [{"path_display": "<redacted:not-hex-chars>"}]
        })));
    }

    #[test]
    fn p089_dto_redaction_is_safe_rejects_malformed_full_path_hash() {
        // SEC-P089-MED-001 regression: a row with a valid-looking `path_display`
        // but a malformed/non-hex/wrong-length `path_hash` must fail closed, not
        // pass through on `path_display` alone.
        assert!(!dto_redaction_is_safe(&serde_json::json!({
            "status": "complete",
            "errors": [],
            "rows": [{
                "path_display": "<redacted:abababababab>",
                "path_hash": "not-a-real-hash",
                "path_hash_short": "abababababab",
                "correlation_key": "not-a-real-hash"
            }]
        })));
        assert!(
            !dto_redaction_is_safe(&serde_json::json!({
                "status": "complete",
                "errors": [],
                "rows": [{"path_display": "<redacted:abababababab>"}]
            })),
            "a row missing path_hash entirely must fail closed"
        );
    }

    #[test]
    fn p089_dto_redaction_is_safe_rejects_correlation_key_diverging_from_path_hash() {
        // SEC-P089-MED-001 regression: `correlation_key` must equal `path_hash`
        // per the path_hash_contract. A divergent value could be an unvalidated
        // (potentially raw) field smuggled through a projection defect.
        let full_hash = "ab".repeat(32);
        assert!(!dto_redaction_is_safe(&serde_json::json!({
            "status": "complete",
            "errors": [],
            "rows": [{
                "path_display": "<redacted:abababababab>",
                "path_hash": full_hash,
                "path_hash_short": "abababababab",
                "correlation_key": "/Users/someone/leaked/path"
            }]
        })));
    }

    #[test]
    fn p089_dto_redaction_is_safe_rejects_unredacted_error_message() {
        // SEC-P089-MED-001 regression: `errors[*].message` must be exactly the
        // `<redacted>` literal every scanner/DTO-builder call site emits; any
        // other content indicates unredacted text reached this lane boundary.
        assert!(!dto_redaction_is_safe(&serde_json::json!({
            "status": "error",
            "errors": [{"code": "internal_error", "message": "/Users/someone/real/path failed"}],
            "rows": []
        })));
    }

    #[test]
    fn p089_dto_redaction_is_safe_accepts_valid_hash_lengths() {
        // 12-20 char lowercase hex hashes (the full compute_path_hash_short range)
        // must all be accepted.
        for len in [12usize, 14, 16, 18, 20] {
            let short_hash = "a".repeat(len);
            let full_hash = "a".repeat(64);
            assert!(
                dto_redaction_is_safe(&serde_json::json!({
                    "status": "complete",
                    "errors": [],
                    "rows": [{
                        "path_display": format!("<redacted:{}>", short_hash),
                        "path_hash": full_hash,
                        "path_hash_short": short_hash,
                        "correlation_key": full_hash
                    }]
                })),
                "hash length {len} must be accepted"
            );
        }
    }

    #[test]
    fn p089_record_and_enforce_lane_parity_fails_closed_for_reused_unsafe_dto() {
        // Regression for the reports.rs fail-open defect: lanes that reuse an
        // already-computed DTO (run_report/release_receipt, and the N+1-amplification
        // reused-DTO path in artifact_report_json_with_temp_artifact_inventory) must
        // derive their own parity verdict from the actual DTO content, not hardcode
        // "pass" — and must substitute a safe payload rather than forward unsafe rows.
        let unsafe_dto = serde_json::json!({
            "status": "complete",
            "dry_run": serde_json::Value::Null,
            "rows": [
                {"path_display": "/Users/someone/not-redacted/path"}
            ]
        });
        let result = record_and_enforce_lane_parity("run_report", &unsafe_dto);
        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "internal_error");
        assert_eq!(result["rows"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn p089_record_and_enforce_lane_parity_passes_through_safe_reused_dto() {
        let safe_dto = serde_json::json!({
            "status": "complete",
            "dry_run": serde_json::Value::Null,
            "errors": [],
            "rows": [
                {
                    "path_display": "<redacted:abababababab>",
                    "path_hash": "ab".repeat(32),
                    "path_hash_short": "abababababab",
                    "correlation_key": "ab".repeat(32)
                }
            ]
        });
        let result = record_and_enforce_lane_parity("release_receipt", &safe_dto);
        assert_eq!(
            result, safe_dto,
            "safe reused DTOs must pass through unchanged"
        );
    }

    #[test]
    fn p089_timeout_payload_has_deadline_exceeded_status_and_error_code() {
        // Regression for SR-MEDIUM-001: when scope/override resolution itself
        // exceeds the request deadline (before the scan phase is reached), the
        // response must carry status=timeout and a deadline_exceeded error, not
        // silently fall through to a scan result.
        let dto = timeout_payload(true);
        assert_eq!(dto["status"], InventoryStatus::Timeout.as_str());
        assert_eq!(dto["rows"].as_array().map(Vec::len), Some(0));
        let errors = dto["errors"].as_array().expect("errors array");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0]["code"],
            InventoryErrorCode::DeadlineExceeded.as_str()
        );
    }

    #[tokio::test]
    async fn p089_resolve_scope_and_override_rejects_relative_workspace_root_before_spawn_blocking()
    {
        // Confirms the spawn_blocking-wrapped resolution path (added for
        // SR-MEDIUM-001) still enforces the same validation as before the refactor:
        // a non-canonicalizable / relative workspace root must return an error
        // DTO, not a panic or a successful scan.
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        let params = serde_json::json!({
            "workspace_context": {"workspace_root": "relative/not/absolute"}
        });
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("must not error at the Result level");
        assert_eq!(result["status"], "error");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
    }

    #[tokio::test]
    async fn p089_disabled_payload_summary_has_all_required_metrics() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        let summary = &result["summary"];
        for field in &[
            "artifact_tree_count",
            "estimated_bytes",
            "active_or_recent_count",
            "terminal_candidate_count",
            "orphan_candidate_count",
            "legacy_unmanaged_count",
            "scan_error_count",
            "dry_run_candidate_count",
            "truncated",
            "queue_wait_ms",
        ] {
            assert!(summary.get(*field).is_some(), "summary.{field} required");
        }
        assert_eq!(
            summary["estimated_bytes"].as_str().unwrap_or(""),
            "0",
            "estimated_bytes must be ByteCountString '0'"
        );
    }

    #[tokio::test]
    async fn p089_hidden_readback_without_test_root_returns_complete_enabled() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        // Point CHAINWORKS_META_ROOT at a temp dir with no subdirs so roots are empty
        let empty_meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            empty_meta.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": TEST_RUN_ID});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(result["status"], "complete");
        assert_eq!(result["enabled_state"], "enabled");
        assert_eq!(
            result["schema_version"],
            TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION
        );
        let rows = result["rows"].as_array().expect("rows array");
        assert!(rows.is_empty(), "empty meta root → zero rows");
    }

    #[tokio::test]
    async fn p089_hidden_readback_without_test_root_scans_requested_run_dir() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        let meta_root = tempfile::TempDir::new().expect("temp meta dir");
        let run_dir = meta_root.path().join("runs").join(TEST_RUN_ID);
        std::fs::create_dir_all(&run_dir).expect("create run dir");
        std::fs::write(run_dir.join("artifact1"), b"data").expect("write");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta_root.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": TEST_RUN_ID});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");

        assert_eq!(result["status"], "complete");
        assert_eq!(result["enabled_state"], "enabled");
        let rows = result["rows"].as_array().expect("rows array");
        assert_eq!(rows.len(), 1, "one artifact in requested run → one row");
        // path_display must be redacted
        assert!(
            rows[0]["path_display"]
                .as_str()
                .unwrap_or("")
                .starts_with("<redacted:"),
            "path_display must be redacted"
        );
    }

    #[tokio::test]
    async fn p089_hidden_readback_with_test_root_override_scans_real_dir() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let dir = tempfile::TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("artifact.txt"), b"test content").expect("write");
        // Must configure the allowlist to include this temp dir for containment check.
        let dir_path = dir.path().to_str().expect("valid path");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", dir_path);
        let params = serde_json::json!({
            "run_id": TEST_RUN_ID,
            "test_root_override": dir_path
        });
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");

        assert_eq!(result["status"], "complete");
        assert_eq!(result["enabled_state"], "enabled");
        let rows = result["rows"].as_array().expect("rows array");
        assert_eq!(rows.len(), 1, "one file in test root → one row");
        let row = &rows[0];
        // path_display must be redacted
        assert!(
            row["path_display"]
                .as_str()
                .unwrap_or("")
                .starts_with("<redacted:"),
            "path_display must be redacted"
        );
        // path_hash must be 64 hex chars
        let hash = row["path_hash"].as_str().unwrap_or("");
        assert_eq!(hash.len(), 64, "path_hash must be 64 chars");
        // correlation_key must equal path_hash (proposal: use path_hash when present)
        assert_eq!(
            row["correlation_key"], row["path_hash"],
            "correlation_key must equal path_hash"
        );
        // mutation guard still set
        let guard = &result["mutation_guard"];
        assert_eq!(guard["no_delete"], true);
        assert_eq!(guard["no_persist"], true);
    }

    #[tokio::test]
    async fn p089_test_root_override_outside_allowlist_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let other_dir = tempfile::TempDir::new().expect("other dir");
        // Configure allowlist to only contain dir, not other_dir.
        let allowlist = dir.path().to_str().expect("valid path");
        let override_path = other_dir.path().to_str().expect("valid path");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", allowlist);
        let params =
            serde_json::json!({"run_id": TEST_RUN_ID, "test_root_override": override_path});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");

        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "invalid_root_override");
    }

    #[test]
    fn p089_resolve_contained_test_root_rejects_cross_allowlist_symlink_escape() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let root_a = tempfile::TempDir::new().expect("root a");
        let root_b = tempfile::TempDir::new().expect("root b");
        let target_in_b = root_b.path().join("target");
        std::fs::create_dir(&target_in_b).expect("create target dir in root b");
        // A symlink lexically inside allowlisted root A whose realpath resolves into a
        // *different* allowlisted root B must be rejected: lexical and resolved containment
        // must bind to the same allowlist entry, not "any" allowlist entry.
        let link_in_a = root_a.path().join("escape-link");
        std::os::unix::fs::symlink(&target_in_b, &link_in_a).expect("create symlink a->b");

        let allowlist = format!(
            "{}:{}",
            root_a.path().to_str().expect("valid path a"),
            root_b.path().to_str().expect("valid path b")
        );
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", &allowlist);
        let result = resolve_contained_test_root(link_in_a.to_str().expect("valid link path"));
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");

        assert!(
            result.is_none(),
            "symlink resolving from allowlist root A into allowlist root B must be rejected"
        );
    }

    #[test]
    fn p089_resolve_contained_test_root_accepts_same_root_symlink() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let root_a = tempfile::TempDir::new().expect("root a");
        let target_in_a = root_a.path().join("target");
        std::fs::create_dir(&target_in_a).expect("create target dir in root a");
        let link_in_a = root_a.path().join("link");
        std::os::unix::fs::symlink(&target_in_a, &link_in_a).expect("create symlink within a");

        let allowlist = root_a.path().to_str().expect("valid path a").to_string();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", &allowlist);
        let result = resolve_contained_test_root(link_in_a.to_str().expect("valid link path"));
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");

        assert!(
            result.is_some(),
            "a symlink whose target stays within the same allowlisted root must still be accepted"
        );
    }

    #[tokio::test]
    async fn p089_test_root_override_rejected_when_no_allowlist_configured() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let dir_path = dir.path().to_str().expect("valid path");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");
        let params = serde_json::json!({"run_id": TEST_RUN_ID, "test_root_override": dir_path});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");

        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "invalid_root_override");
    }

    #[tokio::test]
    async fn p089_test_root_override_rejected_for_non_automation_caller() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let dir_path = dir.path().to_str().expect("valid path");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", dir_path);
        let params = serde_json::json!({"run_id": TEST_RUN_ID, "test_root_override": dir_path});
        // operator_principal → agent_operator caller class, not automation or developer_break_glass
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");

        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "invalid_root_override");
    }

    // ── Scope enforcement tests (SEC-P089-001) ────────────────────────────────

    #[tokio::test]
    async fn p089_hidden_readback_missing_scope_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        // Neither run_id nor workspace_context provided in enabled mode
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(
            result["status"], "error",
            "missing scope in enabled mode must return error"
        );
    }

    #[tokio::test]
    async fn p089_hidden_readback_both_selectors_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        // Both run_id and workspace_context provided — ambiguous scope
        let params = serde_json::json!({"run_id": TEST_RUN_ID, "workspace_context": {}});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(
            result["status"], "error",
            "both selectors in enabled mode must return error"
        );
    }

    #[tokio::test]
    async fn p089_hidden_readback_run_id_scopes_scan_to_run_dir_only() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        let run_id = TEST_RUN_ID;
        let run_dir = meta.path().join("runs").join(run_id);
        std::fs::create_dir_all(&run_dir).expect("create run dir");
        std::fs::write(run_dir.join("artifact.bin"), b"data for requested run").expect("write");
        // A different run that must NOT appear in the results
        let other_run_dir = meta.path().join("runs").join(OTHER_RUN_ID);
        std::fs::create_dir_all(&other_run_dir).expect("create other run dir");
        std::fs::write(other_run_dir.join("other.bin"), b"other run data").expect("write other");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": run_id});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");

        assert_eq!(result["status"], "complete");
        assert_eq!(result["enabled_state"], "enabled");
        let rows = result["rows"].as_array().expect("rows array");
        // Only the requested run's directory is scanned; other run is excluded (SEC-P089-001)
        assert_eq!(
            rows.len(),
            1,
            "run_id scope must limit scan to that run's directory only"
        );
    }

    #[tokio::test]
    async fn p089_hidden_readback_run_id_symlink_escape_returns_empty() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        let outside = tempfile::TempDir::new().expect("outside dir");
        std::fs::write(outside.path().join("outside.bin"), b"outside").expect("write outside");
        let runs_dir = meta.path().join("runs");
        std::fs::create_dir_all(&runs_dir).expect("create runs dir");
        std::os::unix::fs::symlink(outside.path(), runs_dir.join(TEST_RUN_ID))
            .expect("create escaping run symlink");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": TEST_RUN_ID});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");

        assert_eq!(result["status"], "complete");
        let rows = result["rows"].as_array().expect("rows array");
        assert!(
            rows.is_empty(),
            "canonical run directory escaping runs/ must not be scanned"
        );
    }

    #[tokio::test]
    async fn p089_hidden_readback_run_id_nonexistent_run_returns_complete_empty() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": MISSING_RUN_ID});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        // Nonexistent run dir → no roots → complete with zero rows
        assert_eq!(result["status"], "complete");
        let rows = result["rows"].as_array().expect("rows array");
        assert!(rows.is_empty(), "nonexistent run dir → zero rows");
    }

    // ── Strict type parsing tests (SEC-P089-003) ──────────────────────────────

    #[tokio::test]
    async fn p089_wrong_type_for_include_dry_run_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        // include_dry_run must be a boolean, not a string
        let params = serde_json::json!({"include_dry_run": "yes"});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(
            result["status"], "error",
            "non-boolean include_dry_run must return error"
        );
    }

    #[tokio::test]
    async fn p089_wrong_type_for_limit_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        // limit must be an integer, not a string
        let params = serde_json::json!({"limit": "500"});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        assert_eq!(result["status"], "error", "string limit must return error");
    }

    #[tokio::test]
    async fn p089_wrong_type_for_test_root_override_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        // test_root_override must be a string, not a number
        let params = serde_json::json!({"run_id": TEST_RUN_ID, "test_root_override": 42});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        assert_eq!(
            result["status"], "error",
            "non-string test_root_override must return error"
        );
    }

    #[test]
    fn p089_derive_meta_root_from_database_url_parses_sqlite_prefix() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let expected_parent =
            std::path::PathBuf::from("/Users/user/Documents/Chainworks Forge/.chainworks");
        std::env::set_var(
            "DATABASE_URL",
            "sqlite:///Users/user/Documents/Chainworks Forge/.chainworks/control-plane.db?mode=rwc",
        );
        let result = derive_meta_root_from_database_url();
        std::env::remove_var("DATABASE_URL");
        assert_eq!(result, Some(expected_parent));
    }

    #[tokio::test]
    async fn p089_hidden_readback_test_root_override_invalid_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        let params =
            serde_json::json!({"run_id": TEST_RUN_ID, "test_root_override": "relative/path"});
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        assert_eq!(result["status"], "error");
        assert_eq!(result["errors"][0]["code"], "invalid_root_override");
    }

    // ── SEC-P089-005: run_id path traversal negative tests ───────────────────

    #[tokio::test]
    async fn p089_run_id_absolute_path_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        // Absolute path as run_id — must be rejected (SEC-P089-005)
        let params = serde_json::json!({"run_id": "/etc"});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(
            result["status"], "error",
            "absolute run_id must return error status"
        );
    }

    #[tokio::test]
    async fn p089_run_id_traversal_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        // Traversal in run_id — must be rejected (SEC-P089-005)
        let params = serde_json::json!({"run_id": "../../../etc"});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(
            result["status"], "error",
            "traversal run_id must return error status"
        );
    }

    #[tokio::test]
    async fn p089_run_id_path_separator_returns_error() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        // Path separator in run_id — must be rejected (SEC-P089-005)
        let params = serde_json::json!({"run_id": "foo/bar"});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(
            result["status"], "error",
            "run_id with path separator must return error status"
        );
    }

    // ── workspace_context bounded managed-root discovery ─────────────────────

    #[tokio::test]
    async fn p089_workspace_context_scans_only_known_managed_descendants() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        let workspace = tempfile::TempDir::new().expect("temp workspace");
        let runs_root = workspace.path().join(".chainworks").join("runs");
        let run_root = runs_root.join(TEST_RUN_ID);
        std::fs::create_dir_all(&run_root).expect("create managed runs root");
        std::fs::write(run_root.join("artifact.bin"), b"managed").expect("write managed");
        std::fs::write(
            workspace.path().join("unrelated-secret.txt"),
            b"do not scan",
        )
        .expect("write unrelated");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        let params = serde_json::json!({
            "workspace_context": {
                "workspace_root": workspace.path().to_str().expect("valid workspace path")
            }
        });
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");

        assert_eq!(result["status"], "complete");
        let rows = result["rows"].as_array().expect("rows");
        assert_eq!(rows.len(), 1, "only the managed runs descendant is scanned");
        assert_eq!(rows[0]["root_kind"], "run_meta_root");
        assert!(
            rows.iter().all(|row| {
                row["path_display"]
                    .as_str()
                    .is_some_and(|display| !display.contains("unrelated-secret"))
            }),
            "unrelated workspace files must never be inventoried"
        );
    }

    #[tokio::test]
    async fn p089_workspace_context_requires_canonical_existing_directory() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        let missing =
            std::env::temp_dir().join(format!("p089-missing-workspace-{}", uuid::Uuid::new_v4()));
        let params = serde_json::json!({
            "workspace_context": {
                "workspace_root": missing.to_str().expect("valid path")
            }
        });
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        assert_eq!(result["status"], "error");
    }

    #[tokio::test]
    async fn p089_resource_exhausted_preempts_scope_resolution_filesystem_work() {
        // Regression for SEC-P089-HIGH-001: the permit must be admitted before any
        // canonicalize/is_dir filesystem work runs for scope resolution, not only
        // before the scan phase. Exhaust every context permit for a real,
        // resolvable, non-empty workspace_context, then confirm the request is
        // rejected as resource_exhausted with no scan output — proving scope
        // resolution never ran — rather than proceeding to resolve/scan and
        // returning `complete`.
        let _guard = temp_artifact_inventory_env_test_lock();
        let workspace = tempfile::TempDir::new().expect("temp workspace");
        let runs_root = workspace.path().join(".chainworks").join("runs");
        let run_root = runs_root.join(TEST_RUN_ID);
        std::fs::create_dir_all(&run_root).expect("create managed runs root");
        std::fs::write(run_root.join("artifact.bin"), b"managed").expect("write managed");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");

        let raw_workspace_root = workspace.path().to_str().expect("valid path").to_string();
        // Mirrors the context_key derivation in `execute_inventory_preview`: hashes
        // the raw (pre-canonicalization) workspace_root string.
        let hash = domain::temp_artifact_inventory::compute_path_hash(
            raw_workspace_root.as_bytes(),
            RootKind::Unknown,
        );
        let context_key = format!("workspace:{}", &hash[..12]);

        let mut held = Vec::new();
        for _ in 0..domain::temp_artifact_inventory::SCAN_CONTEXT_PERMIT_MAX {
            held.push(
                crate::tools::scanner::ScanPermitGuard::try_acquire(&context_key)
                    .expect("permit available"),
            );
        }

        let params = serde_json::json!({
            "workspace_context": { "workspace_root": raw_workspace_root }
        });
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        drop(held);

        assert_eq!(
            result["status"], "resource_exhausted",
            "a request whose scope would otherwise resolve and scan successfully \
             must still be rejected before any filesystem work when its context's \
             permits are already held"
        );
    }

    // ── include_dry_run=false row-level fix ───────────────────────────────────

    #[tokio::test]
    async fn p089_include_dry_run_false_rows_have_null_dry_run_recommendation() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let _managed_roots = isolate_managed_roots();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        let run_dir = meta.path().join("runs").join(TEST_RUN_ID);
        std::fs::create_dir_all(&run_dir).expect("create run dir");
        std::fs::write(run_dir.join("artifact1"), b"data").expect("write artifact");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": TEST_RUN_ID, "include_dry_run": false});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        assert_eq!(result["status"], "complete");
        assert!(
            result["dry_run"].is_null(),
            "top-level dry_run must be null"
        );
        let rows = result["rows"].as_array().expect("rows array");
        for row in rows {
            assert!(
                row["dry_run_recommendation"].is_null(),
                "row dry_run_recommendation must be null when include_dry_run=false, got: {:?}",
                row["dry_run_recommendation"]
            );
        }
    }

    // ── Root discovery breadth (control_plane_cache, provider_home_copy,
    //    legacy_chainworks_tmp) ────────────────────────────────────────────

    #[test]
    fn p089_workspace_common_roots_include_provider_home_cache_and_legacy_tmp() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        std::fs::create_dir_all(workspace.path().join(".chainworks/cargo-target"))
            .expect("cache root");
        std::fs::create_dir_all(workspace.path().join(".forge-codex-acp"))
            .expect("provider home root");
        std::fs::create_dir_all(workspace.path().join(".chainworks/tmp")).expect("legacy root");

        let roots = discover_workspace_common_roots(
            &std::fs::canonicalize(workspace.path()).expect("canonical workspace"),
        );
        let kinds = roots.iter().map(|root| root.root_kind).collect::<Vec<_>>();
        assert_eq!(kinds.len(), 3);
        assert!(kinds.contains(&RootKind::ControlPlaneCache));
        assert!(kinds.contains(&RootKind::ProviderHomeCopy));
        assert!(kinds.contains(&RootKind::LegacyChainworksTmp));
    }

    #[tokio::test]
    async fn p089_managed_and_legacy_roots_are_scanned_by_default() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let meta = tempfile::TempDir::new().expect("temp meta dir");
        let cache_dir = tempfile::TempDir::new().expect("cache dir");
        let provider_home_dir = tempfile::TempDir::new().expect("provider home dir");
        let legacy_dir = tempfile::TempDir::new().expect("legacy dir");
        std::fs::write(cache_dir.path().join("cached.bin"), b"cache").expect("write cache");
        std::fs::write(provider_home_dir.path().join("home.bin"), b"home")
            .expect("write provider home");
        std::fs::write(legacy_dir.path().join("legacy.bin"), b"legacy").expect("write legacy");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var(
            "CHAINWORKS_META_ROOT",
            meta.path().to_str().expect("valid path"),
        );
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_CONTROL_PLANE_CACHE_ROOT",
            cache_dir.path().to_str().expect("valid path"),
        );
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_PROVIDER_HOME_FALLBACK_ROOT",
            provider_home_dir.path().to_str().expect("valid path"),
        );
        std::env::set_var(
            "CHAINWORKS_TEMP_ARTIFACT_LEGACY_TMP_ROOT",
            legacy_dir.path().to_str().expect("valid path"),
        );
        let params = serde_json::json!({"run_id": TEST_RUN_ID});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_META_ROOT");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_CONTROL_PLANE_CACHE_ROOT");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_PROVIDER_HOME_FALLBACK_ROOT");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_LEGACY_TMP_ROOT");

        assert_eq!(result["status"], "complete");
        assert!(
            result["limits_applied"]
                .get("global_roots_scan_enabled")
                .is_none(),
            "unapproved global_roots_scan_enabled must not leak into the canonical DTO"
        );
        let rows = result["rows"].as_array().expect("rows array");
        assert_eq!(
            rows.len(),
            3,
            "control_plane_cache, provider_home_copy, and legacy_chainworks_tmp roots must each contribute a row without an opt-in"
        );
        let root_kinds: std::collections::BTreeSet<&str> = rows
            .iter()
            .map(|r| r["root_kind"].as_str().expect("root_kind"))
            .collect();
        assert!(root_kinds.contains("control_plane_cache"));
        assert!(root_kinds.contains("provider_home_copy"));
        assert!(root_kinds.contains("legacy_chainworks_tmp"));
    }

    // ── P089 §4.4/B6: mcp-result-schema.fixture.json conformance ────────────
    //
    // A minimal JSON Schema (draft-07 subset) validator scoped to exactly what
    // the checked-in fixture uses (const/enum/type/pattern/minLength/maxLength/
    // properties/required/additionalProperties/items/maxItems). This exists so
    // the fixture is checked against a *real emitted payload*, not just proven
    // to exist on disk — the gap the reference doc calls out at
    // docs/reference/managed-temporary-artifact-inventory.md §4.4. `pattern` is
    // deliberately not backed by a general regex engine: only the exact
    // patterns the fixture uses are recognized, so an unrecognized pattern
    // fails loudly (extend `matches_known_pattern`) instead of being silently
    // skipped.
    fn json_schema_type_matches(ty: &str, instance: &serde_json::Value) -> bool {
        match ty {
            "string" => instance.is_string(),
            "integer" => instance.is_i64() || instance.is_u64(),
            "number" => instance.is_number(),
            "boolean" => instance.is_boolean(),
            "object" => instance.is_object(),
            "array" => instance.is_array(),
            "null" => instance.is_null(),
            other => panic!("unrecognized JSON Schema \"type\" in fixture: {other:?}"),
        }
    }

    fn matches_known_pattern(pattern: &str, value: &str) -> bool {
        match pattern {
            "^(0|[1-9][0-9]*)$" => {
                !value.is_empty()
                    && value.bytes().all(|b| b.is_ascii_digit())
                    && (value == "0" || !value.starts_with('0'))
            }
            "^[0-9a-f]{64}$" => {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }
            "^[0-9a-f]{12,20}$" => {
                (12..=20).contains(&value.len())
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }
            other => panic!(
                "unrecognized JSON Schema \"pattern\" in fixture; extend matches_known_pattern: {other:?}"
            ),
        }
    }

    fn validate_json_schema(
        schema: &serde_json::Value,
        instance: &serde_json::Value,
        path: &str,
        errors: &mut Vec<String>,
    ) {
        if let Some(const_val) = schema.get("const") {
            if instance != const_val {
                errors.push(format!(
                    "{path}: expected const {const_val}, got {instance}"
                ));
            }
        }
        if let Some(enum_vals) = schema.get("enum").and_then(|v| v.as_array()) {
            if !enum_vals.contains(instance) {
                errors.push(format!("{path}: {instance} not in enum {enum_vals:?}"));
            }
        }
        if let Some(ty) = schema.get("type") {
            let allowed: Vec<&str> = match ty {
                serde_json::Value::String(s) => vec![s.as_str()],
                serde_json::Value::Array(arr) => {
                    arr.iter().filter_map(|v| v.as_str()).collect()
                }
                _ => vec![],
            };
            if !allowed.is_empty()
                && !allowed
                    .iter()
                    .any(|t| json_schema_type_matches(t, instance))
            {
                errors.push(format!(
                    "{path}: type mismatch, expected one of {allowed:?}, got {instance}"
                ));
            }
        }
        if let (Some(pattern), Some(s)) =
            (schema.get("pattern").and_then(|v| v.as_str()), instance.as_str())
        {
            if !matches_known_pattern(pattern, s) {
                errors.push(format!(
                    "{path}: value {s:?} does not match pattern {pattern:?}"
                ));
            }
        }
        if let (Some(min_len), Some(s)) = (
            schema.get("minLength").and_then(|v| v.as_u64()),
            instance.as_str(),
        ) {
            if (s.len() as u64) < min_len {
                errors.push(format!("{path}: length {} < minLength {min_len}", s.len()));
            }
        }
        if let (Some(max_len), Some(s)) = (
            schema.get("maxLength").and_then(|v| v.as_u64()),
            instance.as_str(),
        ) {
            if (s.len() as u64) > max_len {
                errors.push(format!("{path}: length {} > maxLength {max_len}", s.len()));
            }
        }
        if schema.get("properties").is_some() || schema.get("required").is_some() {
            if let Some(obj) = instance.as_object() {
                let required: Vec<&str> = schema
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                for key in &required {
                    if !obj.contains_key(*key) {
                        errors.push(format!("{path}: missing required property {key:?}"));
                    }
                }
                let properties = schema.get("properties").and_then(|v| v.as_object());
                let additional_allowed = schema
                    .get("additionalProperties")
                    .map(|v| v != &serde_json::Value::Bool(false))
                    .unwrap_or(true);
                for (key, value) in obj {
                    match properties.and_then(|p| p.get(key)) {
                        Some(prop_schema) => validate_json_schema(
                            prop_schema,
                            value,
                            &format!("{path}.{key}"),
                            errors,
                        ),
                        None if !additional_allowed => errors.push(format!(
                            "{path}: unexpected property {key:?} not permitted by additionalProperties:false"
                        )),
                        None => {}
                    }
                }
            }
        }
        if let Some(items_schema) = schema.get("items") {
            if let Some(arr) = instance.as_array() {
                if let Some(max_items) = schema.get("maxItems").and_then(|v| v.as_u64()) {
                    if (arr.len() as u64) > max_items {
                        errors.push(format!(
                            "{path}: array length {} > maxItems {max_items}",
                            arr.len()
                        ));
                    }
                }
                for (i, item) in arr.iter().enumerate() {
                    validate_json_schema(items_schema, item, &format!("{path}[{i}]"), errors);
                }
            }
        }
    }

    fn workspace_root_for_fixtures() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            .expect("mcp-server crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn load_mcp_result_schema_fixture() -> serde_json::Value {
        let path = workspace_root_for_fixtures()
            .join("docs/evidence/089/temp-inventory/contracts/mcp-result-schema.fixture.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&raw).expect("fixture must be valid JSON")
    }

    fn assert_validates_against_mcp_result_schema(instance: &serde_json::Value) {
        let schema = load_mcp_result_schema_fixture();
        let mut errors = Vec::new();
        validate_json_schema(&schema, instance, "$", &mut errors);
        assert!(
            errors.is_empty(),
            "mcp-result-schema.fixture.json validation failed against a real emitted payload:\n{}\npayload: {}",
            errors.join("\n"),
            serde_json::to_string_pretty(instance).unwrap_or_default()
        );
    }

    #[tokio::test]
    async fn p089_mcp_result_schema_fixture_validates_real_disabled_payload() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "disabled");
        let params = serde_json::json!({});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");

        assert_eq!(result["status"], "disabled");
        assert_validates_against_mcp_result_schema(&result);
    }

    #[tokio::test]
    async fn p089_mcp_result_schema_fixture_validates_real_error_payload_with_populated_errors() {
        let _guard = temp_artifact_inventory_env_test_lock();
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let params = serde_json::json!({"limit": 501});
        let result = execute_inventory_preview(params, &operator_principal())
            .await
            .expect("ok");

        assert_eq!(result["status"], "error");
        assert!(!result["errors"].as_array().expect("errors array").is_empty());
        assert_validates_against_mcp_result_schema(&result);
    }

    #[tokio::test]
    async fn p089_mcp_result_schema_fixture_validates_real_scanned_payload_with_rows() {
        let _guard = temp_artifact_inventory_env_test_lock();
        let dir = tempfile::TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("artifact.txt"), b"test content").expect("write");
        let dir_path = dir.path().to_str().expect("valid path");

        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE", "hidden_readback");
        std::env::set_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS", dir_path);
        let params = serde_json::json!({
            "run_id": TEST_RUN_ID,
            "test_root_override": dir_path
        });
        let result = execute_inventory_preview(params, &automation_principal())
            .await
            .expect("ok");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS");

        assert_eq!(result["status"], "complete");
        assert_eq!(result["rows"].as_array().expect("rows array").len(), 1);
        assert!(result["dry_run"].is_object(), "default include_dry_run=true");
        assert_validates_against_mcp_result_schema(&result);
    }
}
