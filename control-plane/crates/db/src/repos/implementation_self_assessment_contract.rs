const LEGACY_IMPLEMENTATION_SELF_ASSESSMENT_CONTRACT_IDS: &[&str] = &[
    "implementation_self_assessment_v1",
    "implementation_self_assessment",
];

#[derive(Clone, Debug, PartialEq)]
pub struct StoredImplementationSelfAssessmentSummary {
    pub artifact_id: ArtifactId,
    pub run_id: RunId,
    pub source_contract_id: String,
    pub source_kind: String,
    pub source_precedence: i64,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    pub summary: ImplementationSelfAssessmentSummary,
}

#[derive(Clone, Debug, PartialEq)]
pub struct V1FallbackRetirementCheck {
    pub active_non_terminal_v1_only_run_ids: Vec<RunId>,
}

impl V1FallbackRetirementCheck {
    pub fn safe_to_retire(&self) -> bool {
        self.active_non_terminal_v1_only_run_ids.is_empty()
    }

    pub fn active_non_terminal_v1_only_run_count(&self) -> usize {
        self.active_non_terminal_v1_only_run_ids.len()
    }
}

pub async fn persist_implementation_self_assessment_summary(
    pool: &SqlitePool,
    run_id: RunId,
    artifact_id: ArtifactId,
    source_contract_id: &str,
    summary: &ImplementationSelfAssessmentSummary,
    created_at: DateTime<Utc>,
) -> Result<StoredImplementationSelfAssessmentSummary> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(pool, crate::writer::class_a_operation("artifact_contract_summaries.persist", crate::write_class::WriteLane::CriticalBarrier, "artifact_contract_summaries.persist"), "artifact_contract_summaries.persist")
        .await
        .context("begin artifact contract transaction")?;
    let run_id_str = run_id.to_string();
    let artifact_id_str = artifact_id.to_string();
    let source_kind = classify_source_kind(source_contract_id, summary);
    let source_precedence = source_precedence(&source_kind);

    let active = sqlx::query(
        r#"SELECT artifact_id, artifact_path, source_kind, source_precedence
           FROM artifact_contract_summaries
           WHERE run_id = ?1 AND contract_id = ?2 AND is_active = 1"#,
    )
    .bind(&run_id_str)
    .bind(&summary.contract_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find active artifact contract summary")?;

    let mut warnings = summary.warnings.clone();
    if let Some(active) = &active {
        let active_artifact_path: String = active.get("artifact_path");
        let active_source_kind: String = active.get("source_kind");
        if active_artifact_path == summary.artifact_path && active_source_kind != source_kind {
            warnings.push(ValidationIssue {
                code: "same_path_contract_conflict".to_string(),
                message: "multiple implementation self-assessment contract generations share the same canonical artifact path".to_string(),
                pointer: summary.artifact_path.clone(),
            });
        }
    }

    let status = summary.status.to_string();
    let created_at_str = created_at.to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO artifact_contract_summaries (
          artifact_id, run_id, contract_id, artifact_path, source_contract_id,
          source_kind, source_precedence, status, implementation_complete,
          verification_green, remaining_code_task_count,
          blocking_remaining_code_task_count, handoff_task_count,
          blocking_review_handoff_task_count, owner_class_counts_json,
          target_stage_summaries_json, remaining_code_tasks_json,
          handoff_tasks_json, known_risks_json, tests_run_json,
          docs_impacted_json, validation_errors_json, warnings_json,
          raw_artifact_available, is_active, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, 0, ?25)
        ON CONFLICT(artifact_id) DO UPDATE SET
          run_id = excluded.run_id,
          contract_id = excluded.contract_id,
          artifact_path = excluded.artifact_path,
          source_contract_id = excluded.source_contract_id,
          source_kind = excluded.source_kind,
          source_precedence = excluded.source_precedence,
          status = excluded.status,
          implementation_complete = excluded.implementation_complete,
          verification_green = excluded.verification_green,
          remaining_code_task_count = excluded.remaining_code_task_count,
          blocking_remaining_code_task_count = excluded.blocking_remaining_code_task_count,
          handoff_task_count = excluded.handoff_task_count,
          blocking_review_handoff_task_count = excluded.blocking_review_handoff_task_count,
          owner_class_counts_json = excluded.owner_class_counts_json,
          target_stage_summaries_json = excluded.target_stage_summaries_json,
          remaining_code_tasks_json = excluded.remaining_code_tasks_json,
          handoff_tasks_json = excluded.handoff_tasks_json,
          known_risks_json = excluded.known_risks_json,
          tests_run_json = excluded.tests_run_json,
          docs_impacted_json = excluded.docs_impacted_json,
          validation_errors_json = excluded.validation_errors_json,
          warnings_json = excluded.warnings_json,
          raw_artifact_available = excluded.raw_artifact_available,
          is_active = 0,
          created_at = excluded.created_at
        "#,
    )
    .bind(&artifact_id_str)
    .bind(&run_id_str)
    .bind(&summary.contract_id)
    .bind(&summary.artifact_path)
    .bind(source_contract_id)
    .bind(&source_kind)
    .bind(source_precedence)
    .bind(status)
    .bind(optional_bool_to_i64(summary.implementation_complete))
    .bind(optional_bool_to_i64(summary.verification_green))
    .bind(optional_usize_to_i64(summary.remaining_code_task_count))
    .bind(optional_usize_to_i64(
        summary.blocking_remaining_code_task_count,
    ))
    .bind(optional_usize_to_i64(summary.handoff_task_count))
    .bind(optional_usize_to_i64(
        summary.blocking_review_handoff_task_count,
    ))
    .bind(to_json(&summary.owner_class_counts)?)
    .bind(to_json(&summary.target_stage_summaries)?)
    .bind(to_json(&summary.remaining_code_tasks)?)
    .bind(to_json(&summary.handoff_tasks)?)
    .bind(to_json(&summary.known_risks)?)
    .bind(to_json(&summary.tests_run)?)
    .bind(to_json(&summary.docs_impacted)?)
    .bind(to_json(&summary.validation_errors)?)
    .bind(to_json(&warnings)?)
    .bind(if summary.raw_artifact_available { 1 } else { 0 })
    .bind(created_at_str)
    .execute(&mut **tx)
    .await
    .context("upsert artifact contract summary")?;

    let should_activate = active
        .as_ref()
        .map(|row| {
            let active_precedence: i64 = row.get("source_precedence");
            source_precedence >= active_precedence
        })
        .unwrap_or(true);

    if should_activate {
        sqlx::query(
            r#"UPDATE artifact_contract_summaries
               SET is_active = 0
               WHERE run_id = ?1 AND contract_id = ?2"#,
        )
        .bind(&run_id_str)
        .bind(&summary.contract_id)
        .execute(&mut **tx)
        .await
        .context("deactivate prior artifact contract summaries")?;

        sqlx::query(
            r#"UPDATE artifact_contract_summaries
               SET is_active = 1
               WHERE artifact_id = ?1"#,
        )
        .bind(&artifact_id_str)
        .execute(&mut **tx)
        .await
        .context("activate artifact contract summary")?;
    }

    tx.commit()
        .await
        .context("commit artifact contract transaction")?;
    log_write_transaction("artifact_contract_summaries.persist", tx_started);

    find_active_implementation_self_assessment_summary(pool, run_id)
        .await?
        .context("active implementation self-assessment summary after persist")
}

