//! P075 evidence spool file writer.
//!
//! Implements the file-before-metadata ordering contract that must complete
//! before any Class C metadata is enqueued to [`crate::writer::DbWriter`]:
//!
//! 1. Write content to a deterministic temp path under the final directory.
//! 2. Compute SHA-256 checksum.
//! 3. `fsync(temp_file)` — flush content to durable storage.
//! 4. Atomic `rename(temp → final)`.
//! 5. `fsync(parent_dir)` — make the rename durable.
//!
//! After [`write_spool_file`] returns `Ok(SpoolFileOutput)`, the caller must
//! enqueue a Class C metadata write via `DbWriter`. If metadata enqueue fails,
//! the already-fsynced file is orphan-safe: its checksum and size are stable on
//! disk and a future startup sweep can backfill the metadata row (Phase 4+).
//!
//! # Path security
//!
//! `relative_path` is validated by
//! [`crate::repos::evidence_spool_refs::validate_relative_path`] before any
//! filesystem access. Absolute paths, `..` traversal, empty segments, and
//! backslashes are all rejected (P075-SEC-001).
//!
//! # No-clobber commit
//!
//! The temp filename is `{filename}.tmp.{uuid4}`. UUID v4 makes collisions
//! negligible. If the final path already exists (idempotent retry), the
//! content is compared without following symlinks and within a size cap:
//! same checksum + same size → idempotent skip (temp cleaned up); differing
//! bytes → hard error. A leaf symlink at `final_path` is always rejected,
//! even if it points inside `canonical_root`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use sha2::Digest;
use sqlx::SqlitePool;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::repos::evidence_spool_refs::{
    find_by_run_and_path, insert_idempotent, redact_path, validate_path_ownership,
    validate_relative_path, EvidenceKind, EvidenceSpoolRef, EvidenceSpoolRefStatus,
};

// ─── Output types ────────────────────────────────────────────────────────────

/// Result of a successful [`write_spool_file`] call.
#[derive(Debug, Clone)]
pub struct SpoolFileOutput {
    /// Absolute filesystem path of the written file.
    pub absolute_path: PathBuf,
    /// Path relative to `artifact_root`, normalized to forward slashes.
    /// Passes [`validate_relative_path`] without modification.
    pub relative_path: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Always `"sha256"`.
    pub checksum_algorithm: &'static str,
    /// 64 lowercase hex characters (SHA-256 of the content bytes).
    pub checksum: String,
}

/// Result of a [`verify_spool_file`] call.
#[derive(Debug, PartialEq, Eq)]
pub enum VerifyResult {
    /// File exists and checksum matches.
    Ok,
    /// File does not exist (orphan candidate).
    Missing,
    /// File exists but checksum does not match the stored metadata.
    ChecksumMismatch {
        /// Actual checksum of the on-disk file.
        actual: String,
    },
}

/// Summary returned by [`sweep_evidence_orphans`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OrphanSweepReport {
    /// Candidate files found under the canonical evidence spool tree.
    pub scanned_files: u64,
    /// Files that already had a compact metadata row.
    pub already_indexed: u64,
    /// Missing metadata rows backfilled as `recovered_orphan`.
    pub recovered_orphans: u64,
    /// Files skipped because they are temp files, invalid paths, oversized, or unknown kinds.
    pub skipped_files: u64,
}

/// Maximum file size that [`verify_spool_file`] will read into memory (SEC-P075-004).
/// Files larger than this cap return `Err` rather than allocating unbounded RAM.
pub const VERIFY_SIZE_CAP_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB

// ─── Public API ──────────────────────────────────────────────────────────────

