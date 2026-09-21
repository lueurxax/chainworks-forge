//! Canonical, bounded operator readback. This module never exposes filesystem
//! locations, caller identities, runtime configuration or raw effect responses.
use anyhow::{ensure, Result};
use db::repos::run_continuations::{self, Continuation};
use domain::{ids::RunId, run_carry_forward::ContentDigest, run_carry_forward_api::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;

/// Runtime facts, injected by the composition root; never inferred from a request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReadbackConfig {
    pub enabled: bool,
    pub reconcile_available: bool,
}

pub fn compact(
    op: &Continuation,
    admission_enabled: bool,
) -> Result<RunContinuationCompactReadbackV1> {
    compact_with_config(
        op,
        ReadbackConfig {
            enabled: admission_enabled,
            reconcile_available: false,
        },
    )
}

pub fn compact_with_config(
    op: &Continuation,
    config: ReadbackConfig,
) -> Result<RunContinuationCompactReadbackV1> {
    ensure!(
        op.holds_json.len() <= 65536,
        "continuation readback hold bound"
    );
    let stored: Vec<String> = serde_json::from_str(&op.holds_json)?;
    ensure!(stored.len() <= 128, "continuation readback hold bound");
    let holds=stored.into_iter().map(|reason| {
        let code=serde_json::from_value::<ContinuationReasonCode>(json!(reason))
            .unwrap_or(ContinuationReasonCode::EffectOutcomeUnknown);
        // Only the closed machine code crosses the boundary, not stored free text.
        json!({"code":code,"message":serde_json::to_value(code).expect("enum").as_str().expect("enum string")})
    }).collect::<Vec<_>>();
    let (disposition, verification, next) = match op.phase.as_str() {
        "preparing" => ("source_reserved", "pending", vec!["await_worker"]),
        "prepared" => ("source_reserved", "verified", vec!["inspect"]),
        "activated" => ("source_historical", "verified", vec!["none"]),
        "aborted" => ("aborted_source_blocked", "unverified", vec!["none"]),
        "aborting" => ("source_reserved", "unknown", vec!["read_operation"]),
        "needs_reconciliation" => ("source_reserved", "unknown", vec!["explicit_reconcile"]),
        _ => ("source_reserved", "unknown", Vec::new()),
    };
    Ok(serde_json::from_value(json!({
        "schema_version":"run_continuation_readback_v1",
        "operation_id":op.operation_id,"source_run_id":op.source_run_id,"successor_run_id":op.successor_run_id,
        "phase":op.phase,"version":op.version,
        "plan_sha256":ContentDigest::from_sha256_hex(&op.plan_sha256)?,
        "manifest_sha256":op.manifest_sha256.as_deref().map(ContentDigest::from_sha256_hex).transpose()?,
        "execution_disposition":disposition,"verification_status":verification,"holds":holds,
        "next_actions":next,"journal_id":op.journal_id,"updated_at":op.updated_at,"projection_freshness":"fresh",
        "admission":{"schema_version":"run_continuation_admission_v1","enabled":config.enabled,
            "reason":if config.enabled {None}else{Some("rollout_hold")},"fences_enforced":true,"reconcile_available":config.reconcile_available}
    }))?)
}

/// Callers authorize the queried Run and each linked source/successor before
/// serializing this value. The shared projection itself grants no access.
pub async fn links(
    pool: &SqlitePool,
    run_id: RunId,
    admission_enabled: bool,
) -> Result<RunCarryForwardLinksV1> {
    links_with_config(
        pool,
        run_id,
        ReadbackConfig {
            enabled: admission_enabled,
            reconcile_available: false,
        },
    )
    .await
}

pub async fn links_with_config(
    pool: &SqlitePool,
    run_id: RunId,
    config: ReadbackConfig,
) -> Result<RunCarryForwardLinksV1> {
    let values = run_continuations::links(pool, &run_id.to_string()).await?;
    Ok(RunCarryForwardLinksV1 {
        schema_version: LinksSchema::V1,
        incoming: values
            .incoming
            .as_ref()
            .map(|op| compact_with_config(op, config))
            .transpose()?,
        outgoing: values
            .outgoing
            .as_ref()
            .map(|op| compact_with_config(op, config))
            .transpose()?,
    })
}

