use std::path::Path;

use anyhow::Result;
use sqlx::SqlitePool;

pub use engine::steward::config::{
    load_effective_config, StewardConfig, StewardConfigLoadStatus, StewardRuntimeInputs,
};

pub async fn bootstrap_steward_runtime(
    pool: &SqlitePool,
    config_path: Option<&Path>,
    catalog_path: Option<&Path>,
) -> Result<StewardRuntimeInputs> {
    let effective = engine::steward::config::load_effective_config(config_path);
    let catalog_path = catalog_path
        .map(|path| path.to_path_buf())
        .unwrap_or_else(engine::steward::config::default_agent_catalog_path);
    let (agent_catalog_json, agent_catalog_hash) =
        engine::steward::config::load_agent_catalog_json(&catalog_path)?;

    let previous_config_hash =
        db::repos::steward::get_runtime_state(pool, "steward_config_hash").await?;
    let previous_catalog_hash =
        db::repos::steward::get_runtime_state(pool, "steward_catalog_hash").await?;
    let config_changed = previous_config_hash
        .as_ref()
        .is_some_and(|previous| previous != &effective.hash);
    let catalog_changed = previous_catalog_hash
        .as_ref()
        .is_some_and(|previous| previous != &agent_catalog_hash);
    let pending =
        effective.config.triggers.on_config_change.enabled && (config_changed || catalog_changed);
    if pending {
        db::repos::steward::mark_config_change_pending(
            pool,
            Some(&effective.hash),
            Some(&agent_catalog_hash),
        )
        .await?;
    }
    db::repos::steward::set_runtime_state(pool, "steward_config_hash", &effective.hash).await?;
    db::repos::steward::set_runtime_state(pool, "steward_catalog_hash", &agent_catalog_hash)
        .await?;
    db::repos::steward::set_post_run_trigger_config(
        pool,
        effective.config.triggers.post_run_hook.enabled,
        effective.config.triggers.post_run_hook.run_interval,
    )
    .await?;

    Ok(StewardRuntimeInputs {
        steward_config_path: config_path
            .map(|path| path.to_path_buf())
            .unwrap_or_else(engine::steward::config::default_config_path),
        steward_config: effective.config,
        steward_config_hash: effective.hash,
        steward_config_load_status: effective.load_status,
        agent_catalog_path: catalog_path,
        agent_catalog_json,
        agent_catalog_hash,
        previous_steward_config_hash: previous_config_hash,
        previous_agent_catalog_hash: previous_catalog_hash,
        config_change_analysis_scheduled: pending,
    })
}