/// Write evidence content to a spool file following the P075 ordering contract.
///
/// `run_id` must match the run segment of `relative_path` —
/// `evidence/runs/{run_id}/...` — so that ownership is validated before any
/// filesystem work (H-002). A path belonging to a different run is rejected
/// before the first directory is created.
///
/// # Ordering
///
/// ```text
/// validate_path_ownership → write(temp) → sha256 → fsync(temp) → rename(temp→final) → fsync(parent_dir)
/// ```
///
/// On any failure the temp file is cleaned up before returning `Err`.
///
/// # Returns
///
/// [`SpoolFileOutput`] with the final path, size, and SHA-256 checksum.
/// The caller **must** enqueue a Class C metadata write via `DbWriter` after
/// this returns, using the returned `checksum` and `size_bytes`.
pub async fn write_spool_file(
    artifact_root: &Path,
    run_id: &str,
    relative_path: &str,
    content: &[u8],
) -> Result<SpoolFileOutput> {
    // Validate before any filesystem work (P075-SEC-001).
    validate_relative_path(relative_path).context("validate relative_path before spool write")?;

    // Enforce canonical evidence layout: all spool paths must be under evidence/runs/.
    // Layout: evidence/runs/{run_id}/stages/{stage_id}/agents/{agent_id}/{kind}/...
    // Per P075 §architecture.evidence_spooling.layout (P075-SEC-002).
    if !relative_path.starts_with("evidence/runs/") {
        return Err(anyhow::anyhow!(
            "evidence spool path must start with 'evidence/runs/' ({}); \
             use the canonical layout: \
             evidence/runs/{{run_id}}/stages/{{stage_id}}/agents/{{agent_id}}/{{kind}}/...",
            redact_path(relative_path)
        ));
    }

    // Bind path to run_id before any filesystem access (H-002).
    // A producer with the wrong run_id cannot leave orphans under another run's tree.
    validate_path_ownership(relative_path, run_id)
        .context("validate run_id ownership of relative_path before spool write")?;

    // Canonicalize artifact_root so all subsequent path operations use resolved paths (H-001).
    // artifact_root must already exist; guaranteed by daemon startup.
    let canonical_root = tokio::fs::canonicalize(artifact_root)
        .await
        .context("canonicalize artifact_root for path containment check")?;

    let absolute_path = canonical_root.join(relative_path);

    let parent = absolute_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("spool path has no parent directory"))?
        .to_path_buf();

    // Create parent directories with mode 0o700 — no group/world access regardless of umask (H-002).
    #[cfg(unix)]
    tokio::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&parent)
        .await
        .context("create evidence spool parent directories")?;
    #[cfg(not(unix))]
    tokio::fs::create_dir_all(&parent)
        .await
        .context("create evidence spool parent directories")?;

    // After creation: verify parent is within canonical_root (detects symlink escapes — H-001).
    // A symlinked intermediate directory (e.g. evidence/ → /etc/) would have a canonical path
    // that does not start with canonical_root, so this check catches it before any write.
    let canonical_parent = tokio::fs::canonicalize(&parent)
        .await
        .context("canonicalize spool parent for containment check")?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(anyhow::anyhow!(
            "evidence spool path escapes artifact root after symlink resolution (P075-SEC-H001)"
        ));
    }

    // Deterministic temp path: {parent}/{filename}.tmp.{uuid4}
    let file_name = absolute_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("spool path has no file name component"))?
        .to_string_lossy();
    let temp_name = format!("{}.tmp.{}", file_name, Uuid::new_v4());
    let temp_path = canonical_parent.join(&temp_name);
    let final_path = canonical_parent.join(
        absolute_path
            .file_name()
            .expect("validated above: file_name is Some"),
    );

    // Perform write with cleanup on failure.
    match write_and_commit(content, &temp_path, &final_path, &canonical_parent).await {
        Ok(checksum) => Ok(SpoolFileOutput {
            absolute_path: final_path,
            relative_path: relative_path.to_string(),
            size_bytes: content.len() as u64,
            checksum_algorithm: "sha256",
            checksum,
        }),
        Err(e) => {
            // Best-effort cleanup; ignore cleanup errors to preserve the root cause.
            let _ = tokio::fs::remove_file(&temp_path).await;
            Err(e)
        }
    }
}

