/// P066 Phase 0 cleanup readback durability tests (T17 + T18).
///
/// Covers:
/// - startupRecoverySummary returns None when no readback exists
/// - startupRecoverySummary.toolchainCache fields are returned when present
/// - toolchainCacheHousekeepingSummary returns None before first sweep
/// - toolchainCacheHousekeepingSummary returns named fields after a sweep
use std::sync::Arc;

use async_graphql::Request;
use chrono::Utc;
use db::pool::create_pool;
use db::repos::startup_repairs::{
    record_startup_recovery_readback, StartupRecoveryReadback, ToolchainCacheRecoveryReadback,
};
use db::repos::toolchain_cache_housekeeping::{
    insert as insert_housekeeping, ToolchainCacheHousekeepingReadback,
};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

fn make_schema(pool: sqlx::SqlitePool) -> graphql_server::schema::AppSchema {
    let events = event_bus::new_bus(16);
    let handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        WorkQueue::new(pool.clone()),
    ));
    build_schema(
        pool,
        handler,
        events.clone(),
        auth::PrincipalTable::test_fixture(),
        LifecycleReporter::new(15, "test-build", events),
    )
}

fn operator_request(query: &str) -> Request {
    Request::new(query.to_string()).data(auth::Principal::new(
        "operator",
        auth::PrincipalClass::Operator,
    ))
}

/// T17: startupRecoverySummary returns None when no readback has been recorded.
#[tokio::test]
async fn p066_startup_recovery_summary_returns_none_when_empty() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let schema = make_schema(pool);

    let query = r#"{ startupRecoverySummary { id toolchainCache { sessionScopedRootsSeen } } }"#;
    let response = schema.execute(operator_request(query)).await;
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    assert!(
        data["startupRecoverySummary"].is_null(),
        "should be null when no readback recorded"
    );
}

/// T17: startupRecoverySummary.toolchainCache includes named fields when populated.
#[tokio::test]
async fn p066_startup_recovery_summary_includes_toolchain_cache_fields() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let now = Utc::now();

    let readback = StartupRecoveryReadback {
        id: uuid::Uuid::new_v4().to_string(),
        recovered_item_count: 3,
        queued_under_startup_recovery_backpressure_count: 0,
        oldest_recovered_queued_age_ms: None,
        affected_run_count: 2,
        next_retry_or_backoff_time: None,
        stale_after_ms: 60_000,
        updated_at: now,
        toolchain_cache: ToolchainCacheRecoveryReadback {
            session_scoped_roots_seen: Some(5),
            session_scoped_roots_reclaimed: Some(2),
            session_scoped_cleanup_failures: Some(0),
            orphan_threshold_minutes: Some(30),
            last_sweep_started_at: Some(now),
        },
    };
    record_startup_recovery_readback(&pool, &readback)
        .await
        .unwrap();

    let schema = make_schema(pool);
    let query = r#"{ startupRecoverySummary {
        id recoveredItemCount
        toolchainCache {
            sessionScopedRootsSeen
            sessionScopedRootsReclaimed
            sessionScopedCleanupFailures
            orphanThresholdMinutes
            lastSweepStartedAt
        }
    } }"#;

    let response = schema.execute(operator_request(query)).await;
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let summary = &data["startupRecoverySummary"];
    assert!(!summary.is_null(), "summary should be present");
    assert_eq!(summary["recoveredItemCount"], 3);

    let tc = &summary["toolchainCache"];
    assert_eq!(
        tc["sessionScopedRootsSeen"], 5,
        "sessionScopedRootsSeen must match stored value"
    );
    assert_eq!(
        tc["sessionScopedRootsReclaimed"], 2,
        "sessionScopedRootsReclaimed must match stored value"
    );
    assert_eq!(
        tc["sessionScopedCleanupFailures"], 0,
        "sessionScopedCleanupFailures must match stored value"
    );
    assert_eq!(
        tc["orphanThresholdMinutes"], 30,
        "orphanThresholdMinutes must match stored value"
    );
    assert!(
        tc["lastSweepStartedAt"].is_string(),
        "lastSweepStartedAt must be an ISO-8601 string"
    );
}

