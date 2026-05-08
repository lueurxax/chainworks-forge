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
//! # Temp-path collision safety
//!
//! The temp filename is `{filename}.tmp.{uuid4}`. UUID v4 makes collisions
//! negligible. If the final path already exists (idempotent retry), the atomic
//! rename overwrites it — the caller must re-verify the checksum via
//! `insert_idempotent` or treat the overwrite as a producer error.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::Digest;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::repos::evidence_spool_refs::{redact_path, validate_relative_path};

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

/// Maximum file size that [`verify_spool_file`] will read into memory (SEC-P075-004).
/// Files larger than this cap return `Err` rather than allocating unbounded RAM.
const VERIFY_SIZE_CAP_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB

// ─── Public API ──────────────────────────────────────────────────────────────

/// Write evidence content to a spool file following the P075 ordering contract.
///
/// # Ordering
///
/// ```text
/// write(temp) → sha256 → fsync(temp) → rename(temp→final) → fsync(parent_dir)
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
    let final_path = canonical_parent
        .join(absolute_path.file_name().expect("validated above: file_name is Some"));

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

// ─── Internal helpers ─────────────────────────────────────────────────────────

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
    // • Same checksum + same size → idempotent retry; skip rename, clean up temp.
    // • Content differs → hard error; the caller must reconcile via storage.reconcile_evidence_orphans.
    if let Ok(existing_bytes) = tokio::fs::read(final_path).await {
        let existing_checksum = sha256_hex(&existing_bytes);
        // Clean up temp file before returning either way.
        let _ = tokio::fs::remove_file(temp_path).await;
        if existing_checksum == checksum && existing_bytes.len() as u64 == content.len() as u64 {
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
        write_spool_file(dir.path(), spool_path, b"data")
            .await
            .unwrap();
        // No .tmp.* files should remain after success.
        let mut tmp_count = 0usize;
        let leaf_dir = dir.path().join("evidence/runs/run-test/stages/s-1/agents/a-1/transcripts");
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
            "evidence/runs/run-t/stages/s-1/agents/a-1/stdout/tamper.bin",
            b"original",
        )
        .await
        .unwrap();
        // Overwrite with different content.
        tokio::fs::write(&output.absolute_path, b"tampered").await.unwrap();
        let result = verify_spool_file(&output.absolute_path, &output.checksum)
            .await
            .unwrap();
        assert!(matches!(result, VerifyResult::ChecksumMismatch { .. }));
    }

    #[tokio::test]
    async fn write_spool_file_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let result = write_spool_file(dir.path(), "evidence/../secret.bin", b"data").await;
        assert!(result.is_err(), "path traversal must be rejected");
    }

    #[tokio::test]
    async fn write_spool_file_rejects_absolute_path() {
        let dir = TempDir::new().unwrap();
        let result = write_spool_file(dir.path(), "/etc/passwd", b"data").await;
        assert!(result.is_err(), "absolute path must be rejected");
    }

    #[tokio::test]
    async fn write_spool_file_creates_nested_directories() {
        let dir = TempDir::new().unwrap();
        let path = "evidence/runs/run-001/stages/s-01/agents/a-01/transcripts/chunk.bin";
        let output = write_spool_file(dir.path(), path, b"nested")
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
        assert_eq!(runs_mode, 0o700, "evidence/runs/ must be 0700, got {:o}", runs_mode);
    }

    /// Layout enforcement regression: paths not under evidence/runs/ must be rejected.
    #[tokio::test]
    async fn write_spool_file_rejects_non_canonical_layout_path() {
        let dir = TempDir::new().unwrap();
        // A safe relative path but not under evidence/runs/ — must be rejected.
        let result = write_spool_file(dir.path(), "evidence/test.bin", b"data").await;
        assert!(
            result.is_err(),
            "path outside evidence/runs/ must be rejected by layout enforcement (P075-SEC-002)"
        );
        // Flat non-evidence path.
        let result2 = write_spool_file(dir.path(), "artifacts/run-1/output.bin", b"data").await;
        assert!(
            result2.is_err(),
            "path not under evidence/runs/ must be rejected"
        );
    }
}
