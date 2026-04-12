use async_graphql::*;
use domain::artifact::Artifact;
use db::repos::projections::ArtifactIndexRow;

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlArtifact {
    pub id: ID,
    pub run_id: ID,
    pub stage_id: String,
    pub agent_id: String,
    pub name: String,
    pub contract_id: String,
    pub format: String,
    pub file_path: String,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub created_at: String,
    pub is_pinned: bool,
    pub report_kind: Option<String>,
    pub report_version: Option<i64>,
}

impl From<Artifact> for GqlArtifact {
    fn from(a: Artifact) -> Self {
        GqlArtifact {
            id: ID(a.id.to_string()),
            run_id: ID(a.run_id.to_string()),
            stage_id: a.stage_id,
            agent_id: a.agent_id,
            name: a.name,
            contract_id: a.contract_id,
            format: a.format.to_string(),
            file_path: a.file_path,
            checksum_sha256: a.checksum_sha256,
            size_bytes: a.size_bytes,
            provider: a.provider,
            model: a.model,
            created_at: a.created_at.to_rfc3339(),
            is_pinned: a.is_pinned,
            report_kind: a.report_kind,
            report_version: a.report_version,
        }
    }
}

impl From<ArtifactIndexRow> for GqlArtifact {
    fn from(r: ArtifactIndexRow) -> Self {
        GqlArtifact {
            id: ID(r.id),
            run_id: ID(r.run_id),
            stage_id: r.stage_id,
            agent_id: r.agent_id,
            name: r.name,
            contract_id: r.contract_id,
            format: r.format,
            file_path: r.file_path,
            checksum_sha256: r.checksum_sha256,
            size_bytes: r.size_bytes,
            provider: r.provider,
            model: r.model,
            created_at: r.created_at,
            is_pinned: r.is_pinned,
            report_kind: r.report_kind,
            report_version: r.report_version,
        }
    }
}
