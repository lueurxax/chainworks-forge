use anyhow::Result;
use sqlx::SqlitePool;

/// Validate that `key` is a well-formed UUIDv7 string.
/// UUIDv7 format: xxxxxxxx-xxxx-7xxx-[89ab]xxx-xxxxxxxxxxxx (36 chars, version nibble = 7,
/// variant nibble ∈ {8,9,a,b}). Rejects non-hex chars, wrong hyphens, wrong version/variant.
fn validate_uuidv7_idempotency_key(key: &str) -> Result<()> {
    if key.len() != 36 {
        return Err(anyhow::anyhow!(
            "idempotency_key must be a 36-character UUIDv7 string"
        ));
    }
    let bytes = key.as_bytes();
    for &pos in &[8usize, 13, 18, 23] {
        if bytes[pos] != b'-' {
            return Err(anyhow::anyhow!(
                "idempotency_key must be UUIDv7 (hyphen expected at position {pos})"
            ));
        }
    }
    if bytes[14] != b'7' {
        return Err(anyhow::anyhow!(
            "idempotency_key must be UUIDv7 (version nibble at position 14 must be '7', got '{}')",
            bytes[14] as char
        ));
    }
    if !matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B') {
        return Err(anyhow::anyhow!(
            "idempotency_key must be UUIDv7 (variant nibble at position 19 must be 8/9/a/b, got '{}')",
            bytes[19] as char
        ));
    }
    for (i, &b) in bytes.iter().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            continue;
        }
        if !b.is_ascii_hexdigit() {
            return Err(anyhow::anyhow!(
                "idempotency_key must be UUIDv7 (non-hex character '{}' at position {i})",
                b as char
            ));
        }
    }
    Ok(())
}

