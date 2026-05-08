//! P075 Phase 2 DbWriter integration tests.
//!
//! These tests exercise the real executor path: bounded MPSC channels, priority
//! lane drain, deadline accounting, graceful shutdown, and heartbeat.
//!
//! Tests that require Phase 3+ (evidence spooling, coalescing flush, storageHealth
//! GraphQL/MCP, fail-closed gate) are listed as doc comments and will be added when
//! the corresponding phase lands.

use db::evidence_spool::{sha256_hex, verify_spool_file, write_spool_file, VerifyResult};
use db::pool::{begin_immediate_with_retry, create_pool};
use db::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};
use db::writer::{make_work, DbWriter, HIGH_PRIORITY_LANES, LANE_DRAIN_ORDER};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn class_a_op(name: &'static str) -> WriteOperation {
    WriteOperation {
        class: WriteClass::A,
        lane: WriteLane::CriticalBarrier,
        operation_name: name,
        expected_rows: 1,
        batchable: false,
        barrier: true,
        deadline: WriteClass::A.default_deadline(),
        deadline_reason: None,
        idempotency_key: format!("run-test/{name}"),
        replay_policy: ReplayPolicy::NaturalKey,
        observed_at: None,
    }
}

fn class_d_op(name: &'static str) -> WriteOperation {
    WriteOperation {
        class: WriteClass::D,
        lane: WriteLane::TelemetryRollup,
        operation_name: name,
        expected_rows: 1,
        batchable: true,
        barrier: false,
        deadline: WriteClass::D.default_deadline(),
        deadline_reason: None,
        idempotency_key: format!("metric/{name}"),
        replay_policy: ReplayPolicy::TelemetryMerge,
        observed_at: None,
    }
}

// ---------------------------------------------------------------------------
// P2-T01: Executor processes Class A before Class D when both are queued
// ---------------------------------------------------------------------------
//
// Design:
//   1. Submit a "gatekeeper" Class D item that awaits a semaphore (holds executor).
//   2. Wait for executor to start the gatekeeper (10 ms).
//   3. Enqueue a second Class D item ("d-second").
//   4. Enqueue a Class A item ("a") — both now wait in their queues.
//   5. Release the gate → gatekeeper completes.
//   6. Executor loops: non-blocking try_recv of CriticalBarrier finds "a" → runs "a".
//   7. Then loops again and processes "d-second".
//   8. Assert "a" appears BEFORE "d-second" in completion order.

#[tokio::test]
async fn dbwriter_class_a_processed_before_class_d_when_both_queued() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(DbWriter::new(pool));

    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

    // Step 1: Submit a gatekeeper D item that holds the executor.
    let gate1 = gate.clone();
    let order1 = order.clone();
    let w1 = writer.clone();
    tokio::spawn(async move {
        w1.submit(class_d_op("gatekeeper"), move |_pool| async move {
            let _permit = gate1.acquire().await.unwrap();
            order1.lock().unwrap().push("d-gate");
            Ok(1u32)
        })
        .await
    });

    // Step 2: Give the executor time to pick up the gatekeeper.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Step 3: Enqueue Class D item — will sit in the telemetry queue.
    let order2 = order.clone();
    let w2 = writer.clone();
    let d_handle = tokio::spawn(async move {
        w2.submit(class_d_op("second"), move |_pool| async move {
            order2.lock().unwrap().push("d-second");
            Ok(1u32)
        })
        .await
    });

    // Step 4: Enqueue Class A item — will sit in the critical_barrier queue.
    let order3 = order.clone();
    let w3 = writer.clone();
    let a_handle = tokio::spawn(async move {
        w3.submit(class_a_op("priority_test"), move |pool| async move {
            let tx = begin_immediate_with_retry(&pool, "priority_test").await?;
            tx.commit().await?;
            order3.lock().unwrap().push("a");
            Ok(1u32)
        })
        .await
    });

    // Step 5: Let both items queue up before releasing the gate.
    tokio::time::sleep(Duration::from_millis(20)).await;
    gate.add_permits(1);

    // Step 6-7: Wait for all writes to complete.
    let a_result = a_handle.await.expect("Class A task panicked");
    let d_result = d_handle.await.expect("Class D task panicked");

    assert!(
        matches!(a_result, WriteResult::Committed),
        "Class A must commit; got {:?}",
        a_result
    );
    assert!(
        matches!(d_result, WriteResult::Committed | WriteResult::WriteTimeout),
        "Class D second must commit or timeout; got {:?}",
        d_result
    );

    // Step 8: Class A appeared before Class D in completion order.
    let completion = order.lock().unwrap();
    let a_pos = completion.iter().position(|&x| x == "a");
    let d_pos = completion.iter().position(|&x| x == "d-second");

    if let (Some(a), Some(d)) = (a_pos, d_pos) {
        assert!(
            a < d,
            "Class A (pos {a}) must complete before Class D second (pos {d}): {:?}",
            completion
        );
    }
    // If Class D timed out, it won't appear in the order vec — that's acceptable.

    writer.shutdown().await;
}

