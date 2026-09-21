use super::budget::Deadline;
use anyhow::{ensure, Context, Result};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};

pub(super) const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;

pub(super) async fn run(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    deadline: Deadline,
) -> Result<Vec<u8>> {
    let time_limit = deadline.remaining()?.min(Duration::from_secs(30));
    let mut child = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("LC_ALL", "C")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.excludesFile=/dev/null",
            // Porcelain diff can refresh a clean index even with optional locks disabled.
            "-c",
            "diff.autoRefreshIndex=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=never",
            "-c",
            "fetch.recurseSubmodules=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
        ])
        .args(args)
        .current_dir(root)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("carry_forward_git_spawn")?;
    let mut stdin = child.stdin.take().context("git stdin")?;
    let stdout = child.stdout.take().context("git stdout")?;
    let stderr = child.stderr.take().context("git stderr")?;
    let work = async {
        let write = async move {
            if let Some(bytes) = input {
                stdin.write_all(bytes).await?;
            }
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let read_out = async {
            let mut bytes = Vec::new();
            stdout
                .take(OUTPUT_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)
                .await?;
            Ok::<_, std::io::Error>(bytes)
        };
        let read_err = async {
            let mut bytes = Vec::new();
            stderr.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
            Ok::<_, std::io::Error>(bytes)
        };
        tokio::try_join!(write, read_out, read_err, child.wait())
    };
    let result = timeout(time_limit, work).await;
    let (_, stdout, stderr, status) = match result {
        Ok(Ok(value)) => value,
        _ => {
            let _ = child.kill().await;
            anyhow::bail!(
                "effect_outcome_unknown: bounded git {} did not settle normally",
                args.first().unwrap_or(&"unknown")
            );
        }
    };
    ensure!(
        stdout.len() <= OUTPUT_LIMIT && stderr.len() <= 64 * 1024,
        "continuation_budget_exceeded: git output"
    );
    ensure!(
        status.success(),
        "carry_forward_git_failed: {}",
        args.first().unwrap_or(&"unknown")
    );
    deadline.check()?;
    Ok(stdout)
}

pub(super) async fn text(root: &Path, args: &[&str], deadline: Deadline) -> Result<String> {
    Ok(String::from_utf8(run(root, args, None, deadline).await?)
        .context("unsafe_path: non-UTF8 Git data")?
        .trim_end_matches('\n')
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_carry_forward::{materialize, PreviewOptions, WorkspacePlanner};
    use std::fs;

    #[tokio::test]
    async fn read_commands_preserve_first_clean_materialized_index() {
        let base = tempfile::tempdir().unwrap();
        let source = base.path().join("source");
        fs::create_dir(&source).unwrap();
        let deadline = Deadline::after(Duration::from_secs(120));
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "P039 Test"],
            vec!["config", "user.email", "p039@example.invalid"],
        ] {
            run(&source, &args, None, deadline).await.unwrap();
        }
        fs::write(source.join("code.txt"), b"original\n").unwrap();
        run(&source, &["add", "."], None, deadline).await.unwrap();
        run(
            &source,
            &["-c", "commit.gpgsign=false", "commit", "-m", "fixture"],
            None,
            deadline,
        )
        .await
        .unwrap();
        let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
            .await
            .unwrap();
        let receipt = materialize(&preview, base.path(), uuid::Uuid::new_v4())
            .await
            .unwrap();
        let checkout = &receipt.checkout_root;
        let index = text(
            checkout,
            &["rev-parse", "--path-format=absolute", "--git-path", "index"],
            deadline,
        )
        .await
        .unwrap();
        let before = fs::read(&index).unwrap();
        for args in [
            vec!["config", "--null", "--list"],
            vec!["rev-parse", "--show-toplevel"],
            vec!["rev-parse", "--absolute-git-dir"],
            vec!["rev-parse", "--path-format=absolute", "--git-common-dir"],
            vec!["rev-parse", "--verify", "HEAD^{commit}"],
            vec!["rev-parse", "--shared-index-path"],
            vec!["ls-files", "-v", "-z"],
            vec!["ls-files", "--stage", "-z"],
            vec!["ls-tree", "-r", "-z", "HEAD"],
            vec!["ls-files", "--others", "--exclude-standard", "-z"],
            vec![
                "check-attr",
                "-z",
                "filter",
                "working-tree-encoding",
                "--",
                "code.txt",
            ],
            vec![
                "diff",
                "--cached",
                "--binary",
                "--no-ext-diff",
                "--no-textconv",
                "HEAD",
                "--",
            ],
            vec!["diff", "--binary", "--no-ext-diff", "--no-textconv", "--"],
        ] {
            run(checkout, &args, None, deadline).await.unwrap();
            assert_eq!(
                fs::read(&index).unwrap(),
                before,
                "index changed by {args:?}"
            );
        }
    }
}