use db::repos::{approvals, lead_mediation_confirmations};
use domain::commands::{
    ApprovalResolutionDecision, Command, ResolveApprovalCmd, ResolveLeadMediationConfirmationCmd,
};
use domain::ids::{ApprovalId, RunId};
use domain::mediation::{ApprovalInboxItem, ApprovalSubjectKind};
use engine::command_handler::{validate_caller_request_id, CommandHandler};

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "approvals.list".to_string(),
            description:
                "List pending approvals (mixed inbox: stage approvals + mediation confirmations)"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: None,
        },
        McpTool {
            name: "approvals.resolve".to_string(),
            description: "Resolve a pending approval or mediation confirmation. Requires a \
                CallerRequestId (lowercase UUIDv4) for command_idempotency_contract_v1 \
                replay safety (TTL=300s)."
                .to_string(),
            input_schema: serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "required": ["decision", "request_id"],
                "additionalProperties": false,
                "properties": {
                    "subject_kind": {
                        "type": "string",
                        "enum": ["stage_approval", "lead_mediation_confirmation"],
                        "description": "Type of approval subject. Omit or use 'stage_approval' for legacy stage approval compatibility."
                    },
                    "approval_id": {
                        "type": "string",
                        "description": "P072: Canonical approval identifier for stage approvals. Preferred over run_id+stage_id."
                    },
                    "subject_id": {
                        "type": "string",
                        "description": "Subject identifier (confirmation id for mediation)"
                    },
                    "run_id": { "type": "string", "description": "Deprecated compatibility hint for stage_approval; required for lead_mediation_confirmation" },
                    "stage_id": { "type": "string", "description": "Deprecated compatibility hint for stage_approval" },
                    "decision": {
                        "type": "string",
                        "enum": ["granted", "rejected", "confirm", "manual_fallback"],
                        "description": "Decision: granted/rejected for stage approvals, confirm/manual_fallback for mediation"
                    },
                    "comment": { "type": "string" },
                    "conflict_fingerprint": {
                        "type": "string",
                        "description": "Required for lead_mediation_confirmation"
                    },
                    "request_id": {
                        "type": "string",
                        "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
                        "description": "CallerRequestId: lowercase UUIDv4 for command_idempotency_contract_v1 (TTL=300s)"
                    }
                }
            }),
            output_schema: Some(serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["resolved"],
                "properties": {
                    "resolved": { "type": "boolean", "description": "true when the approval resolution was durably committed; false on denial" },
                    "approval_id": { "type": "string" },
                    "journal_id": { "type": "string" },
                    "request_id": { "type": "string", "description": "Replayed CallerRequestId for idempotency confirmation" },
                    "denied": { "type": "boolean", "description": "true when the command was denied before execution" },
                    "denial_code": { "type": "string", "enum": ["malformed_request_id","request_intent_mismatch","idempotency_in_flight","idempotency_replayed","idempotency_expired_reacquired","idempotency_replay_corrupt","idempotency_terminal_failure","operator_required","p083_operator_required","provider_session_not_found","run_not_found","stage_not_retryable","approval_not_actionable","side_effect_not_reconcilable","enforcement_mode_transition_denied","identity_ambiguous","internal"], "description": "Bounded denial code from P083LifecycleDenialCode" },
                    "message": { "type": "string", "description": "Human-readable denial or error message" }
                }
            })),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "approvals.list" => {
            // Union canonical pending rows from stage approvals and mediation confirmations
            let stage_items = approvals::list_pending(pool).await?;
            let mediation_items = lead_mediation_confirmations::list_pending(pool).await?;

            let mut inbox: Vec<ApprovalInboxItem> = Vec::new();

            // Add stage approvals
            for approval in &stage_items {
                inbox.push(ApprovalInboxItem {
                    subject_kind: ApprovalSubjectKind::StageApproval,
                    subject_id: approval.id.to_string(),
                    run_id: approval.run_id.to_string(),
                    status: approval.decision.to_string(),
                    requested_at: approval.requested_at.to_rfc3339(),
                    deadline_at: approval.expires_at.map(|t| t.to_rfc3339()),
                    readback_ref: None,
                    stage_id: Some(approval.stage_id.clone()),
                    decision: Some(approval.decision.to_string()),
                    conflict_id: None,
                    conflict_fingerprint: None,
                    suggested_action: None,
                    resolution_mode: None,
                });
            }

            // Add mediation confirmations — MC-004: derive resolution_mode using
            // the shared domain function to ensure MCP and GraphQL stay consistent.
            for confirmation in &mediation_items {
                let resolution_mode = match db::repos::lead_conflict_mediations::find_by_id(
                    pool,
                    &confirmation.mediation_record_id,
                )
                .await?
                {
                    Some(mediation) => domain::mediation::derive_resolution_mode(&mediation),
                    None => None,
                };

                inbox.push(ApprovalInboxItem {
                    subject_kind: ApprovalSubjectKind::LeadMediationConfirmation,
                    subject_id: confirmation.id.clone(),
                    run_id: confirmation.run_id.clone(),
                    status: confirmation.status.to_string(),
                    requested_at: confirmation.requested_at.to_rfc3339(),
                    deadline_at: confirmation.deadline_at.map(|t| t.to_rfc3339()),
                    readback_ref: confirmation.readback_ref.clone(),
                    stage_id: None,
                    decision: None,
                    conflict_id: Some(confirmation.conflict_id.clone()),
                    conflict_fingerprint: Some(confirmation.conflict_fingerprint.clone()),
                    suggested_action: confirmation.suggested_action.clone(),
                    resolution_mode,
                });
            }

            // Sort by requested_at
            inbox.sort_by(|a, b| a.requested_at.cmp(&b.requested_at));

            Ok(serde_json::to_value(&inbox)?)
        }

        "approvals.resolve" => {
            let subject_kind = params["subject_kind"].as_str().unwrap_or("stage_approval");

            match subject_kind {
                "lead_mediation_confirmation" => {
                    resolve_mediation_confirmation(&params, cmd_handler, principal).await
                }
                "stage_approval" => resolve_stage_approval(&params, cmd_handler, principal).await,
                unknown => Err(anyhow::anyhow!(
                    "Unknown subject_kind: '{}'. Expected 'stage_approval' or 'lead_mediation_confirmation'",
                    unknown
                )),
            }
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

async fn resolve_stage_approval(
    params: &serde_json::Value,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let decision_str = params["decision"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'decision'"))?;
    let comment = params["comment"].as_str().map(|s| s.to_string());

    // P083 command_idempotency_contract_v1: request_id (lowercase UUIDv4) is required.
    // Legacy idempotency_key fallback is removed per P083 CallerRequestId mandate.
    let p083_request_id = if let Some(rid) = params["request_id"].as_str() {
        validate_caller_request_id(rid)?;
        rid.to_string()
    } else {
        return Err(anyhow::anyhow!(
            "approvals.resolve requires 'request_id' (lowercase UUIDv4 per P083 command_idempotency_contract_v1)"
        ));
    };
    let p081_idempotency_key: Option<String> = None;

    // SEC-P083-MED-003: stamp the validated CallerRequestId into CallerContext so
    // command_journal.request_id carries the lifecycle request id, not the HTTP request id.
    let caller = mcp_caller(&principal, "approvals.resolve")
        .with_request_id(p083_request_id.clone());

    // P072: Prefer approval_id-based routing through ResolveApprovalCmd.
    if let Some(approval_id_str) = params["approval_id"].as_str() {
        let approval_id: ApprovalId = approval_id_str
            .parse::<uuid::Uuid>()
            .map_err(|e| anyhow::anyhow!("Invalid approval_id: {e}"))?
            .into();

        let decision: ApprovalResolutionDecision = decision_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!("{e}"))?;

        // Server-resolve run_id and stage_id from the approval record.
        let approval = approvals::find_by_id(cmd_handler.pool(), approval_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Approval '{approval_id}' not found"))?;

        // P072: If deprecated compatibility hints are supplied, reject unless
        // they exactly match the resolved provenance.
        if let Some(hint_run_id) = params["run_id"].as_str() {
            if hint_run_id != approval.run_id.to_string() {
                return Err(anyhow::anyhow!(
                    "Compatibility hint run_id does not match resolved provenance"
                ));
            }
        }
        if let Some(hint_stage_id) = params["stage_id"].as_str() {
            if hint_stage_id != approval.stage_id {
                return Err(anyhow::anyhow!(
                    "Compatibility hint stage_id does not match resolved provenance"
                ));
            }
        }

        let cmd = Command::ResolveApproval(ResolveApprovalCmd {
            approval_id,
            decision,
            rationale: comment,
            run_id: approval.run_id,
            stage_id: approval.stage_id,
            idempotency_key: p081_idempotency_key,
            request_id: Some(p083_request_id.clone()),
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        return Ok(serde_json::json!({
            "resolved": true,
            "request_id": p083_request_id,
            "journal_id": commanded.journal_id,
        }));
    }

    // Legacy path: resolve by run_id + stage_id.
    let run_id: RunId = params["run_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'run_id' (required when approval_id is absent)"))?
        .parse()?;
    let stage_id = params["stage_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id' (required when approval_id is absent)"))?
        .to_string();

    let decision: ApprovalResolutionDecision = decision_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{e}"))?;

    let approval = approvals::find_pending_by_run_stage(cmd_handler.pool(), run_id, &stage_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No pending approval found for run '{}', stage '{}'",
                run_id,
                stage_id
            )
        })?;

    let cmd = Command::ResolveApproval(ResolveApprovalCmd {
        approval_id: approval.id,
        decision,
        rationale: comment,
        run_id: approval.run_id,
        stage_id: approval.stage_id,
        idempotency_key: p081_idempotency_key,
        request_id: Some(p083_request_id.clone()),
    });

    let commanded = cmd_handler.handle(cmd, caller).await?;
    Ok(serde_json::json!({
        "resolved": true,
        "request_id": p083_request_id,
        "journal_id": commanded.journal_id,
    }))
}