/// Verify that a spool file at `absolute_path` matches `expected_checksum`.
///
/// Returns:
/// - [`VerifyResult::Ok`] — file present, checksum matches.
/// - [`VerifyResult::Missing`] — file absent; treated as an orphan candidate by readers.
/// - [`VerifyResult::ChecksumMismatch`] — file present but checksum differs.
/// - `Err` — I/O error other than not-found.
pub async fn verify_spool_file(
    absolute_path: &Path,
    expected_checksum: &str,
) -> Result<VerifyResult> {
    // SEC-P075-004: stat before read to enforce the size cap. Prevents unbounded RAM
    // allocation if a large or attacker-controlled file path is passed by orphan sweep.
    let meta = match tokio::fs::metadata(absolute_path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(VerifyResult::Missing),
        Err(e) => return Err(anyhow::Error::from(e).context("stat spool file for verification")),
        Ok(m) => m,
    };
    if meta.len() > VERIFY_SIZE_CAP_BYTES {
        return Err(anyhow::anyhow!(
            "spool file exceeds verify size cap ({} bytes > {} MiB cap); \
             use streaming verification for large artifacts",
            meta.len(),
            VERIFY_SIZE_CAP_BYTES / (1024 * 1024)
        ));
    }
    match tokio::fs::read(absolute_path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(VerifyResult::Missing),
        Err(e) => Err(anyhow::Error::from(e).context("read spool file for verification")),
        Ok(bytes) => {
            let actual = sha256_hex(&bytes);
            if actual == expected_checksum {
                Ok(VerifyResult::Ok)
            } else {
                Ok(VerifyResult::ChecksumMismatch { actual })
            }
        }
    }
}

/// Walk `artifact_root/evidence/runs` and backfill metadata for intact evidence files
/// that were durably written before a crash prevented the Class C metadata insert.
///
/// This is conservative by design: only canonical
/// `evidence/runs/{run}/stages/{stage}/agents/{agent}/{kind}/...` files are
/// eligible, temp files are ignored, files above [`VERIFY_SIZE_CAP_BYTES`] are
/// skipped, and unknown evidence kind directories are not guessed.
pub async fn sweep_evidence_orphans(
    pool: &SqlitePool,
    artifact_root: &Path,
) -> Result<OrphanSweepReport> {
    let root = artifact_root.join("evidence").join("runs");
    if !root.exists() {
        return Ok(OrphanSweepReport::default());
    }

    let mut report = OrphanSweepReport::default();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                report.skipped_files += 1;
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    report.skipped_files += 1;
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => {
                    report.skipped_files += 1;
                    continue;
                }
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                report.skipped_files += 1;
                continue;
            }

            match recover_orphan_candidate(pool, artifact_root, &path).await {
                Ok(OrphanCandidateOutcome::AlreadyIndexed) => {
                    report.scanned_files += 1;
                    report.already_indexed += 1;
                }
                Ok(OrphanCandidateOutcome::Recovered) => {
                    report.scanned_files += 1;
                    report.recovered_orphans += 1;
                }
                Ok(OrphanCandidateOutcome::Skipped) => {
                    report.skipped_files += 1;
                }
                Err(_) => {
                    report.skipped_files += 1;
                }
            }
        }
    }

    Ok(report)
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

enum OrphanCandidateOutcome {
    AlreadyIndexed,
    Recovered,
    Skipped,
}