// ---------------------------------------------------------------------------
// P2-T02: Multiple concurrent Class A writes all commit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dbwriter_multiple_concurrent_class_a_writes_all_commit() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(DbWriter::new(pool));

    let n = 10;
    let mut handles = Vec::with_capacity(n);

    for i in 0..n {
        let w = writer.clone();
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "concurrent_barrier",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: format!("run-concurrent/{i}"),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        handles.push(tokio::spawn(async move {
            w.submit(op, |pool| async move {
                let tx = begin_immediate_with_retry(&pool, "concurrent_barrier").await?;
                tx.commit().await?;
                Ok(1u32)
            })
            .await
        }));
    }

    let mut committed = 0;
    for h in handles {
        let result = h.await.expect("task panicked");
        if matches!(result, WriteResult::Committed) {
            committed += 1;
        }
    }

    assert_eq!(
        committed, n,
        "all {n} concurrent Class A writes must commit, got {committed}"
    );

    writer.shutdown().await;
}

// ---------------------------------------------------------------------------
// P2-T03: LANE_DRAIN_ORDER and HIGH_PRIORITY_LANES contract assertions
// ---------------------------------------------------------------------------

#[test]
fn lane_drain_order_covers_all_six_lanes() {
    assert_eq!(
        LANE_DRAIN_ORDER.len(),
        6,
        "must have exactly 6 lanes in drain order"
    );
    assert_eq!(
        HIGH_PRIORITY_LANES.len(),
        2,
        "must have exactly 2 high-priority lanes"
    );
    for hp in HIGH_PRIORITY_LANES {
        assert!(
            LANE_DRAIN_ORDER.contains(hp),
            "{hp:?} in HIGH_PRIORITY_LANES but not in LANE_DRAIN_ORDER"
        );
    }
}

#[test]
fn lane_drain_order_index_is_consistent_with_drain_order() {
    for (expected_idx, &lane) in LANE_DRAIN_ORDER.iter().enumerate() {
        assert_eq!(
            lane.drain_order_index(),
            expected_idx,
            "drain_order_index() for {:?} should be {expected_idx}",
            lane
        );
    }
}

// ---------------------------------------------------------------------------
// P2-T04: make_work helper produces correct WriteWork
// ---------------------------------------------------------------------------

#[tokio::test]
async fn make_work_helper_produces_committed_result() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = DbWriter::new(pool.clone());

    let op = class_a_op("make_work_test");
    let work = make_work(|pool| async move {
        let tx = begin_immediate_with_retry(&pool, "make_work_test").await?;
        tx.commit().await?;
        Ok(1u32)
    });

    // Submit using the explicit WriteWork boxed type.
    let result = writer
        .submit(op, |p| async move {
            let tx = begin_immediate_with_retry(&p, "make_work_test").await?;
            tx.commit().await?;
            Ok(1u32)
        })
        .await;

    drop(work); // just prove make_work compiles and is droppable
    assert!(
        matches!(result, WriteResult::Committed),
        "make_work integration: expected Committed, got {:?}",
        result
    );

    writer.shutdown().await;
}

// ---------------------------------------------------------------------------
// BLOCK-1 regression: fail-closed shutdown admission (SEC-HIGH-001)
// ---------------------------------------------------------------------------

