use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const XCODE_RUNTIME_OBSERVATION_MAX_EVENTS: usize = 1000;
pub const XCODE_RUNTIME_OBSERVATION_MAX_BYTES: usize = 1024 * 1024;

fn default_observation_version() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeRuntimeObservation {
    #[serde(default = "default_observation_version")]
    pub version: u32,
    #[serde(default)]
    pub mcp_broker_observations: Vec<McpBrokerObservation>,
    #[serde(default)]
    pub xcode_shim_events: Vec<XcodeShimEvent>,
    #[serde(default)]
    pub xcode_host_executor_events: Vec<XcodeHostExecutorEvent>,
    #[serde(default)]
    pub storage: XcodeRuntimeObservationStorageStatus,
}

impl Default for XcodeRuntimeObservation {
    fn default() -> Self {
        Self {
            version: default_observation_version(),
            mcp_broker_observations: Vec::new(),
            xcode_shim_events: Vec::new(),
            xcode_host_executor_events: Vec::new(),
            storage: XcodeRuntimeObservationStorageStatus::default(),
        }
    }
}

impl XcodeRuntimeObservation {
    pub fn apply_update(&mut self, update: XcodeRuntimeObservationUpdate) {
        match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => {
                self.mcp_broker_observations.push(observation);
            }
            XcodeRuntimeObservationUpdate::XcodeShimEvent(event) => {
                self.xcode_shim_events.push(event);
            }
            XcodeRuntimeObservationUpdate::XcodeHostExecutorEvent(event) => {
                self.xcode_host_executor_events.push(event);
            }
            XcodeRuntimeObservationUpdate::McpBrokerStatusUpdate(update) => {
                self.mcp_broker_observations
                    .push(McpBrokerObservation::status_update(update));
            }
        }
    }

    pub fn record_corrupt_json_recovery(&mut self, quarantined_bytes: usize) {
        self.storage.corrupt_json_recovery_count += 1;
        self.storage.corrupt_json_quarantined_bytes += quarantined_bytes;
    }

    pub fn apply_default_storage_bounds(&mut self) -> serde_json::Result<()> {
        self.apply_storage_bounds(
            XCODE_RUNTIME_OBSERVATION_MAX_EVENTS,
            XCODE_RUNTIME_OBSERVATION_MAX_BYTES,
        )
    }

    pub fn total_event_count(&self) -> usize {
        self.mcp_broker_observations.len()
            + self.xcode_shim_events.len()
            + self.xcode_host_executor_events.len()
    }

    fn apply_storage_bounds(
        &mut self,
        max_events: usize,
        max_bytes: usize,
    ) -> serde_json::Result<()> {
        self.storage.max_events = max_events;
        self.storage.max_bytes = max_bytes;

        while self.total_event_count() > max_events {
            if !self.drop_oldest_event() {
                break;
            }
        }

        while serde_json::to_string(self)?.len() > max_bytes {
            if !self.drop_oldest_event() {
                break;
            }
        }

        Ok(())
    }

    fn drop_oldest_event(&mut self) -> bool {
        if !self.mcp_broker_observations.is_empty() {
            self.mcp_broker_observations.remove(0);
            self.storage.mcp_broker_observations_dropped += 1;
        } else if !self.xcode_shim_events.is_empty() {
            self.xcode_shim_events.remove(0);
            self.storage.xcode_shim_events_dropped += 1;
        } else if !self.xcode_host_executor_events.is_empty() {
            self.xcode_host_executor_events.remove(0);
            self.storage.xcode_host_executor_events_dropped += 1;
        } else {
            return false;
        }

        self.storage.truncated = true;
        self.storage.total_events_dropped += 1;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeRuntimeObservationStorageStatus {
    pub max_events: usize,
    pub max_bytes: usize,
    pub truncated: bool,
    pub total_events_dropped: usize,
    pub mcp_broker_observations_dropped: usize,
    pub xcode_shim_events_dropped: usize,
    pub xcode_host_executor_events_dropped: usize,
    pub corrupt_json_recovery_count: usize,
    pub corrupt_json_quarantined_bytes: usize,
}

impl Default for XcodeRuntimeObservationStorageStatus {
    fn default() -> Self {
        Self {
            max_events: XCODE_RUNTIME_OBSERVATION_MAX_EVENTS,
            max_bytes: XCODE_RUNTIME_OBSERVATION_MAX_BYTES,
            truncated: false,
            total_events_dropped: 0,
            mcp_broker_observations_dropped: 0,
            xcode_shim_events_dropped: 0,
            xcode_host_executor_events_dropped: 0,
            corrupt_json_recovery_count: 0,
            corrupt_json_quarantined_bytes: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeRuntimeFailureClass {
    ProviderHttpMcpUnsupported,
    XcodeMcpRegistryStaleStdio,
    XcodeMcpRegistryAmbiguous,
    HostEnvUnavailable,
    PoolPidDrift,
    XcodeMcpCapacityExhausted,
    XcodeMcpInitializeTimeout,
    XcodeMcpActionRequired,
    XcodeMcpFirstConnectTimeout,
    XcodeShimNoActivePrompt,
    SimulatorDestinationAmbiguous,
    XcodeBuildConcurrencyContention,
    XcodeTargetNotFound,
    XcodeTargetAmbiguous,
    PerLeaseBackend,
    BrokerInfrastructure,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeSimulatorSelection {
    pub mode: String,
    pub simulator_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpBrokerObservation {
    pub source: String,
    pub backend_start_disposition: String,
    pub pool_id: Option<String>,
    pub lease_id: Option<String>,
    pub xcode_pid: Option<String>,
    pub backend_process_id: Option<i64>,
    pub http_endpoint: Option<String>,
    pub xcode_home_disposition: Option<String>,
    pub xcode_tmpdir_disposition: Option<String>,
    pub simulator_selection: Option<XcodeSimulatorSelection>,
    pub sibling_leases_at_spawn: Option<i64>,
    pub backend_initialize_wait_ms: Option<i64>,
    pub backend_startup_latency_ms: Option<i64>,
    pub http_session_startup_latency_ms: Option<i64>,
    pub backend_failure_class: Option<XcodeRuntimeFailureClass>,
    pub originating_execution_id: Option<String>,
    pub prompt_cycle_index: Option<i64>,
    pub status_update: Option<String>,
}

impl McpBrokerObservation {
    pub fn status_update(update: McpBrokerStatusUpdate) -> Self {
        Self {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: "status_update".to_string(),
            pool_id: None,
            lease_id: Some(update.lease_id),
            xcode_pid: None,
            backend_process_id: None,
            http_endpoint: None,
            xcode_home_disposition: None,
            xcode_tmpdir_disposition: None,
            simulator_selection: None,
            sibling_leases_at_spawn: None,
            backend_initialize_wait_ms: None,
            backend_startup_latency_ms: None,
            http_session_startup_latency_ms: None,
            backend_failure_class: Some(update.backend_failure_class),
            originating_execution_id: None,
            prompt_cycle_index: None,
            status_update: Some(update.status_update),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpBrokerStatusUpdate {
    pub lease_id: String,
    pub backend_failure_class: XcodeRuntimeFailureClass,
    pub status_update: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum XcodeShimEvent {
    ShimInvocation(XcodeShimInvocationEvent),
    Warning(XcodeShimWarningEvent),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeShimInvocationEvent {
    pub ts: DateTime<Utc>,
    pub tool: String,
    pub via_xcrun: bool,
    #[serde(default)]
    pub argv: Vec<String>,
    pub cwd: String,
    pub policy_decision: String,
    pub policy_reason: String,
    pub derived_peer_pid: i64,
    pub derived_peer_uid: i64,
    pub claimed_provider_pid: i64,
    pub peer_pid_mismatch: bool,
    pub exit_status: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeShimWarningEvent {
    pub ts: DateTime<Utc>,
    pub policy_reason: String,
    pub source_field: String,
    pub matched_substring: String,
    pub excerpt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeHostExecutorEvent {
    pub ts: DateTime<Utc>,
    pub tool: String,
    #[serde(default)]
    pub argv: Vec<String>,
    pub cwd: String,
    pub host_env_disposition: String,
    #[serde(default)]
    pub env_allowlist_applied: Vec<String>,
    #[serde(default)]
    pub env_dropped_from_provider: Vec<String>,
    pub selected_simulator_id: Option<String>,
    pub exit_status: i64,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum XcodeRuntimeObservationUpdate {
    McpBrokerObservation(McpBrokerObservation),
    XcodeShimEvent(XcodeShimEvent),
    XcodeHostExecutorEvent(XcodeHostExecutorEvent),
    McpBrokerStatusUpdate(McpBrokerStatusUpdate),
}