async fn recover_orphan_candidate(
    pool: &SqlitePool,
    artifact_root: &Path,
    absolute_path: &Path,
) -> Result<OrphanCandidateOutcome> {
    let relative_path = match absolute_path.strip_prefix(artifact_root) {
        Ok(path) => path.to_string_lossy().replace('\\', "/"),
        Err(_) => return Ok(OrphanCandidateOutcome::Skipped),
    };
    if relative_path.contains(".tmp.") {
        return Ok(OrphanCandidateOutcome::Skipped);
    }
    if validate_relative_path(&relative_path).is_err() {
        return Ok(OrphanCandidateOutcome::Skipped);
    }

    let Some(layout) = parse_evidence_layout(&relative_path) else {
        return Ok(OrphanCandidateOutcome::Skipped);
    };
    if find_by_run_and_path(pool, layout.run_id, &relative_path)
        .await?
        .is_some()
    {
        return Ok(OrphanCandidateOutcome::AlreadyIndexed);
    }

    let meta = std::fs::metadata(absolute_path).context("stat orphan evidence candidate")?;
    if meta.len() > VERIFY_SIZE_CAP_BYTES {
        return Ok(OrphanCandidateOutcome::Skipped);
    }
    let bytes = std::fs::read(absolute_path).context("read orphan evidence candidate")?;
    let checksum = sha256_hex(&bytes);
    let spool_ref = EvidenceSpoolRef {
        id: recovered_orphan_id(&relative_path, &checksum),
        metadata_version: 1,
        run_id: layout.run_id.to_string(),
        stage_execution_id: None,
        stage_id: Some(layout.stage_id.to_string()),
        agent_execution_id: None,
        agent_id: Some(layout.agent_id.to_string()),
        kind: layout.kind,
        relative_path,
        size_bytes: meta.len() as i64,
        checksum_algorithm: "sha256".to_string(),
        checksum,
        producer_operation: "p075_evidence_spool_ref_recovery_sweep".to_string(),
        content_type: None,
        summary_json: None,
        created_at: Utc::now(),
        status: EvidenceSpoolRefStatus::RecoveredOrphan,
    };
    insert_idempotent(pool, &spool_ref)
        .await
        .context("backfill recovered orphan evidence metadata")?;
    Ok(OrphanCandidateOutcome::Recovered)
}

struct EvidenceLayout<'a> {
    run_id: &'a str,
    stage_id: &'a str,
    agent_id: &'a str,
    kind: EvidenceKind,
}

fn parse_evidence_layout(relative_path: &str) -> Option<EvidenceLayout<'_>> {
    let mut segments = relative_path.split('/');
    if segments.next()? != "evidence" || segments.next()? != "runs" {
        return None;
    }
    let run_id = segments.next()?;
    if segments.next()? != "stages" {
        return None;
    }
    let stage_id = segments.next()?;
    if segments.next()? != "agents" {
        return None;
    }
    let agent_id = segments.next()?;
    let kind = evidence_kind_from_dir(segments.next()?)?;
    if segments.next().is_none() {
        return None;
    }
    Some(EvidenceLayout {
        run_id,
        stage_id,
        agent_id,
        kind,
    })
}

fn evidence_kind_from_dir(dir: &str) -> Option<EvidenceKind> {
    match dir {
        "transcripts" | "transcript" => Some(EvidenceKind::Transcript),
        "tool_traces" | "tool_trace" => Some(EvidenceKind::ToolTrace),
        "stdout" => Some(EvidenceKind::Stdout),
        "stderr" => Some(EvidenceKind::Stderr),
        "receipts" | "receipt" => Some(EvidenceKind::Receipt),
        "runtime_events" | "runtime_event" => Some(EvidenceKind::RuntimeEvent),
        "model_deltas" | "model_delta" => Some(EvidenceKind::ModelDelta),
        "delivery_readbacks" | "delivery_readback" => Some(EvidenceKind::DeliveryReadback),
        _ => None,
    }
}

fn recovered_orphan_id(relative_path: &str, checksum: &str) -> String {
    let path_hash = sha256_hex(relative_path.as_bytes());
    format!(
        "evsp_recovered_{}_{}",
        &checksum[..checksum.len().min(32)],
        &path_hash[..16]
    )
}