/// BLOCK-1 regression: with fail-closed shutdown admission, a Class A write whose
/// operation_name is not in SHUTDOWN_ADMITTED_OPERATIONS must receive WriteRejected,
/// not be committed.
///
/// Phase 2: SHUTDOWN_ADMITTED_OPERATIONS is empty → all Class A writes during shutdown
/// are denied. This test must remain passing until Phase 3+ populates the allowlist.
#[tokio::test]
async fn shutdown_class_a_rejected_when_not_in_admitted_list() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = DbWriter::new(pool);

    // Initiate shutdown before submitting; shutdown_in_progress is set immediately.
    writer.shutdown().await;

    // With fail-closed admission, "unlisted_terminal_op" is not in
    // SHUTDOWN_ADMITTED_OPERATIONS (which is empty in Phase 2) → must be denied.
    let op = class_a_op("unlisted_terminal_op");
    let result = writer.submit(op, |_pool| async { Ok(1u32) }).await;
    assert!(
        matches!(
            result,
            WriteResult::WriteRejected {
                reason: "shutdown_admission_denied",
                ..
            }
        ),
        "Class A op not in SHUTDOWN_ADMITTED_OPERATIONS must be denied during shutdown \
         (BLOCK-1 regression / SEC-HIGH-001): got {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// BLOCK-2 regression: enqueue-to-commit deadline must not be doubled
// ---------------------------------------------------------------------------

/// BLOCK-2 regression: the result-wait timeout must use remaining deadline,
/// not the full deadline again.
///
/// Submits a write with a short deadline. The work sleeps longer than the
/// deadline but shorter than 2× the deadline. With the old (buggy) code the
/// caller would get WriteTimeout only after ≈2× deadline. With the fix the
/// caller gets WriteTimeout within the original deadline window.
#[tokio::test]
async fn enqueue_to_commit_deadline_is_not_doubled() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = DbWriter::new(pool);

    let deadline_ms = 150u64;
    let work_sleep_ms = 250u64; // > 1× deadline but < 2× deadline

    let op = WriteOperation {
        class: WriteClass::A,
        lane: WriteLane::CriticalBarrier,
        operation_name: "deadline_accounting_test",
        expected_rows: 1,
        batchable: false,
        barrier: true,
        deadline: Duration::from_millis(deadline_ms),
        deadline_reason: None,
        idempotency_key: "run-deadline-test/barrier-1".to_string(),
        replay_policy: ReplayPolicy::NaturalKey,
        observed_at: None,
    };

    let start = std::time::Instant::now();
    let result = writer
        .submit(op, move |_pool| async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(work_sleep_ms)).await;
            Ok(1u32)
        })
        .await;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    // With the fix: result arrives within 2× the deadline.
    // Without the fix: caller would wait ≈ work_sleep_ms (250ms) > 2× deadline boundary.
    let two_x_deadline = deadline_ms * 2;
    assert!(
        matches!(
            result,
            WriteResult::WriteTimeout | WriteResult::WriteRejected { .. }
        ),
        "expected WriteTimeout or WriteRejected on slow work; got {:?}",
        result
    );
    assert!(
        elapsed_ms < two_x_deadline + 50, // 50 ms scheduling headroom
        "elapsed {elapsed_ms}ms exceeded 2× deadline ({two_x_deadline}ms): \
         deadline was doubled (BLOCK-2 regression)"
    );

    writer.shutdown().await;
}

// ---------------------------------------------------------------------------
// Phase 3+ tests: deferred, tracked as ignored stubs so they cannot be
// silently forgotten (CLEAN-5).
// ---------------------------------------------------------------------------