/// T17: toolchainCache fields are None when no sweep has run (pre-P066 rows).
#[tokio::test]
async fn p066_startup_recovery_summary_toolchain_cache_is_empty_before_sweep() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let now = Utc::now();

    let readback = StartupRecoveryReadback {
        id: uuid::Uuid::new_v4().to_string(),
        recovered_item_count: 0,
        queued_under_startup_recovery_backpressure_count: 0,
        oldest_recovered_queued_age_ms: None,
        affected_run_count: 0,
        next_retry_or_backoff_time: None,
        stale_after_ms: 60_000,
        updated_at: now,
        toolchain_cache: ToolchainCacheRecoveryReadback::default(),
    };
    record_startup_recovery_readback(&pool, &readback)
        .await
        .unwrap();

    let schema = make_schema(pool);
    let query = r#"{ startupRecoverySummary {
        toolchainCache {
            sessionScopedRootsSeen
            lastSweepStartedAt
        }
    } }"#;

    let response = schema.execute(operator_request(query)).await;
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let tc = &data["startupRecoverySummary"]["toolchainCache"];
    assert!(
        tc["sessionScopedRootsSeen"].is_null(),
        "sessionScopedRootsSeen must be null before first sweep"
    );
    assert!(
        tc["lastSweepStartedAt"].is_null(),
        "lastSweepStartedAt must be null before first sweep"
    );
}

/// T18: toolchainCacheHousekeepingSummary returns None before first sweep.
#[tokio::test]
async fn p066_toolchain_cache_housekeeping_summary_none_before_sweep() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let schema = make_schema(pool);

    let query = r#"{ toolchainCacheHousekeepingSummary { id lastSweepStartedAt } }"#;
    let response = schema.execute(operator_request(query)).await;
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    assert!(
        data["toolchainCacheHousekeepingSummary"].is_null(),
        "should be null before any sweep"
    );
}

/// T18: toolchainCacheHousekeepingSummary returns named fields after a sweep.
#[tokio::test]
async fn p066_toolchain_cache_housekeeping_summary_exposes_named_fields() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let now = Utc::now();

    let sweep = ToolchainCacheHousekeepingReadback {
        id: uuid::Uuid::new_v4().to_string(),
        last_sweep_started_at: now,
        run_scoped_roots_pruned: 7,
        run_scoped_prune_failures: 0,
        oldest_eligible_root_age_days: Some(3.5),
        disk_pressure_blocks: 0,
        quarantined_roots_created: 0,
        created_at: now,
    };
    insert_housekeeping(&pool, &sweep).await.unwrap();

    let schema = make_schema(pool);
    let query = r#"{ toolchainCacheHousekeepingSummary {
        id
        lastSweepStartedAt
        runScopedRootsPruned
        runScopedPruneFailures
        oldestEligibleRootAgeDays
        diskPressureBlocks
        quarantinedRootsCreated
        createdAt
    } }"#;

    let response = schema.execute(operator_request(query)).await;
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let summary = &data["toolchainCacheHousekeepingSummary"];
    assert!(!summary.is_null(), "summary should be present after sweep");
    assert_eq!(
        summary["runScopedRootsPruned"], 7,
        "runScopedRootsPruned must match"
    );
    assert_eq!(
        summary["runScopedPruneFailures"], 0,
        "runScopedPruneFailures must match"
    );
    assert!(
        summary["oldestEligibleRootAgeDays"].as_f64().is_some(),
        "oldestEligibleRootAgeDays must be a number"
    );
    assert!(
        (summary["oldestEligibleRootAgeDays"].as_f64().unwrap() - 3.5).abs() < 0.001,
        "oldestEligibleRootAgeDays must be ~3.5"
    );
    assert_eq!(summary["diskPressureBlocks"], 0);
    assert_eq!(summary["quarantinedRootsCreated"], 0);
    assert!(summary["lastSweepStartedAt"].is_string());
    assert!(summary["createdAt"].is_string());
}
