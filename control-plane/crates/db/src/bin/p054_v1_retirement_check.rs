use anyhow::{Context, Result};
use serde_json::json;

use db::pool::create_pool;
use db::repos::artifact_contracts;

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL is required for the P054 v1 fallback retirement check")?;
    let pool = create_pool(&database_url).await?;
    let check = artifact_contracts::v1_fallback_retirement_check(&pool).await?;
    let active_run_ids: Vec<String> = check
        .active_non_terminal_v1_only_run_ids
        .iter()
        .map(ToString::to_string)
        .collect();

    let report = json!({
        "gate": "p054_v1_fallback_retirement",
        "safe_to_retire": check.safe_to_retire(),
        "active_non_terminal_v1_only_run_count": check.active_non_terminal_v1_only_run_count(),
        "active_non_terminal_v1_only_run_ids": active_run_ids
    });

    println!("{}", serde_json::to_string_pretty(&report)?);

    if !check.safe_to_retire() {
        std::process::exit(1);
    }

    Ok(())
}