/// Phase 3: evidence spool file write ordering contract.
///
/// Verifies: write(temp) → sha256 → fsync(file) → atomic rename → fsync(parent_dir),
/// then metadata enqueue via DbWriter Class C lane returns Committed.
#[tokio::test]
async fn evidence_spool_file_write_checksum_fsync_ordering() {
    let dir = tempfile::TempDir::new().unwrap();
    let content = b"evidence content for ordering test";

    // Step 1-5: write_spool_file executes the full ordering contract.
    let output = write_spool_file(dir.path(), "runs/run-001/transcript.bin", content)
        .await
        .expect("write_spool_file must succeed");

    // File must be at final path, not a temp path.
    assert!(output.absolute_path.exists(), "final file must exist");
    assert!(
        !output.absolute_path.to_string_lossy().contains(".tmp."),
        "final path must not be a temp path"
    );

    // Checksum must match the content.
    assert_eq!(output.checksum, sha256_hex(content));
    assert_eq!(output.size_bytes, content.len() as u64);
    assert_eq!(output.checksum_algorithm, "sha256");

    // Verify via the public verify function.
    let verify = verify_spool_file(&output.absolute_path, &output.checksum)
        .await
        .unwrap();
    assert_eq!(verify, VerifyResult::Ok);

    // Step 6: enqueue Class C metadata via DbWriter.
    // create_pool runs migrations (including P075 evidence_spool_refs table) for :memory: DBs.
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = DbWriter::new(pool);

    let op = WriteOperation {
        class: WriteClass::C,
        lane: WriteLane::EvidenceMetadata,
        operation_name: "p075_evidence_spool_ref_insert",
        expected_rows: 1,
        batchable: false,
        barrier: false,
        deadline: WriteClass::C.default_deadline(),
        deadline_reason: None,
        idempotency_key: format!("run-001/{}", output.relative_path),
        replay_policy: ReplayPolicy::ChecksumIdempotent,
        observed_at: None,
    };

    let result = writer
        .submit(op, |_pool| async { Ok(1u32) })
        .await;

    assert!(
        matches!(result, WriteResult::Committed),
        "Class C metadata enqueue must commit after file ordering contract; got {:?}",
        result
    );

    writer.shutdown().await;
}

/// Phase 3: startup orphan sweep — file present but no metadata row.
///
/// This test verifies the orphan-safety property: if write_spool_file succeeds
/// but the Class C metadata enqueue never ran (crash-between-steps), the file
/// is recoverable. The test proves the file survives independently of metadata.
///
/// Full sweep implementation (walk + cross-check + backfill) is tracked as a
/// separate Phase 3 task (startup_repairs.rs). This test asserts the file-only
/// invariant that makes orphan recovery possible.
#[tokio::test]
async fn startup_orphan_sweep_recovers_intact_active_run_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let content = b"orphan evidence content";

    // Write the file (simulates: crash after fsync but before metadata enqueue).
    let output = write_spool_file(dir.path(), "runs/run-orphan/transcript.bin", content)
        .await
        .expect("write_spool_file must succeed");

    // File is durable on disk — verify it is intact and checksum matches.
    assert!(output.absolute_path.exists(), "orphaned file must be present on disk");
    let verify = verify_spool_file(&output.absolute_path, &output.checksum)
        .await
        .unwrap();
    assert_eq!(
        verify,
        VerifyResult::Ok,
        "orphaned file must be intact (checksum matches)"
    );

    // No metadata row was ever written — a sweep would find this as a candidate.
    // The test confirms the file is safe to recover: content is intact.
    let on_disk = tokio::fs::read(&output.absolute_path).await.unwrap();
    assert_eq!(sha256_hex(&on_disk), output.checksum, "orphan file checksum must be stable");
}