pub fn can_read_run(principal: &auth::Principal, run_id: &str) -> bool {
    auth::check_p080_run_scope(principal, Some(run_id)).is_ok()
        && match &principal.run_scope {
            Some(scope) => scope.iter().any(|id| id == run_id),
            None => principal.class == auth::PrincipalClass::Operator,
        }
}

/// The transport must first authorize its existing run-read capability. This
/// helper only narrows that access; it never grants a new continuation tool.
pub async fn authorized_links(
    pool: &SqlitePool,
    run_id: RunId,
    principal: &auth::Principal,
    config: ReadbackConfig,
) -> Result<RunCarryForwardLinksV1> {
    ensure!(
        can_read_run(principal, &run_id.to_string()),
        "continuation access denied"
    );
    let mut value = links_with_config(pool, run_id, config).await?;
    for direction in [&mut value.incoming, &mut value.outgoing] {
        if direction.as_ref().is_some_and(|op| {
            !can_read_run(principal, &op.source_run_id.as_uuid().to_string())
                || op
                    .successor_run_id
                    .as_ref()
                    .is_some_and(|id| !can_read_run(principal, &id.as_uuid().to_string()))
        }) {
            *direction = None;
        }
    }
    Ok(value)
}

pub fn public_fields(links: &RunCarryForwardLinksV1) -> Result<serde_json::Value> {
    let incoming = links
        .incoming
        .as_ref()
        .filter(|op| op.phase.as_str() == "activated");
    let outgoing = links
        .outgoing
        .as_ref()
        .filter(|op| op.phase.as_str() == "activated");
    Ok(json!({
        "continued_from_run_id": incoming.map(|op| &op.source_run_id),
        "continued_as_run_id": outgoing.and_then(|op| op.successor_run_id.as_ref()),
        "carry_forward_readback": if links.incoming.is_none() && links.outgoing.is_none() { serde_json::Value::Null } else { serde_json::to_value(links)? },
    }))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportFields {
    pub continued_from_run_id: Option<CanonicalUuid>,
    pub continued_as_run_id: Option<CanonicalUuid>,
    pub carry_forward_readback: Option<RunCarryForwardLinksV1>,
    #[serde(default)]
    pub carry_forward_input_provenance: Vec<ReportInputBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportInputBinding {
    pub operation_id: CanonicalUuid,
    pub input_id: CanonicalUuid,
    pub source_schema: BoundedText<256>,
    /// Historical storage retains an identifier, not schema-definition bytes.
    pub source_schema_sha256: Option<ContentDigest>,
    pub content_sha256: ContentDigest,
    pub manifest_sha256: Option<ContentDigest>,
}

/// Durable report/receipt writers only. No manifest contents or paths are read.
pub async fn report_fields(
    pool: &SqlitePool,
    run_id: RunId,
    config: ReadbackConfig,
) -> Result<ReportFields> {
    use sqlx::Row;
    let links = links_with_config(pool, run_id, config).await?;
    let mut fields: ReportFields = serde_json::from_value(public_fields(&links)?)?;
    for op in [&links.incoming, &links.outgoing].into_iter().flatten() {
        let rows = sqlx::query("SELECT input_id,source_schema,content_sha256 FROM run_continuation_inputs WHERE operation_id=? AND installed=1 ORDER BY ordinal LIMIT 130")
            .bind(op.operation_id.as_uuid().to_string()).fetch_all(pool).await?;
        ensure!(rows.len() <= 129, "continuation report input bound");
        for row in rows {
            fields
                .carry_forward_input_provenance
                .push(ReportInputBinding {
                    operation_id: op.operation_id.clone(),
                    input_id: serde_json::from_value(json!(row.try_get::<String, _>("input_id")?))?,
                    source_schema: serde_json::from_value(json!(
                        row.try_get::<String, _>("source_schema")?
                    ))?,
                    source_schema_sha256: None,
                    content_sha256: ContentDigest::from_sha256_hex(
                        &row.try_get::<String, _>("content_sha256")?,
                    )?,
                    manifest_sha256: op.manifest_sha256.clone(),
                });
        }
    }
    Ok(fields)
}
