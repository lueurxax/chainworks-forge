//! Read-only host probe. Never starts a service, opens a project or grants access.

use acp::xcode_headless_host::{HeadlessHostInspector, LocalHeadlessHostInspector};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let host = LocalHeadlessHostInspector::new().inspect().await?;
    println!(
        "{}",
        serde_json::json!({
            "schema_version": 1,
            "host_kind": "headless",
            "pid": host.generation.pid,
            "xcode_build": host.generation.xcode_build,
            "service_build": host.generation.build,
            "open_project_count": host.open_projects.len(),
            "generation_digest": domain::xcode_contract::canonical_digest(
                "cw.xcode.binding.v1", &serde_json::to_value(&host.generation)?
            )?,
            "effects_sent": 0
        })
    );
    Ok(())
}
