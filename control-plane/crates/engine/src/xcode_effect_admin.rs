//! Operator-only durable Xcode diagnostics and revision-checked reconciliation.

use acp::xcode_headless_host::TrustedProject;
use anyhow::{ensure, Result};
use auth::Principal;
pub use db::repos::xcode_effect_attempts::AttemptPage;
use db::{
    repos::xcode_effect_attempts as journal,
    write_class::WriteLane,
    writer::{class_a_operation, DbWriter},
};
use domain::{
    xcode_effect::{validate_text, validate_uuid, AttemptRevision, Reconciliation, StoredAttempt},
    CapabilityToolId,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

pub struct XcodeEffectAdmin {
    pool: SqlitePool,
    writer: Arc<DbWriter>,
}

impl XcodeEffectAdmin {
    pub fn new(pool: SqlitePool, writer: Arc<DbWriter>) -> Self {
        Self { pool, writer }
    }

    pub async fn list(
        &self,
        principal: &Principal,
        run_id: Uuid,
        owner: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<AttemptPage> {
        authorize(principal, CapabilityToolId::XcodeEffectsDiagnostics, run_id)?;
        validate_text("owner_lineage", owner, 256)?;
        ensure!((1..=100).contains(&limit), "xcode_admin_invalid_limit");
        let after = if let Some(cursor) = cursor {
            let id = Uuid::parse_str(cursor)
                .map_err(|_| anyhow::anyhow!("xcode_admin_invalid_cursor"))?;
            ensure!(id.to_string() == cursor, "xcode_admin_invalid_cursor");
            self.scoped_get(run_id, owner, id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("xcode_admin_invalid_cursor"))?
                .sequence
        } else {
            0
        };
        // Filter before pagination. Owner lineages can contain attempts from
        // multiple runs; a foreign cursor must not disclose or skip their rows.
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT attempt_id FROM xcode_effect_attempts WHERE owner_lineage = ? \
             AND json_extract(intent_json, '$.run_id') = ? AND sequence > ? ORDER BY sequence LIMIT ?")
            .bind(owner).bind(run_id.to_string()).bind(after).bind(i64::from(limit) + 1)
            .fetch_all(&self.pool).await?;
        let more = ids.len() > limit as usize;
        let mut items = Vec::with_capacity(limit as usize);
        for id in ids.into_iter().take(limit as usize) {
            let attempt = self
                .scoped_get(run_id, owner, Uuid::parse_str(&id)?)
                .await?
                .ok_or_else(|| anyhow::anyhow!("xcode_admin_corrupt_record"))?;
            items.push(attempt);
        }
        let next_cursor = if more {
            items.last().map(|a| a.attempt_id.to_string())
        } else {
            None
        };
        Ok(AttemptPage { items, next_cursor })
    }

    pub async fn get(
        &self,
        principal: &Principal,
        run_id: Uuid,
        owner: &str,
        attempt_id: Uuid,
    ) -> Result<Option<StoredAttempt>> {
        authorize(principal, CapabilityToolId::XcodeEffectsDiagnostics, run_id)?;
        self.scoped_get(run_id, owner, attempt_id).await
    }

    pub async fn reconcile(
        &self,
        principal: &Principal,
        run_id: Uuid,
        owner: &str,
        attempt: AttemptRevision,
        evidence: &Reconciliation,
    ) -> Result<StoredAttempt> {
        authorize(principal, CapabilityToolId::XcodeEffectsReconcile, run_id)?;
        let stored = self
            .scoped_get(run_id, owner, attempt.attempt_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("xcode_admin_not_found"))?;
        let mut evidence = evidence.clone();
        evidence.operator_id = principal.id.clone();
        evidence.validate(&stored.intent.result_schema)?;
        // The journal makes intent identity immutable. Recheck state/revision
        // and settlement proof in its single-purpose writer transaction.
        let operation = "xcode_effect.reconcile";
        let tx = self
            .writer
            .begin_immediate_transaction(
                class_a_operation(
                    operation,
                    WriteLane::CriticalBarrier,
                    format!(
                        "{run_id}/{owner}/{}/{}",
                        attempt.attempt_id, attempt.revision
                    ),
                ),
                operation,
            )
            .await?;
        Ok(journal::reconcile(
            tx,
            owner,
            attempt.attempt_id,
            attempt.revision,
            &evidence,
            chrono::Utc::now(),
        )
        .await?)
    }

    pub async fn authorize_trust_target(
        &self,
        principal: &Principal,
        run_id: Uuid,
        project: &TrustedProject,
    ) -> Result<()> {
        authorize(principal, CapabilityToolId::XcodeProjectTrust, run_id)?;
        project.revalidate()?;
        let run = db::repos::runs::find_by_id(&self.pool, run_id.into())
            .await?
            .ok_or_else(|| anyhow::anyhow!("xcode_admin_run_not_found"))?;
        let expected = acp::execution_root::resolve_execution_root(
            &run.workspace_root,
            run.worktree_root.as_deref(),
            project.root().kind == domain::execution_root::ExecutionRootKind::Worktree,
            project.root().strategy.as_deref(),
        )?;
        ensure!(
            &expected == project.root(),
            "xcode_admin_project_scope_mismatch"
        );
        Ok(())
    }

    async fn scoped_get(
        &self,
        run_id: Uuid,
        owner: &str,
        attempt_id: Uuid,
    ) -> Result<Option<StoredAttempt>> {
        Ok(journal::get(&self.pool, owner, attempt_id)
            .await?
            .filter(|attempt| attempt.intent.run_id == run_id))
    }
}

fn authorize(principal: &Principal, capability: CapabilityToolId, run_id: Uuid) -> Result<()> {
    auth::require_xcode_operator(principal, capability, Some(run_id))
        .map_err(anyhow::Error::msg)?;
    validate_uuid("run_id", run_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xcode_effect_journal::DbXcodeEffectJournal;
    use acp::xcode_effect_journal::{XcodeDispatchDecision, XcodeEffectJournal};
    use domain::{xcode_effect::*, CapabilityToolId, PrincipalClass};

    struct Fixture {
        _dir: tempfile::TempDir,
        pool: SqlitePool,
        writer: Arc<DbWriter>,
        admin: XcodeEffectAdmin,
        journal: DbXcodeEffectJournal,
        intent: NormalizedIntent,
    }

    impl Fixture {
        async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().canonicalize().unwrap().join("journal.sqlite");
            let pool = db::pool::create_pool(&format!("sqlite://{}?mode=rwc", path.display()))
                .await
                .unwrap();
            let writer = Arc::new(DbWriter::new(pool.clone()));
            let intent = NormalizedIntent {
                run_id: Uuid::new_v4(),
                owner_lineage: "durable-owner".into(),
                invocation_id: Uuid::new_v4(),
                operation_key: Uuid::new_v4(),
                project_key: ProjectKey {
                    version: 1,
                    uid: 501,
                    canonical_path: "/fixture/App.xcodeproj".into(),
                    device: "7".into(),
                    inode: "99".into(),
                },
                operation: "retired.operation".into(),
                target_digest: "a".repeat(64),
                binding_digest: Some("b".repeat(64)),
                request_digest: "c".repeat(64),
                origin: EffectOrigin::Mcp,
                effect_class: EffectClass::Mutate,
                result_schema: SchemaIdentity {
                    contract_id: "retired.result".into(),
                    version: 1,
                    manifest_digest: "d".repeat(64),
                    schema_digest: "e".repeat(64),
                },
            };
            let journal = DbXcodeEffectJournal::new(
                pool.clone(),
                writer.clone(),
                intent.run_id,
                intent.owner_lineage.clone(),
            );
            let admin = XcodeEffectAdmin::new(pool.clone(), writer.clone());
            Self {
                _dir: dir,
                pool,
                writer,
                admin,
                journal,
                intent,
            }
        }

        async fn unknown(&self) -> StoredAttempt {
            let prepared = self.journal.prepare(&self.intent).await.unwrap();
            let XcodeDispatchDecision::Dispatch(dispatched) = self
                .journal
                .dispatch(prepared.nonce, &self.intent.request_digest, 0)
                .await
                .unwrap()
            else {
                panic!("expected first dispatch");
            };
            self.journal
                .complete(
                    AttemptRevision {
                        attempt_id: dispatched.attempt_id,
                        revision: dispatched.revision,
                    },
                    &Completion::Unknown {
                        reason: UncertaintyReason::Restart,
                    },
                )
                .await
                .unwrap()
        }

        async fn close(self) {
            self.writer.shutdown().await;
            self.pool.close().await;
        }
    }

    #[tokio::test]
    async fn diagnostics_read_frozen_attempts_without_current_manifest_and_filter_run() {
        let f = Fixture::new().await;
        let first = f.journal.prepare(&f.intent).await.unwrap();
        let mut foreign = f.intent.clone();
        foreign.run_id = Uuid::new_v4();
        foreign.operation_key = Uuid::new_v4();
        let foreign_attempt = DbXcodeEffectJournal::new(
            f.pool.clone(),
            f.writer.clone(),
            foreign.run_id,
            foreign.owner_lineage.clone(),
        )
        .prepare(&foreign)
        .await
        .unwrap();
        let mut next_intent = f.intent.clone();
        next_intent.operation_key = Uuid::new_v4();
        let next = f.journal.prepare(&next_intent).await.unwrap();
        let operator = Principal::new("operator", PrincipalClass::Operator);
        let page = f
            .admin
            .list(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                10,
                None,
            )
            .await
            .unwrap();
        assert_eq!(page.items, vec![first.clone(), next.clone()]);
        assert!(page.next_cursor.is_none());
        let page = f
            .admin
            .list(&operator, f.intent.run_id, &f.intent.owner_lineage, 1, None)
            .await
            .unwrap();
        assert_eq!(page.items, vec![first.clone()]);
        assert_eq!(page.next_cursor, Some(first.attempt_id.to_string()));
        let page = f
            .admin
            .list(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                1,
                page.next_cursor.as_deref(),
            )
            .await
            .unwrap();
        assert_eq!(page.items, vec![next]);
        assert!(page.next_cursor.is_none());
        assert!(f
            .admin
            .list(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                1,
                Some(&foreign_attempt.attempt_id.to_string())
            )
            .await
            .is_err());
        let read = f
            .admin
            .get(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                first.attempt_id,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.intent.result_schema.contract_id, "retired.result");
        assert!(f
            .admin
            .get(
                &operator,
                foreign.run_id,
                &f.intent.owner_lineage,
                first.attempt_id
            )
            .await
            .unwrap()
            .is_none());
        f.close().await;
    }

    #[tokio::test]
    async fn reconcile_is_operator_bound_and_compare_and_swap() {
        let f = Fixture::new().await;
        let unknown = f.unknown().await;
        let operator = Principal::new("authenticated-operator", PrincipalClass::Operator);
        let revision = AttemptRevision {
            attempt_id: unknown.attempt_id,
            revision: unknown.revision,
        };
        let evidence = Reconciliation {
            operator_id: "untrusted-body-claim".into(),
            inspected_state: RedactedText::new("Fixture inspected").unwrap(),
            proof: SettlementProof {
                evidence_ref: "fixture/settled".into(),
                active_operation_check_ref: "fixture/no-active".into(),
                no_operation_in_flight: true,
            },
            disposition: ReconciliationDisposition::Applied,
            outcome: HistoricalResult {
                schema_version: 1,
                schema: f.intent.result_schema.clone(),
                payload: HistoricalPayload::Result {
                    summary: RedactedText::new("Fixture applied").unwrap(),
                },
                result_digest: "f".repeat(64),
            },
        };
        let mut insufficient = evidence.clone();
        insufficient.proof.no_operation_in_flight = false;
        assert!(f
            .admin
            .reconcile(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                revision,
                &insufficient
            )
            .await
            .is_err());
        let held: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&f.pool)
            .await
            .unwrap();
        assert_eq!(held, 1);
        let done = f
            .admin
            .reconcile(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                revision,
                &evidence,
            )
            .await
            .unwrap();
        assert_eq!(done.state, AttemptState::Reconciled);
        assert_eq!(
            done.reconciliation.unwrap().operator_id,
            "authenticated-operator"
        );
        assert!(f
            .admin
            .reconcile(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                revision,
                &evidence
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("revision_conflict"));
        let holds: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&f.pool)
            .await
            .unwrap();
        assert_eq!(holds, 0);
        f.close().await;
    }

    #[tokio::test]
    async fn restricted_operator_cannot_read_or_reconcile_another_run() {
        let f = Fixture::new().await;
        let attempt = f.unknown().await;
        let mut operator = Principal::new("restricted", PrincipalClass::Operator);
        operator.has_explicit_surface_policies = true;
        operator.tool_capabilities =
            std::collections::BTreeSet::from([CapabilityToolId::XcodeEffectsDiagnostics]);
        operator.run_scope = Some(vec![f.intent.run_id.to_string()]);
        assert!(f
            .admin
            .get(
                &operator,
                f.intent.run_id,
                &f.intent.owner_lineage,
                attempt.attempt_id
            )
            .await
            .unwrap()
            .is_some());
        assert!(f
            .admin
            .list(&operator, Uuid::new_v4(), &f.intent.owner_lineage, 10, None)
            .await
            .is_err());
        let agent = Principal::new("agent", PrincipalClass::Agent);
        assert!(f
            .admin
            .get(
                &agent,
                f.intent.run_id,
                &f.intent.owner_lineage,
                attempt.attempt_id
            )
            .await
            .is_err());
        f.close().await;
    }

    #[tokio::test]
    async fn trust_target_must_belong_to_the_authorized_persisted_run_root() {
        let f = Fixture::new().await;
        let root = f._dir.path().canonicalize().unwrap().join("checkout");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("App.xcodeproj")).unwrap();
        std::fs::write(root.join("App.xcodeproj/project.pbxproj"), b"fixture").unwrap();
        let idea_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?, 'fixture', '', ?)")
            .bind(&idea_id)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?, ?, 'fixture', 'fixture', ?, ?, ?)")
            .bind(f.intent.run_id.to_string()).bind(idea_id).bind(root.to_str().unwrap())
            .bind(root.to_str().unwrap()).bind(chrono::Utc::now().to_rfc3339()).execute(&f.pool).await.unwrap();
        let resolved =
            acp::execution_root::resolve_execution_root(root.to_str().unwrap(), None, false, None)
                .unwrap();
        let project =
            TrustedProject::resolve(resolved, Some("App.xcodeproj"), unsafe { libc::geteuid() })
                .unwrap();
        let operator = Principal::new("operator", PrincipalClass::Operator);
        f.admin
            .authorize_trust_target(&operator, f.intent.run_id, &project)
            .await
            .unwrap();
        let mut scoped = Principal::new("run-scoped-trust", PrincipalClass::Operator);
        scoped.has_explicit_surface_policies = true;
        scoped.tool_capabilities =
            std::collections::BTreeSet::from([CapabilityToolId::XcodeProjectTrust]);
        scoped.run_scope = Some(vec![f.intent.run_id.to_string()]);
        assert!(f
            .admin
            .authorize_trust_target(&scoped, f.intent.run_id, &project)
            .await
            .is_err());
        assert!(f
            .admin
            .authorize_trust_target(&operator, Uuid::new_v4(), &project)
            .await
            .is_err());
        let broader = acp::execution_root::resolve_execution_root(
            f._dir.path().to_str().unwrap(),
            None,
            false,
            None,
        )
        .unwrap();
        let project = TrustedProject::resolve(broader, Some("checkout/App.xcodeproj"), unsafe {
            libc::geteuid()
        })
        .unwrap();
        assert!(f
            .admin
            .authorize_trust_target(&operator, f.intent.run_id, &project)
            .await
            .is_err());
        f.close().await;
    }
}
