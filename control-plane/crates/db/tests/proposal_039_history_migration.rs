use db::migrate::MIGRATOR;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::path::Path;
use tempfile::tempdir;

async fn setup_test_pool(path: &Path) -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await
        .expect("failed to connect to test db")
}

#[tokio::test]
async fn migration_101_to_102_is_lossless_and_expands_ordinal_bound_to_512() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("history_migration.db");
    let pool = setup_test_pool(&db_path).await;

    // Apply migrations up to 101
    MIGRATOR.run_to(101, &pool).await.unwrap();

    let idea_prep = "01999999-0000-7000-8000-000000000001";
    let idea_act = "01999999-0000-7000-8000-000000000002";
    let source_prep = "01999999-0000-7000-8000-000000000011";
    let source_act = "01999999-0000-7000-8000-000000000012";
    let successor_act = "01999999-0000-7000-8000-000000000013";
    let art_seed_prep = "01999999-0000-7000-8000-000000000021";
    let art_ref1_prep = "01999999-0000-7000-8000-000000000022";
    let art_seed_act = "01999999-0000-7000-8000-000000000031";
    let art_ref1_act = "01999999-0000-7000-8000-000000000032";
    let art_ref128_act = "01999999-0000-7000-8000-000000000033";
    let op_prepared = "01999999-0000-7000-8000-000000000041";
    let op_activated = "01999999-0000-7000-8000-000000000042";

    // 1. Ideas
    for id in [idea_prep, idea_act] {
        sqlx::query("INSERT INTO ideas(id, title, body, created_at) VALUES (?, 'Idea', 'Body', '2026-09-27T00:00:00Z')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }

    // 2. Runs
    sqlx::query("INSERT INTO runs(id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at) VALUES (?, ?, 'blocked', 'wf', 'WF', '/repo', '/meta', '2026-09-27T00:00:00Z')")
        .bind(source_prep)
        .bind(idea_prep)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO runs(id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at) VALUES (?, ?, 'blocked', 'wf', 'WF', '/repo', '/meta', '2026-09-27T00:00:00Z')")
        .bind(source_act)
        .bind(idea_act)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO runs(id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at) VALUES (?, ?, 'pending', 'wf', 'WF', '/repo', '/meta', '2026-09-27T00:00:00Z')")
        .bind(successor_act)
        .bind(idea_act)
        .execute(&pool)
        .await
        .unwrap();

    // 3. Artifacts
    for (art_id, run_id, name, contract) in [
        (
            art_seed_prep,
            source_prep,
            "proposal_current",
            "proposal_current",
        ),
        (art_ref1_prep, source_prep, "findings_1", "findings_v1"),
        (
            art_seed_act,
            source_act,
            "proposal_current",
            "proposal_current",
        ),
        (art_ref1_act, source_act, "findings_1", "findings_v1"),
        (art_ref128_act, source_act, "findings_128", "findings_v1"),
    ] {
        sqlx::query("INSERT INTO artifacts(id, run_id, stage_id, agent_id, name, contract_id, format, file_path, provider, created_at) VALUES (?, ?, 'stage_0', 'agent_0', ?, ?, 'json', '/path', 'codex', '2026-09-27T00:00:00Z')")
            .bind(art_id)
            .bind(run_id)
            .bind(name)
            .bind(contract)
            .execute(&pool)
            .await
            .unwrap();
    }

    // 4. Command Journal
    for j_id in ["journal-1", "journal-2"] {
        sqlx::query("INSERT INTO command_journal(id, command_type, payload_json, created_at) VALUES (?, 'test', '{}', '2026-09-27T00:00:00Z')")
            .bind(j_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    // 5. Run Continuations
    let reserved_prep = "01999999-0000-7000-8000-000000000099";
    sqlx::query("INSERT INTO run_continuations(operation_id, source_run_id, idea_id, reserved_successor_run_id, caller_fingerprint, caller_request_id, intent_sha256, profile, phase, plan_ref, plan_sha256, target_ref, source_witness_sha256, journal_id, created_at, updated_at, deadline_at) VALUES (?, ?, ?, ?, 'caller', 'req-1', ?, 'implementation_restart_v1', 'preparing', 'plan.json', ?, 'target.json', ?, 'journal-1', '2026-09-27T00:00:00Z', '2026-09-27T00:00:00Z', '2026-09-28T00:00:00Z')")
        .bind(op_prepared)
        .bind(source_prep)
        .bind(idea_prep)
        .bind(reserved_prep)
        .bind("a".repeat(64))
        .bind("b".repeat(64))
        .bind("c".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO run_continuations(operation_id, source_run_id, idea_id, reserved_successor_run_id, successor_run_id, caller_fingerprint, caller_request_id, intent_sha256, profile, phase, plan_ref, plan_sha256, target_ref, source_witness_sha256, manifest_ref, manifest_sha256, journal_id, created_at, updated_at, deadline_at) VALUES (?, ?, ?, ?, ?, 'caller', 'req-2', ?, 'implementation_restart_v1', 'activated', 'plan.json', ?, 'target.json', ?, 'manifest.json', ?, 'journal-2', '2026-09-27T00:00:00Z', '2026-09-27T00:00:00Z', '2026-09-28T00:00:00Z')")
        .bind(op_activated)
        .bind(source_act)
        .bind(idea_act)
        .bind(successor_act)
        .bind(successor_act)
        .bind("a".repeat(64))
        .bind("b".repeat(64))
        .bind("c".repeat(64))
        .bind("d".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

    // 6. Seed run_continuation_inputs in Schema 101: both prepared (installed=0) and activated (installed=1)
    let seeded_rows = [
        // Prepared execution seed (ordinal 0, installed 0, successor NULL)
        (
            "input-p0",
            op_prepared,
            0,
            None,
            "execution_seed",
            "artifact",
            art_seed_prep,
            source_prep,
            art_seed_prep,
            "proposal_current",
            "1".repeat(64),
            "proposal_current",
            "proposals/current/proposal.md",
            0,
            "{}",
        ),
        // Prepared reference input (ordinal 1, installed 0, successor NULL)
        (
            "input-p1",
            op_prepared,
            1,
            None,
            "reference_only",
            "artifact",
            art_ref1_prep,
            source_prep,
            art_ref1_prep,
            "findings_v1",
            "2".repeat(64),
            "findings_1",
            "carry-forward/references/input-p1/content",
            0,
            "{}",
        ),
        // Installed execution seed (ordinal 0, installed 1, successor set)
        (
            "input-a0",
            op_activated,
            0,
            Some(successor_act),
            "execution_seed",
            "artifact",
            art_seed_act,
            source_act,
            art_seed_act,
            "proposal_current",
            "3".repeat(64),
            "proposal_current",
            "proposals/current/proposal.md",
            1,
            "{}",
        ),
        // Installed reference input (ordinal 1, installed 1, successor set)
        (
            "input-a1",
            op_activated,
            1,
            Some(successor_act),
            "reference_only",
            "artifact",
            art_ref1_act,
            source_act,
            art_ref1_act,
            "findings_v1",
            "4".repeat(64),
            "findings_1",
            "carry-forward/references/input-a1/content",
            1,
            "{}",
        ),
        // Installed reference input at boundary ordinal 128
        (
            "input-a128",
            op_activated,
            128,
            Some(successor_act),
            "reference_only",
            "artifact",
            art_ref128_act,
            source_act,
            art_ref128_act,
            "findings_v1",
            "5".repeat(64),
            "findings_128",
            "carry-forward/references/input-a128/content",
            1,
            "{}",
        ),
    ];

    for row in &seeded_rows {
        sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(row.0)
            .bind(row.1)
            .bind(row.2)
            .bind(row.3)
            .bind(row.4)
            .bind(row.5)
            .bind(row.6)
            .bind(row.7)
            .bind(row.8)
            .bind(row.9)
            .bind(&row.10)
            .bind(row.11)
            .bind(row.12)
            .bind(row.13)
            .bind(row.14)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Prove that in Schema 101, ordinal 129 is rejected by CHECK(ordinal>=0 AND ordinal<=128)
    let rejected_in_101 = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-too-large-101', ?, 129, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_129', 'carry-forward/references/129/content', 0, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("9".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        rejected_in_101.is_err(),
        "Schema 101 must reject ordinal 129"
    );

    // Execute Migration 102
    MIGRATOR.run_to(102, &pool).await.unwrap();

    let old_migrator = sqlx::migrate::Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|m| m.version <= 101)
            .cloned()
            .collect(),
    );
    assert!(matches!(
        old_migrator.run(&pool).await.unwrap_err(),
        sqlx::migrate::MigrateError::VersionMissing(102)
    ));
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_schema WHERE sql LIKE '%run_continuation_inputs_v1_backup%'")
        .fetch_one(&pool).await.unwrap(), 0);

    // Verify foreign key integrity
    let fk_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        fk_violations.is_empty(),
        "foreign key violations after migration 102: {fk_violations:?}"
    );

    // Assert lossless preservation of all seeded rows
    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM run_continuation_inputs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row_count, 5,
        "migration 102 must preserve all 5 seeded rows"
    );

    for expected in &seeded_rows {
        let row = sqlx::query("SELECT * FROM run_continuation_inputs WHERE input_id = ?")
            .bind(expected.0)
            .fetch_one(&pool)
            .await
            .unwrap();

        let operation_id: String = row.get("operation_id");
        let ordinal: i64 = row.get("ordinal");
        let successor_run_id: Option<String> = row.get("successor_run_id");
        let role: String = row.get("role");
        let source_kind: String = row.get("source_kind");
        let source_id: String = row.get("source_id");
        let source_run_id_val: String = row.get("source_run_id");
        let original_artifact_id: String = row.get("original_artifact_id");
        let source_schema: String = row.get("source_schema");
        let content_sha256: String = row.get("content_sha256");
        let target_logical_name: String = row.get("target_logical_name");
        let target_relative_path: String = row.get("target_relative_path");
        let installed: i64 = row.get("installed");
        let historical_relation_json: String = row.get("historical_relation_json");

        assert_eq!(operation_id, expected.1);
        assert_eq!(ordinal, expected.2);
        assert_eq!(successor_run_id.as_deref(), expected.3);
        assert_eq!(role, expected.4);
        assert_eq!(source_kind, expected.5);
        assert_eq!(source_id, expected.6);
        assert_eq!(source_run_id_val, expected.7);
        assert_eq!(original_artifact_id, expected.8);
        assert_eq!(source_schema, expected.9);
        assert_eq!(content_sha256, expected.10);
        assert_eq!(target_logical_name, expected.11);
        assert_eq!(target_relative_path, expected.12);
        assert_eq!(installed, expected.13);
        assert_eq!(historical_relation_json, expected.14);
    }

    // Verify Schema 102: ordinal 512 is accepted
    let insert_512 = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-512', ?, 512, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_512', 'carry-forward/references/512/content', 0, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("6".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        insert_512.is_ok(),
        "Schema 102 must accept ordinal 512: {:?}",
        insert_512.err()
    );

    // Verify Schema 102: ordinal 513 is denied by CHECK(ordinal>=0 AND ordinal<=512)
    let insert_513 = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-513', ?, 513, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_513', 'carry-forward/references/513/content', 0, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("7".repeat(64))
        .execute(&pool)
        .await;
    assert!(insert_513.is_err(), "Schema 102 must reject ordinal 513");

    // Verify Schema 102: negative ordinal is denied
    let insert_negative = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-neg', ?, -1, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_neg', 'carry-forward/references/neg/content', 0, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("8".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        insert_negative.is_err(),
        "Schema 102 must reject negative ordinal"
    );

    // Verify Schema 102: run_continuation_input_seed partial unique index is enforced
    // op_prepared already has an execution_seed at ordinal 0. Attempting to add another execution_seed must fail.
    let duplicate_seed = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-dup-seed', ?, 2, NULL, 'execution_seed', 'artifact', ?, ?, ?, 'proposal_current', ?, 'proposal_current', 'proposals/current/another.md', 0, '{}')")
        .bind(op_prepared)
        .bind(art_seed_prep)
        .bind(source_prep)
        .bind(art_seed_prep)
        .bind("a".repeat(64))
        .execute(&pool)
        .await;
    assert!(duplicate_seed.is_err(), "run_continuation_input_seed partial unique index must deny duplicate execution_seed per operation");

    // Verify Schema 102: UNIQUE(operation_id, ordinal) is enforced
    let duplicate_ordinal = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-dup-ord', ?, 1, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_unique', 'carry-forward/references/unique/content', 0, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("b".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        duplicate_ordinal.is_err(),
        "UNIQUE(operation_id, ordinal) must deny duplicate ordinal"
    );

    // Verify Schema 102: UNIQUE(operation_id, target_relative_path) is enforced
    let duplicate_path = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('input-dup-path', ?, 10, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_1', 'carry-forward/references/input-p1/content', 0, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("c".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        duplicate_path.is_err(),
        "UNIQUE(operation_id, target_relative_path) must deny duplicate path"
    );

    // Verify Schema 102: installed constraint: installed=0 requires successor_run_id IS NULL
    let invalid_installed_0 = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('bad-inst-0', ?, 20, ?, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_bad', 'carry-forward/references/bad0/content', 0, '{}')")
        .bind(op_prepared)
        .bind(successor_act)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("d".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        invalid_installed_0.is_err(),
        "installed=0 with successor_run_id must be denied"
    );

    // Verify Schema 102: installed constraint: installed=1 requires successor_run_id IS NOT NULL
    let invalid_installed_1 = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('bad-inst-1', ?, 21, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_bad', 'carry-forward/references/bad1/content', 1, '{}')")
        .bind(op_prepared)
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("e".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        invalid_installed_1.is_err(),
        "installed=1 with NULL successor_run_id must be denied"
    );

    // Verify Schema 102: Foreign key checks still enforced
    let invalid_fk = sqlx::query("INSERT INTO run_continuation_inputs(input_id, operation_id, ordinal, successor_run_id, role, source_kind, source_id, source_run_id, original_artifact_id, source_schema, content_sha256, target_logical_name, target_relative_path, installed, historical_relation_json) VALUES ('bad-fk', 'non-existent-op', 30, NULL, 'reference_only', 'artifact', ?, ?, ?, 'findings_v1', ?, 'findings_bad_fk', 'carry-forward/references/badfk/content', 0, '{}')")
        .bind(art_ref1_prep)
        .bind(source_prep)
        .bind(art_ref1_prep)
        .bind("f".repeat(64))
        .execute(&pool)
        .await;
    assert!(
        invalid_fk.is_err(),
        "Invalid foreign key operation_id must be denied"
    );
}
