use serde_json::json;
use sha2::{Digest, Sha256};

pub struct InvocationOwnerKeyInput<'a> {
    pub run_id: &'a str,
    pub agent_id: &'a str,
    pub stage_lineage_id: &'a str,
    pub task_name: &'a str,
    pub owner_execution_lineage_id: &'a str,
}

pub fn invocation_owner_key(input: &InvocationOwnerKeyInput<'_>) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        input.run_id,
        input.agent_id,
        input.stage_lineage_id,
        input.task_name,
        input.owner_execution_lineage_id
    )
}

pub struct BindingFingerprintInput<'a> {
    pub agent_id: &'a str,
    pub provider: &'a str,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub prompt: &'a str,
    pub working_directory: &'a str,
    pub workspace_mode: &'a str,
    pub worktree_write_enabled: bool,
    pub worktree_strategy: Option<&'a str>,
    pub inputs: &'a [String],
    pub outputs: &'a [String],
    pub backend_profile: Option<&'a str>,
    pub permission_profile: Option<&'a str>,
    pub mcp_servers: &'a [String],
    pub skill_snapshot_hash: Option<&'a str>,
    pub skill_ref: Option<&'a str>,
    pub skill_role: Option<&'a str>,
    pub output_contract: Option<&'a str>,
    pub max_turns: Option<i64>,
    pub temperature: Option<f64>,
}

pub fn binding_fingerprint(input: &BindingFingerprintInput<'_>) -> String {
    let mut inputs = input.inputs.to_vec();
    inputs.sort();
    let mut outputs = input.outputs.to_vec();
    outputs.sort();
    let mut mcp_servers = input.mcp_servers.to_vec();
    mcp_servers.sort();

    sha256_hex(&json!({
        "agent_id": input.agent_id,
        "provider": input.provider,
        "model": input.model,
        "effort": input.effort,
        "prompt": input.prompt,
        "working_directory": input.working_directory,
        "workspace_mode": input.workspace_mode,
        "worktree_write_enabled": input.worktree_write_enabled,
        "worktree_strategy": input.worktree_strategy,
        "inputs": inputs,
        "outputs": outputs,
        "backend_profile": input.backend_profile,
        "permission_profile": input.permission_profile,
        "mcp_servers": mcp_servers,
        "skill_snapshot_hash": input.skill_snapshot_hash,
        "skill_ref": input.skill_ref,
        "skill_role": input.skill_role,
        "output_contract": input.output_contract,
        "max_turns": input.max_turns,
        "temperature": input.temperature,
    }))
}

fn sha256_hex(value: &serde_json::Value) -> String {
    let raw = serde_json::to_vec(value).expect("fingerprint payload should serialize");
    let digest = Sha256::digest(raw);
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::{
        binding_fingerprint, invocation_owner_key, BindingFingerprintInput, InvocationOwnerKeyInput,
    };

    #[test]
    fn owner_key_changes_when_task_identity_changes() {
        let a = invocation_owner_key(&InvocationOwnerKeyInput {
            run_id: "run-1",
            agent_id: "agent-a",
            stage_lineage_id: "state_4_proposal_reviewed",
            task_name: "draft_proposal",
            owner_execution_lineage_id: "execution-lineage-1",
        });
        let b = invocation_owner_key(&InvocationOwnerKeyInput {
            run_id: "run-1",
            agent_id: "agent-a",
            stage_lineage_id: "state_4_proposal_reviewed",
            task_name: "review_proposal",
            owner_execution_lineage_id: "execution-lineage-1",
        });
        assert_ne!(a, b);
    }

    #[test]
    fn owner_key_matches_explicit_tuple_contract() {
        let owner_key = invocation_owner_key(&InvocationOwnerKeyInput {
            run_id: "run-1",
            agent_id: "agent-a",
            stage_lineage_id: "state_4_proposal_reviewed",
            task_name: "draft_proposal",
            owner_execution_lineage_id: "execution-lineage-1",
        });

        assert_eq!(
            owner_key,
            "run-1:agent-a:state_4_proposal_reviewed:draft_proposal:execution-lineage-1"
        );
    }

    #[test]
    fn binding_fingerprint_changes_when_prompt_or_io_contract_changes() {
        let outputs_a = vec!["proposal.md".to_string()];
        let outputs_b = vec![
            "proposal.md".to_string(),
            "proposal_review_summary.json".to_string(),
        ];
        let inputs = vec!["idea.md".to_string()];
        let mcp = vec!["filesystem".to_string()];

        let a = binding_fingerprint(&BindingFingerprintInput {
            agent_id: "proposal_writer",
            provider: "claude",
            model: Some("sonnet"),
            effort: Some("high"),
            prompt: "Draft the proposal carefully.",
            working_directory: "/tmp/ws",
            workspace_mode: "read_only",
            worktree_write_enabled: false,
            worktree_strategy: None,
            inputs: &inputs,
            outputs: &outputs_a,
            backend_profile: Some("claude_orchestrator_high"),
            permission_profile: Some("read_only"),
            mcp_servers: &mcp,
            skill_snapshot_hash: Some("skill-hash-1"),
            skill_ref: Some("proposal_review_triad"),
            skill_role: Some("writer"),
            output_contract: Some("proposal_v1"),
            max_turns: Some(12),
            temperature: Some(0.2),
        });
        let b = binding_fingerprint(&BindingFingerprintInput {
            agent_id: "proposal_writer",
            provider: "claude",
            model: Some("sonnet"),
            effort: Some("high"),
            prompt: "Draft the proposal carefully, then add a review summary.",
            working_directory: "/tmp/ws",
            workspace_mode: "read_only",
            worktree_write_enabled: false,
            worktree_strategy: None,
            inputs: &inputs,
            outputs: &outputs_b,
            backend_profile: Some("claude_orchestrator_high"),
            permission_profile: Some("read_only"),
            mcp_servers: &mcp,
            skill_snapshot_hash: Some("skill-hash-1"),
            skill_ref: Some("proposal_review_triad"),
            skill_role: Some("writer"),
            output_contract: Some("proposal_v1"),
            max_turns: Some(12),
            temperature: Some(0.2),
        });
        assert_ne!(a, b);
    }
}