pub async fn v1_fallback_retirement_check(pool: &SqlitePool) -> Result<V1FallbackRetirementCheck> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT r.id
        FROM runs r
        INNER JOIN artifact_contract_summaries s ON s.run_id = r.id
        WHERE r.status NOT IN ('completed', 'failed', 'cancelled')
          AND s.contract_id = ?1
          AND s.is_active = 1
          AND s.source_kind = 'legacy_v1'
        ORDER BY r.started_at DESC, r.id ASC
        "#,
    )
    .bind(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID)
    .fetch_all(pool)
    .await
    .context("check active non-terminal v1-only implementation self-assessment runs")?;

    let mut run_ids = Vec::with_capacity(rows.len());
    for row in rows {
        let run_id: String = row.get("id");
        run_ids.push(
            run_id
                .parse::<uuid::Uuid>()
                .context("parse v1 retirement check run_id")?
                .into(),
        );
    }

    Ok(V1FallbackRetirementCheck {
        active_non_terminal_v1_only_run_ids: run_ids,
    })
}

pub async fn find_active_implementation_self_assessment_summary(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<StoredImplementationSelfAssessmentSummary>> {
    let run_id_str = run_id.to_string();
    let row = sqlx::query(
        r#"SELECT *
           FROM artifact_contract_summaries
           WHERE run_id = ?1 AND contract_id = ?2 AND is_active = 1"#,
    )
    .bind(run_id_str)
    .bind(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID)
    .fetch_optional(pool)
    .await
    .context("find active implementation self-assessment summary")?;

    row.map(|row| parse_record(&row)).transpose()
}

pub async fn find_implementation_self_assessment_summary_by_artifact(
    pool: &SqlitePool,
    artifact_id: ArtifactId,
) -> Result<Option<StoredImplementationSelfAssessmentSummary>> {
    let artifact_id_str = artifact_id.to_string();
    let row = sqlx::query(
        r#"SELECT *
           FROM artifact_contract_summaries
           WHERE artifact_id = ?1"#,
    )
    .bind(artifact_id_str)
    .fetch_optional(pool)
    .await
    .context("find implementation self-assessment summary by artifact")?;

    row.map(|row| parse_record(&row)).transpose()
}

fn classify_source_kind(
    source_contract_id: &str,
    summary: &ImplementationSelfAssessmentSummary,
) -> String {
    if source_contract_id == IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID
        || summary
            .validation_errors
            .iter()
            .any(|issue| issue.code == "raw_only_v2_artifact")
    {
        "v2".to_string()
    } else if LEGACY_IMPLEMENTATION_SELF_ASSESSMENT_CONTRACT_IDS.contains(&source_contract_id)
        || summary
            .warnings
            .iter()
            .any(|issue| issue.code == "legacy_v1_compatibility")
    {
        "legacy_v1".to_string()
    } else {
        "unknown".to_string()
    }
}

fn source_precedence(source_kind: &str) -> i64 {
    match source_kind {
        "v2" => 20,
        "legacy_v1" => 10,
        _ => 0,
    }
}

fn optional_bool_to_i64(value: Option<bool>) -> Option<i64> {
    value.map(|value| if value { 1 } else { 0 })
}

fn optional_i64_to_bool(value: Option<i64>) -> Option<bool> {
    value.map(|value| value != 0)
}

fn optional_usize_to_i64(value: Option<usize>) -> Option<i64> {
    value.map(|value| value.min(i64::MAX as usize) as i64)
}

fn count_i64_to_usize(value: Option<i64>) -> Option<usize> {
    value.map(|value| value.max(0) as usize)
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).context("serialize artifact contract summary field")
}

