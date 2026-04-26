use async_graphql::*;
use domain::idea::Idea;

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlIdea {
    pub id: ID,
    pub title: String,
    pub body: String,
    pub workspace_root_path: Option<String>,
    pub project_key: Option<String>,
    pub status: String,
    pub created_at: String,
    pub archived_at: Option<String>,
}

impl From<Idea> for GqlIdea {
    fn from(idea: Idea) -> Self {
        GqlIdea {
            id: ID(idea.id.to_string()),
            title: idea.title,
            body: idea.body,
            workspace_root_path: idea.workspace_root_path,
            project_key: idea.project_key,
            status: idea.status.to_string(),
            created_at: idea.created_at.to_rfc3339(),
            archived_at: idea.archived_at.map(|t| t.to_rfc3339()),
        }
    }
}