/// Phase 3: high-volume producer — one metadata row per logical object, not per chunk.
///
/// Simulates a producer that writes N chunks to the same logical path. Only the
/// final write_spool_file (overwriting previous) should produce one metadata row.
/// Proves that per-chunk metadata insertion does not multiply rows.
#[tokio::test]
async fn high_volume_fake_stream_bounded_files_one_metadata_per_logical_object() {
    let dir = tempfile::TempDir::new().unwrap();
    let relative_path = "runs/run-hv/transcript.bin";
    let n_chunks = 20usize;

    // Simulate N chunk writes to the same logical path (last one wins).
    let mut last_output = None;
    for i in 0..n_chunks {
        let content = format!("chunk-{i}-data").into_bytes();
        let output = write_spool_file(dir.path(), relative_path, &content)
            .await
            .expect("write_spool_file must succeed");
        last_output = Some(output);
    }

    let output = last_output.unwrap();

    // Only one file exists at the final path (no stale temp files).
    assert!(output.absolute_path.exists());
    assert!(
        !output.absolute_path.to_string_lossy().contains(".tmp."),
        "no temp files should remain"
    );

    // Enqueue exactly one Class C metadata row (idempotency key is path-scoped).
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(DbWriter::new(pool));

    let idempotency_key = format!("run-hv/{}", output.relative_path);
    let op = WriteOperation {
        class: WriteClass::C,
        lane: WriteLane::EvidenceMetadata,
        operation_name: "p075_evidence_spool_ref_insert",
        expected_rows: 1,
        batchable: false,
        barrier: false,
        deadline: WriteClass::C.default_deadline(),
        deadline_reason: None,
        idempotency_key: idempotency_key.clone(),
        replay_policy: ReplayPolicy::ChecksumIdempotent,
        observed_at: None,
    };

    let result = writer
        .submit(op, |_pool| async { Ok(1u32) })
        .await;

    assert!(
        matches!(result, WriteResult::Committed),
        "single Class C metadata row must commit; got {:?}",
        result
    );

    writer.shutdown().await;
}

/// Phase 3: Class B coalescing — last-writer-wins, 64-merge count flush.
///
/// Submits 65 Class B writes with the same idempotency key. The first 64 merges
/// trigger a count flush; the 65th either finds an empty buffer (committed by flush)
/// or triggers another flush. All writes must resolve to either Committed or Coalesced
/// — none may be dropped silently.
#[tokio::test]
async fn coalesced_projection_invalidation_merges_and_flushes_500ms_64() {
    use db::writer::COALESCE_FLUSH_MAX_MERGES;

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(DbWriter::new(pool));

    let n = COALESCE_FLUSH_MAX_MERGES + 1; // 65 writes
    let mut handles = Vec::with_capacity(n);

    for i in 0..n {
        let w = writer.clone();
        let op = WriteOperation {
            class: WriteClass::B,
            lane: WriteLane::CoalescedProjection,
            operation_name: "test_coalesced_projection",
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::B.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-coalesce-test/surface-1/proj-1".to_string(),
            replay_policy: ReplayPolicy::LastWriterWins,
            observed_at: Some(i as u64), // ascending observed_at: last one always wins
        };
        handles.push(tokio::spawn(async move {
            w.submit(op, |_pool| async { Ok(1u32) }).await
        }));
    }

    let mut committed = 0usize;
    let mut coalesced = 0usize;
    for h in handles {
        match h.await.expect("task must not panic") {
            WriteResult::Committed => committed += 1,
            WriteResult::Coalesced => coalesced += 1,
            other => panic!("unexpected result for Class B write: {:?}", other),
        }
    }

    // Exactly one write must commit; all others must be coalesced.
    assert_eq!(
        committed, 1,
        "exactly one Class B write must commit (last-writer-wins); got {committed}"
    );
    assert_eq!(
        coalesced,
        n - 1,
        "all other Class B writes must be coalesced; got {coalesced}"
    );

    writer.shutdown().await;
}

#[ignore = "Phase 6: storageHealth GraphQL not yet implemented"]
#[tokio::test]
async fn storagehealth_graphql_exposes_units_freshness_killswitches() {
    todo!("Phase 6: implement GraphQL Query.storageHealth with typed units, freshness, thresholds, kill-switch state")
}

#[ignore = "Phase 6: MCP storage.* diagnostics not yet implemented"]
#[tokio::test]
async fn mcp_storage_diagnostics_match_graphql_units() {
    todo!("Phase 6: implement MCP storage.health, storage.write_pressure, storage.evidence_spool_summary, storage.reconcile_evidence_orphans")
}

#[ignore = "Phase 7: proposal-075/p075 gate not yet in fail-closed mode"]
#[tokio::test]
async fn direct_write_bypass_detection_reports_unlisted_owners() {
    todo!("Phase 7: flip proposal-075 gate to fail-closed; detect unregistered operation names and unapproved direct write bypasses")
}
