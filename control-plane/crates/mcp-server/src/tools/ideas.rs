use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

use db::repos::ideas;
use domain::commands::{Command, CreateIdeaCmd};
use engine::command_handler::{CommandHandler, CommandResult};

use crate::protocol::McpTool;
use crate::request_context;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "ideas.list".to_string(),
            description: "List all ideas, optionally including archived ones".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_archived": {
                        "type": "boolean",
                        "description": "Whether to include archived ideas (default: false)"
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "ideas.create".to_string(),
            description: "Create a new idea".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title", "body", "idempotency_key"],
                "properties": {
                    "title": { "type": "string", "description": "Idea title" },
                    "body": { "type": "string", "description": "Idea body / description" },
                    "workspace_root_path": { "type": "string", "description": "Optional workspace root path" },
                    "project_key": { "type": "string", "description": "Optional stable project cohort key" },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." }
                }
            }),
            output_schema: None,
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "ideas.list" => {
            let include_archived = params["include_archived"].as_bool().unwrap_or(false);
            let items = ideas::list(pool, include_archived).await?;
            Ok(serde_json::to_value(items)?)
        }
        "ideas.create" => {
            let title = params["title"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'title'"))?
                .to_string();
            let body = params["body"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'body'"))?
                .to_string();
            let workspace_root_path = params["workspace_root_path"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            authorize_idea_workspace_root_mutation(principal, workspace_root_path)?;
            let workspace_root_path = workspace_root_path
                .map(canonicalize_idea_workspace_root_path)
                .transpose()?;
            let project_key = params["project_key"].as_str().map(|s| s.to_string());

            let commanded = cmd_handler
                .handle(
                    Command::CreateIdea(CreateIdeaCmd {
                        title,
                        body,
                        workspace_root_path,
                        project_key,
                    }),
                    request_context::mcp_caller(principal, "ideas.create"),
                )
                .await?;
            match commanded.result {
                CommandResult::IdeaCreated { idea } => Ok(serde_json::json!({
                    "idea": idea,
                    "journal_id": commanded.journal_id,
                })),
                _ => Err(anyhow::anyhow!(
                    "ideas.create returned unexpected command result"
                )),
            }
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn authorize_idea_workspace_root_mutation(
    principal: &auth::Principal,
    workspace_root_path: Option<&str>,
) -> Result<()> {
    if workspace_root_path.is_some() && principal.class != auth::PrincipalClass::Operator {
        anyhow::bail!(
            "ideas.create: workspace_root_path establishes filesystem authority and requires Operator principal"
        );
    }
    Ok(())
}

fn canonicalize_idea_workspace_root_path(raw: &str) -> Result<String> {
    validate_idea_workspace_root_path(raw)?;
    let path = Path::new(raw);
    if !path.exists() {
        anyhow::bail!("ideas.create: workspace_root_path does not exist");
    }
    if !path.is_dir() {
        anyhow::bail!("ideas.create: workspace_root_path must be a directory");
    }
    reject_symlink_components(path, "workspace_root_path")?;
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("ideas.create: canonicalize workspace_root_path '{raw}'"))?;
    reject_broad_idea_workspace_root(&canonical)?;
    Ok(canonical.to_string_lossy().to_string())
}

fn validate_idea_workspace_root_path(field_value: &str) -> Result<()> {
    if field_value.contains('\0') {
        anyhow::bail!("ideas.create: workspace_root_path contains a null byte");
    }
    if field_value.contains('\\') {
        anyhow::bail!("ideas.create: workspace_root_path contains a backslash separator");
    }
    if field_value.contains("://") {
        anyhow::bail!("ideas.create: workspace_root_path contains a URI scheme separator");
    }
    for component in field_value.split('/') {
        if component == ".." {
            anyhow::bail!("ideas.create: workspace_root_path contains '..'");
        }
    }
    Ok(())
}

fn reject_broad_idea_workspace_root(canonical: &Path) -> Result<()> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|home| std::fs::canonicalize(home).ok());
    let broad_literals = [
        Path::new("/"),
        Path::new("/Applications"),
        Path::new("/Library"),
        Path::new("/System"),
        Path::new("/Volumes"),
        Path::new("/etc"),
        Path::new("/private"),
        Path::new("/private/etc"),
        Path::new("/tmp"),
        Path::new("/private/tmp"),
        Path::new("/var"),
        Path::new("/private/var"),
        Path::new("/Users"),
        Path::new("/home"),
    ];
    if broad_literals.iter().any(|broad| canonical == *broad)
        || canonical
            .parent()
            .is_some_and(|parent| parent == Path::new("/Volumes"))
        || home.as_deref().is_some_and(|home| canonical == home)
    {
        anyhow::bail!("ideas.create: workspace_root_path is too broad to use as a trusted filesystem boundary");
    }
    Ok(())
}

fn reject_symlink_components(path: &Path, field: &str) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            anyhow::bail!("ideas.create: {field} contains a symlink component");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p082_ideas_create_rejects_broad_workspace_root_policy_boundary() {
        let err = canonicalize_idea_workspace_root_path("/").unwrap_err();
        assert!(
            err.to_string().contains("too broad"),
            "broad idea workspace roots must fail closed; got: {err}"
        );
    }

    #[test]
    fn sec_med_001_ideas_create_rejects_macos_system_workspace_roots() {
        for root in [
            "/private",
            "/private/etc",
            "/Library",
            "/System",
            "/Volumes",
            "/Volumes/External",
            "/Applications",
        ] {
            let err = reject_broad_idea_workspace_root(Path::new(root)).unwrap_err();
            assert!(
                err.to_string().contains("too broad"),
                "ideas.create must reject broad root {root}; got: {err}"
            );
        }
    }

    #[test]
    fn p082_ideas_create_rejects_workspace_root_symlink() {
        let parent = tempfile::tempdir().expect("parent");
        let real = parent.path().join("real");
        std::fs::create_dir(&real).expect("real dir");
        let link = parent.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).expect("workspace root symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real, &link).expect("workspace root symlink");

        let err =
            canonicalize_idea_workspace_root_path(link.to_string_lossy().as_ref()).unwrap_err();
        assert!(
            err.to_string().contains("symlink"),
            "symlinked idea workspace root must fail closed; got: {err}"
        );
    }

    #[test]
    fn p082_agent_cannot_create_idea_with_workspace_root_path() {
        let principal = auth::Principal::new("agent-p082", auth::PrincipalClass::Agent);
        let workspace = tempfile::tempdir().expect("workspace");

        let err = authorize_idea_workspace_root_mutation(
            &principal,
            Some(workspace.path().to_string_lossy().as_ref()),
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("Operator"),
            "Agent-authored workspace roots must fail closed; got: {err}"
        );
    }
}
