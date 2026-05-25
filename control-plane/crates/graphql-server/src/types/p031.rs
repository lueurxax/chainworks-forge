use async_graphql::*;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FreshnessState", rename_items = "snake_case")]
pub enum GqlFreshnessState {
    Live,
    Refreshing,
    ProjectionLag,
    Stale,
    Unavailable,
    Unauthorized,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "DisabledReasonCode", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlDisabledReasonCode {
    WritePathNotAvailable,
    ManagedOutsideUi,
    AmbiguousApprovalIdentity,
    StaleRead,
    ProjectionLag,
    Unauthorized,
    UnsupportedAction,
    // P081 boundary policy reason codes
    ApprovalNotActionable,
    ObserverScope,
    NonApprovalMutation,
    CapabilityOutOfScope,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "WritePathState", rename_items = "snake_case")]
pub enum GqlWritePathState {
    Available,
    ReadOnlyDiagnostic,
    WritePathNotAvailable,
    ExternalTransportRequired,
    Hidden,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PayloadAvailabilityState", rename_items = "snake_case")]
pub enum GqlPayloadAvailabilityState {
    Available,
    MetadataOnly,
    PayloadDeferred,
    Generating,
    Unavailable,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(
    name = "PayloadUnavailableReasonCode",
    rename_items = "SCREAMING_SNAKE_CASE"
)]
pub enum GqlPayloadUnavailableReasonCode {
    PayloadDeferredByP031,
    Generating,
    NotIndexed,
    NotAuthorized,
    NotAvailable,
    Unknown,
}

pub fn freshness_from_projection_lag(projection_lag: bool) -> GqlFreshnessState {
    if projection_lag {
        GqlFreshnessState::ProjectionLag
    } else {
        GqlFreshnessState::Live
    }
}

pub fn is_report_metadata(format: &str, report_kind: Option<&str>) -> bool {
    format == "report" || report_kind.is_some()
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "MutationConflictResultCode", rename_items = "snake_case")]
pub enum GqlMutationConflictResultCode {
    /// Same-key idempotent replay: original result returned, no re-settlement.
    AlreadyResolved,
    /// Terminal approval retried with a different idempotency key: zero settlement side effects.
    ApprovalNotActionable,
}
