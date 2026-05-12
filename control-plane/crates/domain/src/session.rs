use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionGenerationStatus {
    Active,
    Invalidated,
    Closed,
    Reset,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventType {
    Created,
    Reused,
    Invalidated,
    Closed,
    OperatorReset,
    BudgetExceeded,
    Compacted,
    OutputContractRepairStarted,
    OutputContractRepairSucceeded,
    OutputContractRepairFailed,
    OutputContractRepairSkipped,
    CodeWriterCompletionStarted,
    CodeWriterCompletionSucceeded,
    CodeWriterCompletionFailed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionReuseDisposition {
    Fresh,
    Reused,
    ReusedAfterResume,
    FreshAfterReset,
    FreshAfterInvalidation,
    FreshAfterBudget,
    FreshAfterCompaction,
    FreshAfterTransportError,
    FreshAfterTimeout,
    FreshSessionRequired,
    UnverifiableSessionHistory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionLineage {
    pub id: String,
    pub run_id: String,
    pub agent_id: String,
    pub lineage_id: String,
    pub session_reuse_scope: String,
    pub session_family_id: Option<String>,
    pub active_generation_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionGeneration {
    pub id: String,
    pub lineage_id: String,
    pub generation: i64,
    pub invocation_owner_key: String,
    pub provider_session_id: Option<String>,
    pub binding_fingerprint: String,
    pub rehydrated_from_checkpoint_artifact_id: Option<String>,
    pub working_directory: String,
    pub workspace_mode: String,
    pub runtime_provider: String,
    pub runtime_model: String,
    pub status: SessionGenerationStatus,
    pub turn_count: i64,
    pub estimated_input_tokens: i64,
    pub latest_cached_input_tokens: Option<i64>,
    pub latest_output_tokens: Option<i64>,
    pub latest_model_context_window: Option<i64>,
    pub cumulative_prompt_tokens: i64,
    pub cumulative_cost_cents: i64,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: String,
    pub lineage_id: String,
    pub generation_id: String,
    pub event_type: SessionEventType,
    pub recorded_at: DateTime<Utc>,
    pub details_json: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::SessionEventType;

    #[test]
    fn proposal_088_code_writer_completion_event_types_serialize_as_contract_names() {
        let cases = [
            (
                SessionEventType::CodeWriterCompletionStarted,
                "\"code_writer_completion_started\"",
            ),
            (
                SessionEventType::CodeWriterCompletionSucceeded,
                "\"code_writer_completion_succeeded\"",
            ),
            (
                SessionEventType::CodeWriterCompletionFailed,
                "\"code_writer_completion_failed\"",
            ),
        ];

        for (event_type, expected) in cases {
            let json = serde_json::to_string(&event_type).expect("serialize event type");
            assert_eq!(json, expected);
        }
    }
}