fn from_json<T: DeserializeOwned>(value: String, field: &str) -> Result<T> {
    serde_json::from_str(&value).with_context(|| format!("parse {field} json"))
}

fn parse_record(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<StoredImplementationSelfAssessmentSummary> {
    let artifact_id: String = row.get("artifact_id");
    let run_id: String = row.get("run_id");
    let status: String = row.get("status");
    let created_at: String = row.get("created_at");
    let warnings_json: String = row.get("warnings_json");

    let summary = ImplementationSelfAssessmentSummary {
        contract_id: row.get("contract_id"),
        artifact_path: row.get("artifact_path"),
        status: status
            .parse::<ImplementationSelfAssessmentStatus>()
            .map_err(|error| anyhow::anyhow!(error))?,
        implementation_complete: optional_i64_to_bool(row.get("implementation_complete")),
        verification_green: optional_i64_to_bool(row.get("verification_green")),
        remaining_code_task_count: count_i64_to_usize(row.get("remaining_code_task_count")),
        blocking_remaining_code_task_count: count_i64_to_usize(
            row.get("blocking_remaining_code_task_count"),
        ),
        handoff_task_count: count_i64_to_usize(row.get("handoff_task_count")),
        blocking_review_handoff_task_count: count_i64_to_usize(
            row.get("blocking_review_handoff_task_count"),
        ),
        owner_class_counts: from_json(row.get("owner_class_counts_json"), "owner_class_counts")?,
        target_stage_summaries: from_json(
            row.get("target_stage_summaries_json"),
            "target_stage_summaries",
        )?,
        remaining_code_tasks: from_json(
            row.get("remaining_code_tasks_json"),
            "remaining_code_tasks",
        )?,
        handoff_tasks: from_json(row.get("handoff_tasks_json"), "handoff_tasks")?,
        known_risks: from_json(row.get("known_risks_json"), "known_risks")?,
        tests_run: from_json(row.get("tests_run_json"), "tests_run")?,
        docs_impacted: from_json(row.get("docs_impacted_json"), "docs_impacted")?,
        validation_errors: from_json(row.get("validation_errors_json"), "validation_errors")?,
        warnings: from_json(warnings_json, "warnings")?,
        raw_artifact_available: row.get::<i64, _>("raw_artifact_available") != 0,
    };

    Ok(StoredImplementationSelfAssessmentSummary {
        artifact_id: artifact_id
            .parse::<uuid::Uuid>()
            .context("parse artifact contract artifact_id")?
            .into(),
        run_id: run_id
            .parse::<uuid::Uuid>()
            .context("parse artifact contract run_id")?
            .into(),
        source_contract_id: row.get("source_contract_id"),
        source_kind: row.get("source_kind"),
        source_precedence: row.get("source_precedence"),
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .context("parse artifact contract created_at")?
            .with_timezone(&Utc),
        is_active: row.get::<i64, _>("is_active") != 0,
        summary,
    })
}