/// Write, checksum, fsync(file), rename, fsync(parent_dir).
/// Returns the SHA-256 hex digest on success.
async fn write_and_commit(
    content: &[u8],
    temp_path: &Path,
    final_path: &Path,
    parent_dir: &Path,
) -> Result<String> {
    // Step 1 – write content to temp file with mode 0o600 (no group/world access — H-002).
    #[cfg(unix)]
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(temp_path)
        .await
        .context("create temp spool file")?;
    #[cfg(not(unix))]
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .context("create temp spool file")?;
    file.write_all(content)
        .await
        .context("write content to temp spool file")?;

    // Step 2 – compute SHA-256 checksum.
    let checksum = sha256_hex(content);

    // Step 3 – fsync(file): flush data to durable storage before rename.
    file.flush().await.context("flush temp spool file")?;
    file.sync_all().await.context("fsync temp spool file")?;
    drop(file);

    // Step 4 – no-clobber commit: if final_path already exists, compare checksums
    // before touching it (SEC-P075-002). Overwriting committed evidence before the
    // metadata idempotency check runs can silently destroy durable content.
    //
    // We use symlink_metadata so that a leaf symlink at final_path is detected
    // without following it (H-001). A symlink is never a valid committed evidence
    // file — reject it regardless of where it points.
    //
    // • Symlink at final_path → hard error; leaf-symlink attack is not an idempotent retry.
    // • File larger than VERIFY_SIZE_CAP_BYTES → hard error; refuse unbounded RAM allocation.
    // • Same checksum + same size → idempotent retry; skip rename, clean up temp.
    // • Content differs → hard error; caller must reconcile via storage.reconcile_evidence_orphans.
    match tokio::fs::symlink_metadata(final_path).await {
        Ok(meta) => {
            // Clean up temp before returning either way.
            let _ = tokio::fs::remove_file(temp_path).await;
            if meta.file_type().is_symlink() {
                return Err(anyhow::anyhow!(
                    "evidence_spool: final_path is a symlink; leaf-symlink is not a \
                     valid committed evidence file (SEC-P075-H001)"
                ));
            }
            if meta.len() > VERIFY_SIZE_CAP_BYTES {
                return Err(anyhow::anyhow!(
                    "evidence_spool: existing final_path exceeds size cap ({} bytes > {} MiB); \
                     refusing no-clobber read to prevent unbounded RAM allocation (SEC-P075-004)",
                    meta.len(),
                    VERIFY_SIZE_CAP_BYTES / (1024 * 1024)
                ));
            }
            let existing_bytes = tokio::fs::read(final_path)
                .await
                .context("no-clobber: read existing final_path for checksum comparison")?;
            let existing_checksum = sha256_hex(&existing_bytes);
            if existing_checksum == checksum && existing_bytes.len() as u64 == content.len() as u64
            {
                // Idempotent: the committed file already contains the same bytes.
                return Ok(checksum);
            }
            return Err(anyhow::anyhow!(
                "evidence_spool: final_path already exists with different content \
                 (SEC-P075-002); use storage.reconcile_evidence_orphans to resolve. \
                 existing_checksum_prefix={} new_checksum_prefix={}",
                &existing_checksum[..existing_checksum.len().min(12)],
                &checksum[..checksum.len().min(12)]
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Final path does not exist yet — proceed with the rename below.
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(anyhow::Error::from(e).context("no-clobber: stat final_path before commit"));
        }
    }

    // atomic rename temp → final.
    tokio::fs::rename(temp_path, final_path)
        .await
        .context("atomic rename temp to final spool path")?;

    // Step 5 – fsync(parent_dir): make the rename entry durable.
    {
        let dir = tokio::fs::File::open(parent_dir)
            .await
            .context("open parent dir for fsync")?;
        dir.sync_all()
            .await
            .context("fsync parent directory after spool rename")?;
    }

    Ok(checksum)
}

/// Compute SHA-256 and return 64 lowercase hex characters.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn write_spool_file_produces_correct_checksum_and_size() {
        let dir = TempDir::new().unwrap();
        let content = b"hello evidence spool";
        let output = write_spool_file(
            dir.path(),
            "run-test",
            "evidence/runs/run-test/stages/s-1/agents/a-1/transcripts/test.bin",
            content,
        )
        .await
        .expect("write_spool_file should succeed");

        assert_eq!(output.size_bytes, content.len() as u64);
        assert_eq!(output.checksum_algorithm, "sha256");
        assert_eq!(output.checksum.len(), 64);
        // Verify file exists at final path
        assert!(output.absolute_path.exists());
        // Verify checksum matches the file content
        let on_disk = tokio::fs::read(&output.absolute_path).await.unwrap();
        assert_eq!(on_disk, content);
        assert_eq!(sha256_hex(&on_disk), output.checksum);
    }

    #[tokio::test]
    async fn write_spool_file_leaves_no_temp_file_on_success() {
        let dir = TempDir::new().unwrap();
        let spool_path = "evidence/runs/run-test/stages/s-1/agents/a-1/transcripts/clean.bin";
        write_spool_file(dir.path(), "run-test", spool_path, b"data")
            .await
            .unwrap();
        // No .tmp.* files should remain after success.
        let mut tmp_count = 0usize;
        let leaf_dir = dir
            .path()
            .join("evidence/runs/run-test/stages/s-1/agents/a-1/transcripts");
        let mut rd = tokio::fs::read_dir(&leaf_dir).await.unwrap();
        while let Some(entry) = rd.next_entry().await.unwrap() {
            if entry.file_name().to_string_lossy().contains(".tmp.") {
                tmp_count += 1;
            }
        }
        assert_eq!(tmp_count, 0, "no .tmp. files should remain after success");
    }

    #[tokio::test]
    async fn verify_spool_file_ok_when_matches() {
        let dir = TempDir::new().unwrap();
        let content = b"verify me";
        let output = write_spool_file(
            dir.path(),
            "run-v",
            "evidence/runs/run-v/stages/s-1/agents/a-1/receipts/check.bin",
            content,
        )
        .await
        .unwrap();
        let result = verify_spool_file(&output.absolute_path, &output.checksum)
            .await
            .unwrap();
        assert_eq!(result, VerifyResult::Ok);
    }

    #[tokio::test]
    async fn verify_spool_file_missing_when_absent() {
        let dir = TempDir::new().unwrap();
        let absent = dir.path().join("nonexistent.bin");
        let result = verify_spool_file(&absent, "aabbcc").await.unwrap();
        assert_eq!(result, VerifyResult::Missing);
    }

    #[tokio::test]
    async fn verify_spool_file_mismatch_when_content_changed() {
        let dir = TempDir::new().unwrap();
        let output = write_spool_file(
            dir.path(),
            "run-t",
            "evidence/runs/run-t/stages/s-1/agents/a-1/stdout/tamper.bin",
            b"original",
        )
        .await
        .unwrap();
        // Overwrite with different content.
        tokio::fs::write(&output.absolute_path, b"tampered")
            .await
            .unwrap();
        let result = verify_spool_file(&output.absolute_path, &output.checksum)
            .await
            .unwrap();
        assert!(matches!(result, VerifyResult::ChecksumMismatch { .. }));
    }

    #[tokio::test]
    async fn write_spool_file_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let result = write_spool_file(dir.path(), "run-x", "evidence/../secret.bin", b"data").await;
        assert!(result.is_err(), "path traversal must be rejected");
    }

    #[tokio::test]
    async fn write_spool_file_rejects_absolute_path() {
        let dir = TempDir::new().unwrap();
        let result = write_spool_file(dir.path(), "run-x", "/etc/passwd", b"data").await;
        assert!(result.is_err(), "absolute path must be rejected");
    }

    #[tokio::test]
    async fn write_spool_file_creates_nested_directories() {
        let dir = TempDir::new().unwrap();
        let path = "evidence/runs/run-001/stages/s-01/agents/a-01/transcripts/chunk.bin";
        let output = write_spool_file(dir.path(), "run-001", path, b"nested")
            .await
            .expect("nested path should succeed");
        assert!(output.absolute_path.exists());
    }

    // ── Security regression tests ─────────────────────────────────────────────

    /// H-001 regression: a symlinked intermediate directory escaping artifact_root
    /// must be rejected before any file write occurs.
    ///
    /// Uses a canonical evidence/runs/... path so the layout check passes and the
    /// symlink-escape check is the gate under test.
    #[cfg(unix)]
    #[tokio::test]
    async fn write_spool_file_rejects_symlinked_directory_escaping_root() {
        let artifact_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        // Create evidence/ as a symlink pointing outside artifact_root.
        let link_path = artifact_dir.path().join("evidence");
        std::os::unix::fs::symlink(outside_dir.path(), &link_path).unwrap();

        // Path passes layout check (starts with evidence/runs/) but evidence/ is a symlink
        // escaping the artifact root → must be rejected by the containment check.
        let result = write_spool_file(
            artifact_dir.path(),
            "run-1",
            "evidence/runs/run-1/stages/s-1/agents/a-1/transcripts/secret.bin",
            b"data",
        )
        .await;
        assert!(
            result.is_err(),
            "symlink-to-outside-root must be rejected (P075-SEC-H001): got Ok({:?})",
            result.ok()
        );

        // File must NOT have been written anywhere under the symlink target.
        assert!(
            !outside_dir
                .path()
                .join("runs/run-1/stages/s-1/agents/a-1/transcripts/secret.bin")
                .exists(),
            "file must not be written to symlink target"
        );
    }

    /// H-002 regression: spool files must be created with mode 0o600.
    #[cfg(unix)]
    #[tokio::test]
    async fn write_spool_file_creates_files_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();

        let output = write_spool_file(
            dir.path(),
            "run-p",
            "evidence/runs/run-p/stages/s-1/agents/a-1/transcripts/perm-test.bin",
            b"mode check",
        )
        .await
        .expect("write must succeed");

        let mode = std::fs::metadata(&output.absolute_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "spool file must be 0600, got {:o}", mode);
    }

    /// H-002 regression: spool directories must be created with mode 0o700.
    #[cfg(unix)]
    #[tokio::test]
    async fn write_spool_file_creates_directories_with_mode_0700() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();

        write_spool_file(
            dir.path(),
            "run-d",
            "evidence/runs/run-d/stages/s-1/agents/a-1/stdout/perm-test.bin",
            b"dir mode check",
        )
        .await
        .expect("write must succeed");

        let ev_dir = dir.path().join("evidence");
        let ev_mode = std::fs::metadata(&ev_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(ev_mode, 0o700, "evidence/ must be 0700, got {:o}", ev_mode);

        let runs_dir = ev_dir.join("runs");
        let runs_mode = std::fs::metadata(&runs_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            runs_mode, 0o700,
            "evidence/runs/ must be 0700, got {:o}",
            runs_mode
        );
    }

    /// Layout enforcement regression: paths not under evidence/runs/ must be rejected.
    #[tokio::test]
    async fn write_spool_file_rejects_non_canonical_layout_path() {
        let dir = TempDir::new().unwrap();
        // A safe relative path but not under evidence/runs/ — must be rejected.
        let result = write_spool_file(dir.path(), "run-x", "evidence/test.bin", b"data").await;
        assert!(
            result.is_err(),
            "path outside evidence/runs/ must be rejected by layout enforcement (P075-SEC-002)"
        );
        // Flat non-evidence path.
        let result2 =
            write_spool_file(dir.path(), "run-1", "artifacts/run-1/output.bin", b"data").await;
        assert!(
            result2.is_err(),
            "path not under evidence/runs/ must be rejected"
        );
    }

    /// H-001 regression: a leaf symlink at `final_path` inside `canonical_root` must be
    /// rejected without following the link. This is the actual H-001 attack vector:
    /// the intermediate directories are real, but the final filename is a symlink to an
    /// arbitrary path (including paths outside artifact_root).
    #[cfg(unix)]
    #[tokio::test]
    async fn write_spool_file_rejects_leaf_symlink_at_final_path() {
        let artifact_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        // Create the leaf directory tree so only the final file is a symlink.
        let leaf_dir = artifact_dir
            .path()
            .join("evidence/runs/run-leaf/stages/s-1/agents/a-1/transcripts");
        tokio::fs::create_dir_all(&leaf_dir).await.unwrap();

        // Plant a symlink at final_path pointing to an outside file.
        let outside_target = outside_dir.path().join("secret.txt");
        tokio::fs::write(&outside_target, b"sensitive")
            .await
            .unwrap();
        let leaf_symlink = leaf_dir.join("leaf.bin");
        std::os::unix::fs::symlink(&outside_target, &leaf_symlink).unwrap();

        // Attempt to write through the leaf symlink — must be rejected.
        let result = write_spool_file(
            artifact_dir.path(),
            "run-leaf",
            "evidence/runs/run-leaf/stages/s-1/agents/a-1/transcripts/leaf.bin",
            b"attacker-content",
        )
        .await;
        assert!(
            result.is_err(),
            "leaf symlink at final_path must be rejected (H-001): got Ok({:?})",
            result.ok()
        );

        // The outside target must not have been overwritten.
        let on_disk = tokio::fs::read(&outside_target).await.unwrap();
        assert_eq!(
            on_disk, b"sensitive",
            "outside target must not be written through leaf symlink"
        );
    }

    /// SEC-P075-004 regression: an oversized existing file at `final_path` must not
    /// trigger an unbounded `read()` in the no-clobber path. The call must return Err
    /// with a size-cap diagnostic rather than allocating RAM proportional to the file.
    ///
    /// We cannot actually allocate 512 MiB in a unit test, so we lower the cap by
    /// creating a file just larger than a synthetic cap. Instead we verify that the
    /// production cap constant is checked via a smaller stand-in by injecting a mock:
    /// instead, we verify the code path exists by writing a file that exceeds
    /// `VERIFY_SIZE_CAP_BYTES` via a truncate (sparse file on most platforms), which
    /// allows the stat to report large len without actual disk allocation.
    #[cfg(unix)]
    #[tokio::test]
    async fn write_spool_file_rejects_oversized_final_path_without_reading() {
        use std::os::unix::fs::OpenOptionsExt;
        let artifact_dir = TempDir::new().unwrap();

        // Create leaf directory.
        let leaf_dir = artifact_dir
            .path()
            .join("evidence/runs/run-cap/stages/s-1/agents/a-1/transcripts");
        std::fs::create_dir_all(&leaf_dir).unwrap();

        // Create a sparse file whose stat.len() exceeds VERIFY_SIZE_CAP_BYTES.
        let oversized = leaf_dir.join("huge.bin");
        let f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o600)
            .open(&oversized)
            .unwrap();
        // Seek past the cap and write one byte to create a sparse file.
        use std::io::{Seek, SeekFrom, Write};
        let mut f = f;
        f.seek(SeekFrom::Start(VERIFY_SIZE_CAP_BYTES + 1)).unwrap();
        f.write_all(b"\0").unwrap();
        f.flush().unwrap();
        drop(f);

        assert!(
            std::fs::metadata(&oversized).unwrap().len() > VERIFY_SIZE_CAP_BYTES,
            "sparse file must report len > VERIFY_SIZE_CAP_BYTES"
        );

        let result = write_spool_file(
            artifact_dir.path(),
            "run-cap",
            "evidence/runs/run-cap/stages/s-1/agents/a-1/transcripts/huge.bin",
            b"new-content",
        )
        .await;
        assert!(
            result.is_err(),
            "oversized existing final_path must be rejected without reading (SEC-P075-004): got Ok({:?})",
            result.ok()
        );
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("size cap") || err_msg.contains("MiB"),
            "error must mention size cap; got: {err_msg}"
        );
    }

    /// H-002 regression: write_spool_file must reject a path whose run_id segment
    /// differs from the caller-supplied run_id, BEFORE any filesystem work.
    #[tokio::test]
    async fn write_spool_file_rejects_wrong_run_id_before_filesystem_write() {
        let dir = TempDir::new().unwrap();

        // Supply run_id "run-a" but embed run_id "run-b" in the path.
        let result = write_spool_file(
            dir.path(),
            "run-a",
            "evidence/runs/run-b/stages/s-1/agents/a-1/transcripts/secret.bin",
            b"data",
        )
        .await;
        assert!(
            result.is_err(),
            "path for a different run_id must be rejected before filesystem access (H-002)"
        );

        // Nothing must have been created under either run tree.
        assert!(
            !dir.path().join("evidence/runs/run-a").exists(),
            "run-a directory must not be created"
        );
        assert!(
            !dir.path().join("evidence/runs/run-b").exists(),
            "run-b directory must not be created for wrong run_id"
        );
    }
}