async fn resolve_mediation_confirmation(
    params: &serde_json::Value,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let run_id: RunId = params["run_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
        .parse()?;
    let subject_id = params["subject_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'subject_id'"))?
        .to_string();
    let decision_str = params["decision"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'decision'"))?;
    let conflict_fingerprint = params["conflict_fingerprint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'conflict_fingerprint'"))?
        .to_string();
    let idempotency_key = params["idempotency_key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
        .to_string();
    let comment = params["comment"].as_str().map(|s| s.to_string());

    // CL-002: Input length validation at MCP boundary.
    if conflict_fingerprint.len() > 512 {
        return Err(anyhow::anyhow!(
            "conflict_fingerprint exceeds maximum length (512 bytes)"
        ));
    }
    if conflict_fingerprint
        .bytes()
        .any(|b| b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t')
    {
        return Err(anyhow::anyhow!(
            "conflict_fingerprint contains invalid control characters"
        ));
    }
    // SEC-003: idempotency_key for mediation confirmation must also be UUIDv7.
    validate_uuidv7_idempotency_key(&idempotency_key)?;

    let decision: domain::mediation::MediationConfirmationDecision = decision_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{e}"))?;

    // MF-002: Derive mediation_record_id server-side from the confirmation
    // record. The approved wire contract does not require callers to provide it.
    let confirmation = lead_mediation_confirmations::find_by_id(
        // We need the pool — get it from the command handler
        cmd_handler.pool(),
        &subject_id,
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("Mediation confirmation '{}' not found", subject_id))?;
    let mediation_record_id = confirmation.mediation_record_id.clone();

    let caller = mcp_caller(&principal, "approvals.resolve");
    let cmd = Command::ResolveLeadMediationConfirmation(ResolveLeadMediationConfirmationCmd {
        run_id,
        mediation_record_id,
        confirmation_subject_id: subject_id,
        decision,
        comment,
        conflict_fingerprint,
        idempotency_key,
    });

    let commanded = cmd_handler.handle(cmd, caller).await?;

    // DEF-002: Return typed stale_or_terminal result instead of generic error
    // so MCP callers can distinguish actionability failures from real errors.
    match &commanded.result {
        engine::command_handler::CommandResult::LeadMediationConfirmationStaleOrTerminal {
            confirmation_subject_id,
            reason,
            journal_id,
        } => Ok(serde_json::json!({
            "result": "stale_or_terminal",
            "resolved": false,
            "subject_id": confirmation_subject_id,
            "reason": reason,
            "journal_id": journal_id,
        })),
        _ => Ok(serde_json::json!({
            "resolved": true,
            "journal_id": commanded.journal_id,
        })),
    }
}
