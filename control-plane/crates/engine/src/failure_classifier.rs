use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use domain::agent::{AgentFailureKind, OperatorActionHint};

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeFailureObservation {
    ProviderQuota {
        retry_after: Option<DateTime<Utc>>,
    },
    ProviderPermissionRequired,
    ProviderPermissionRejected,
    ProviderTimeout {
        supervision_classification: Option<String>,
    },
    ProviderInternalError,
    TransportEpipe,
    TransportProtocolError,
    TransportClosed,
    McpStartupTimeout,
    McpPermissionModalStall,
    XcodeHostEnvironmentError,
    MissingRequiredOutputs,
    InvalidOutputContract,
    CancelledByOperator,
    SupersededByRetry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFailureClassification {
    pub failure_kind: AgentFailureKind,
    pub operator_action_hint: OperatorActionHint,
    pub retry_after: Option<DateTime<Utc>>,
    pub transport_error_code: Option<String>,
    pub supervision_classification: Option<String>,
}

pub fn classify_observation(
    observation: RuntimeFailureObservation,
) -> RuntimeFailureClassification {
    use RuntimeFailureObservation::*;
    match observation {
        ProviderQuota { retry_after } => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::ProviderQuota,
            operator_action_hint: OperatorActionHint::WaitUntilRetryAfter,
            retry_after,
            transport_error_code: None,
            supervision_classification: None,
        },
        ProviderPermissionRequired => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::ProviderPermissionRequired,
            operator_action_hint: OperatorActionHint::AuthorizeXcode,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        ProviderPermissionRejected => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::ProviderPermissionRejected,
            operator_action_hint: OperatorActionHint::InspectLogs,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        ProviderTimeout {
            supervision_classification,
        } => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::ProviderTimeout,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification,
        },
        ProviderInternalError => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::ProviderInternalError,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        TransportEpipe => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::TransportEpipe,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: Some("EPIPE".into()),
            supervision_classification: None,
        },
        TransportProtocolError => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::TransportProtocolError,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        TransportClosed => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::TransportClosed,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        McpStartupTimeout => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::McpStartupTimeout,
            operator_action_hint: OperatorActionHint::InspectLogs,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        McpPermissionModalStall => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::McpPermissionModalStall,
            operator_action_hint: OperatorActionHint::AuthorizeXcode,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        XcodeHostEnvironmentError => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::XcodeHostEnvironmentError,
            operator_action_hint: OperatorActionHint::InspectLogs,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        MissingRequiredOutputs => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::MissingRequiredOutputs,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        InvalidOutputContract => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::InvalidOutputContract,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        CancelledByOperator => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::CancelledByOperator,
            operator_action_hint: OperatorActionHint::InspectLogs,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
        SupersededByRetry => RuntimeFailureClassification {
            failure_kind: AgentFailureKind::SupersededByRetry,
            operator_action_hint: OperatorActionHint::Retry,
            retry_after: None,
            transport_error_code: None,
            supervision_classification: None,
        },
    }
}

pub fn observation_from_acp_error_message(message: &str) -> RuntimeFailureObservation {
    if let Some(observation) = observation_from_typed_payload(message) {
        return observation;
    }

    let lower = message.to_ascii_lowercase();
    if lower.contains("quota") || lower.contains("limit") || lower.contains("retry_after") {
        return RuntimeFailureObservation::ProviderQuota {
            retry_after: extract_retry_after_from_message(message),
        };
    }
    if lower.contains("epipe") || lower.contains("broken pipe") {
        return RuntimeFailureObservation::TransportEpipe;
    }
    if lower.contains("idle") && lower.contains("timeout") {
        return RuntimeFailureObservation::ProviderTimeout {
            supervision_classification: Some("idle_hang_before_first_progress".into()),
        };
    }
    if lower.contains("xcode") && lower.contains("host") && lower.contains("environment") {
        return RuntimeFailureObservation::XcodeHostEnvironmentError;
    }
    if lower.contains("xcode") && (lower.contains("permission") || lower.contains("modal")) {
        return RuntimeFailureObservation::McpPermissionModalStall;
    }
    if lower.contains("xcode_mcp_initialize_timeout")
        || lower.contains("xcode_mcp_warmup_failed")
        || (lower.contains("xcode") && lower.contains("mcp") && lower.contains("timeout"))
    {
        return RuntimeFailureObservation::McpPermissionModalStall;
    }
    if lower.contains("permission") && lower.contains("required") {
        return RuntimeFailureObservation::ProviderPermissionRequired;
    }
    if lower.contains("permission") && lower.contains("reject") {
        return RuntimeFailureObservation::ProviderPermissionRejected;
    }
    if lower.contains("mcp") && lower.contains("startup") && lower.contains("timeout") {
        return RuntimeFailureObservation::McpStartupTimeout;
    }
    if lower.contains("protocol") || lower.contains("json-rpc") {
        return RuntimeFailureObservation::TransportProtocolError;
    }
    if lower.contains("stdout closed")
        || lower.contains("transport closed")
        || lower.contains("session closed during active prompt")
        || lower.contains("acp: send session/prompt")
        || lower.contains("write acp message to subprocess stdin")
    {
        return RuntimeFailureObservation::TransportClosed;
    }
    RuntimeFailureObservation::ProviderInternalError
}

fn observation_from_typed_payload(message: &str) -> Option<RuntimeFailureObservation> {
    let trimmed = message.trim();
    let json_slice = if trimmed.starts_with('{') {
        trimmed
    } else {
        let start = trimmed.find('{')?;
        let end = trimmed.rfind('}')?;
        if end <= start {
            return None;
        }
        &trimmed[start..=end]
    };
    let value = serde_json::from_str::<serde_json::Value>(json_slice).ok()?;
    let observation_kind = value.get("observation_kind")?.as_str()?;
    let failure_scope = value.get("failure_scope").and_then(|value| value.as_str());
    let server_id = value.get("server_id").and_then(|value| value.as_str());
    let broker_used = value
        .get("broker_used")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let host_env_error_kind = value
        .get("host_env_error_kind")
        .and_then(|value| value.as_str());
    match observation_kind {
        "xcode_host_env_unavailable" => Some(RuntimeFailureObservation::XcodeHostEnvironmentError),
        "backend_start_failed"
            if failure_scope == Some("host_environment") || host_env_error_kind.is_some() =>
        {
            Some(RuntimeFailureObservation::XcodeHostEnvironmentError)
        }
        "backend_start_failed"
            if failure_scope == Some("startup") && (server_id == Some("xcode") || broker_used) =>
        {
            Some(RuntimeFailureObservation::McpPermissionModalStall)
        }
        "backend_start_failed" if failure_scope == Some("startup") => {
            Some(RuntimeFailureObservation::McpStartupTimeout)
        }
        kind if kind.contains("permission_modal")
            || kind.contains("modal_stall")
            || kind == "tools_list_waited_on_modal" =>
        {
            Some(RuntimeFailureObservation::McpPermissionModalStall)
        }
        _ => None,
    }
}

fn extract_retry_after_from_message(message: &str) -> Option<DateTime<Utc>> {
    extract_retry_after_from_message_at(message, Utc::now())
}

fn extract_retry_after_from_message_at(message: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    for marker in ["retry_after=", "retry-after=", "retry_after:"] {
        if let Some((_, tail)) = message.split_once(marker) {
            let raw = tail
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|c| matches!(c, '"' | '\'' | ',' | ';'));
            if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
                return Some(dt.with_timezone(&Utc));
            }
        }
    }
    if let Some(retry_after) = extract_relative_reset_retry_after(message, now) {
        return Some(retry_after);
    }
    extract_reset_clock_retry_after(message, now)
}

fn extract_relative_reset_retry_after(message: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let lower = message.to_ascii_lowercase();
    let (_, tail) = lower
        .split_once("reset after ")
        .or_else(|| lower.split_once("resets after "))
        .or_else(|| lower.split_once("retry after "))?;
    let token = tail
        .split_whitespace()
        .next()?
        .trim_matches(|c| matches!(c, '"' | '\'' | ',' | ';' | '.'));
    let duration = parse_compact_duration(token)?;
    Some(now + duration)
}

fn parse_compact_duration(token: &str) -> Option<Duration> {
    let mut total = Duration::zero();
    let mut digits = String::new();
    let mut saw_unit = false;
    for ch in token.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        let value: i64 = digits.parse().ok()?;
        digits.clear();
        match ch {
            'd' => total = total + Duration::days(value),
            'h' => total = total + Duration::hours(value),
            'm' => total = total + Duration::minutes(value),
            's' => total = total + Duration::seconds(value),
            _ => return None,
        }
        saw_unit = true;
    }
    if !digits.is_empty() || !saw_unit || total <= Duration::zero() {
        return None;
    }
    Some(total)
}

fn extract_reset_clock_retry_after(message: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let lower = message.to_ascii_lowercase();
    let (start, marker_len) = ["resets at ", "reset at ", "resets "]
        .iter()
        .find_map(|marker| lower.find(marker).map(|index| (index, marker.len())))?;
    let tail = message[start + marker_len..].trim_start();
    let time_token = tail.split_whitespace().next()?;
    let reset_time = parse_reset_time_token(time_token)?;
    let tail_after_time = tail[time_token.len()..].trim_start();
    let timezone = tail_after_time
        .strip_prefix('(')
        .and_then(|value| value.split_once(')'))
        .map(|(timezone, _)| timezone.trim())?;
    retry_after_for_local_reset(now, reset_time, timezone)
}

fn parse_reset_time_token(token: &str) -> Option<NaiveTime> {
    let cleaned = token.trim_matches(|c| matches!(c, '"' | '\'' | ',' | ';' | '.'));
    let lower = cleaned.to_ascii_lowercase();
    let (raw_time, suffix) = if let Some(raw) = lower.strip_suffix("am") {
        (raw, Some("am"))
    } else if let Some(raw) = lower.strip_suffix("pm") {
        (raw, Some("pm"))
    } else {
        (lower.as_str(), None)
    };
    let mut parts = raw_time.split(':');
    let mut hour: u32 = parts.next()?.parse().ok()?;
    let minute: u32 = parts
        .next()
        .map(|part| part.parse().ok())
        .unwrap_or(Some(0))?;
    if parts.next().is_some() {
        return None;
    }
    match suffix {
        Some("am") if hour == 12 => hour = 0,
        Some("am") => {}
        Some("pm") if hour < 12 => hour += 12,
        Some("pm") => {}
        _ => {}
    }
    NaiveTime::from_hms_opt(hour, minute, 0)
}

fn retry_after_for_local_reset(
    now: DateTime<Utc>,
    reset_time: NaiveTime,
    timezone: &str,
) -> Option<DateTime<Utc>> {
    let offset_now = timezone_offset_seconds_for_utc(timezone, now)?;
    let local_now = now.naive_utc() + Duration::seconds(offset_now.into());
    let mut target_date = local_now.date();
    let mut target_local = target_date.and_time(reset_time);
    if target_local <= local_now {
        target_date = target_date.checked_add_signed(Duration::days(1))?;
        target_local = target_date.and_time(reset_time);
    }
    let offset_target = timezone_offset_seconds_for_local_date(timezone, target_date)?;
    Some(DateTime::from_naive_utc_and_offset(
        target_local - Duration::seconds(offset_target.into()),
        Utc,
    ))
}

fn timezone_offset_seconds_for_utc(timezone: &str, now: DateTime<Utc>) -> Option<i32> {
    match timezone {
        "UTC" | "Etc/UTC" => Some(0),
        "Asia/Nicosia" => {
            let date = (now.naive_utc() + Duration::hours(2)).date();
            Some(nicosia_offset_seconds_for_local_date(date))
        }
        _ => None,
    }
}

fn timezone_offset_seconds_for_local_date(timezone: &str, date: NaiveDate) -> Option<i32> {
    match timezone {
        "UTC" | "Etc/UTC" => Some(0),
        "Asia/Nicosia" => Some(nicosia_offset_seconds_for_local_date(date)),
        _ => None,
    }
}

fn nicosia_offset_seconds_for_local_date(date: NaiveDate) -> i32 {
    let month = date.month();
    let day = date.day();
    let dst = match month {
        4..=9 => true,
        1 | 2 | 11 | 12 => false,
        3 => day >= last_sunday_of_month(date.year(), 3),
        10 => day < last_sunday_of_month(date.year(), 10),
        _ => false,
    };
    if dst {
        3 * 3600
    } else {
        2 * 3600
    }
}

fn last_sunday_of_month(year: i32, month: u32) -> u32 {
    let mut day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    };
    loop {
        let date = Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap();
        if date.weekday().num_days_from_sunday() == 0 {
            return day;
        }
        day -= 1;
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_058_classification_matrix_covers_runtime_observations() {
        let cases = [
            (
                RuntimeFailureObservation::ProviderQuota { retry_after: None },
                AgentFailureKind::ProviderQuota,
                OperatorActionHint::WaitUntilRetryAfter,
            ),
            (
                RuntimeFailureObservation::ProviderPermissionRequired,
                AgentFailureKind::ProviderPermissionRequired,
                OperatorActionHint::AuthorizeXcode,
            ),
            (
                RuntimeFailureObservation::ProviderPermissionRejected,
                AgentFailureKind::ProviderPermissionRejected,
                OperatorActionHint::InspectLogs,
            ),
            (
                RuntimeFailureObservation::ProviderTimeout {
                    supervision_classification: Some("idle_hang_before_first_progress".into()),
                },
                AgentFailureKind::ProviderTimeout,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::ProviderInternalError,
                AgentFailureKind::ProviderInternalError,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::TransportEpipe,
                AgentFailureKind::TransportEpipe,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::TransportProtocolError,
                AgentFailureKind::TransportProtocolError,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::TransportClosed,
                AgentFailureKind::TransportClosed,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::McpStartupTimeout,
                AgentFailureKind::McpStartupTimeout,
                OperatorActionHint::InspectLogs,
            ),
            (
                RuntimeFailureObservation::McpPermissionModalStall,
                AgentFailureKind::McpPermissionModalStall,
                OperatorActionHint::AuthorizeXcode,
            ),
            (
                RuntimeFailureObservation::XcodeHostEnvironmentError,
                AgentFailureKind::XcodeHostEnvironmentError,
                OperatorActionHint::InspectLogs,
            ),
            (
                RuntimeFailureObservation::MissingRequiredOutputs,
                AgentFailureKind::MissingRequiredOutputs,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::InvalidOutputContract,
                AgentFailureKind::InvalidOutputContract,
                OperatorActionHint::Retry,
            ),
            (
                RuntimeFailureObservation::CancelledByOperator,
                AgentFailureKind::CancelledByOperator,
                OperatorActionHint::InspectLogs,
            ),
            (
                RuntimeFailureObservation::SupersededByRetry,
                AgentFailureKind::SupersededByRetry,
                OperatorActionHint::Retry,
            ),
        ];
        for (observation, expected_kind, expected_hint) in cases {
            let classification = classify_observation(observation);
            assert_eq!(classification.failure_kind, expected_kind);
            assert_eq!(classification.operator_action_hint, expected_hint);
        }
    }

    #[test]
    fn proposal_058_acp_error_adapter_produces_typed_observations() {
        assert_eq!(
            observation_from_acp_error_message("ACP write error: Error: write EPIPE"),
            RuntimeFailureObservation::TransportEpipe
        );
        assert_eq!(
            observation_from_acp_error_message(
                "ACP: send session/prompt: write ACP message to subprocess stdin: broken pipe"
            ),
            RuntimeFailureObservation::TransportEpipe
        );
        assert!(matches!(
            observation_from_acp_error_message(
                "provider quota exceeded retry_after=2026-04-19T11:00:00Z"
            ),
            RuntimeFailureObservation::ProviderQuota {
                retry_after: Some(_)
            }
        ));
        assert_eq!(
            observation_from_acp_error_message("ACP session idle timeout: no message"),
            RuntimeFailureObservation::ProviderTimeout {
                supervision_classification: Some("idle_hang_before_first_progress".into())
            }
        );
        assert_eq!(
            observation_from_acp_error_message("ACP session closed during active prompt"),
            RuntimeFailureObservation::TransportClosed
        );
        assert_eq!(
            observation_from_acp_error_message(
                "xcode_mcp_warmup_failed: initialize lease 'lease-1': xcode_mcp_initialize_timeout: timed out after 600s waiting for brokered Xcode MCP method 'initialize'"
            ),
            RuntimeFailureObservation::McpPermissionModalStall
        );
    }

    #[test]
    fn proposal_058_quota_retry_after_parses_local_provider_reset_message() {
        let now = DateTime::parse_from_rfc3339("2026-04-19T16:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let retry_after = extract_retry_after_from_message_at(
            "provider limit reached; resets 10pm (Asia/Nicosia)",
            now,
        )
        .expect("Nicosia provider reset should parse");

        assert_eq!(retry_after.to_rfc3339(), "2026-04-19T19:00:00+00:00");
        assert!(matches!(
            observation_from_acp_error_message(
                "provider limit reached; resets 10pm (Asia/Nicosia)"
            ),
            RuntimeFailureObservation::ProviderQuota {
                retry_after: Some(_)
            }
        ));
    }

    #[test]
    fn provider_quota_parses_relative_reset_message() {
        let now = DateTime::parse_from_rfc3339("2026-04-26T07:24:05Z")
            .unwrap()
            .with_timezone(&Utc);
        let retry_after = extract_retry_after_from_message_at(
            "You have exhausted your capacity on this model. Your quota will reset after 12h21m1s.",
            now,
        )
        .expect("relative provider reset should parse");

        assert_eq!(retry_after.to_rfc3339(), "2026-04-26T19:45:06+00:00");
        assert!(matches!(
            observation_from_acp_error_message(
                "You have exhausted your capacity on this model. Your quota will reset after 12h21m1s."
            ),
            RuntimeFailureObservation::ProviderQuota {
                retry_after: Some(_)
            }
        ));
    }

    #[test]
    fn proposal_058_p051_backend_start_failure_uses_typed_observation_discriminators() {
        assert_eq!(
            observation_from_acp_error_message(
                r#"{"observation_kind":"backend_start_failed","server_id":"xcode","broker_used":true,"failure_scope":"startup","elapsed_ms":30000}"#
            ),
            RuntimeFailureObservation::McpPermissionModalStall
        );
        assert_eq!(
            observation_from_acp_error_message(
                r#"{"observation_kind":"backend_start_failed","server_id":"xcode","broker_used":true,"failure_scope":"host_environment","host_env_error_kind":"xcode_host_env_unavailable","elapsed_ms":30000}"#
            ),
            RuntimeFailureObservation::XcodeHostEnvironmentError
        );
        assert_eq!(
            observation_from_acp_error_message(
                r#"{"observation_kind":"xcode_permission_modal_stall","server_id":"xcode","mcp_xcode_pid_present":true,"elapsed_ms":30000}"#
            ),
            RuntimeFailureObservation::McpPermissionModalStall
        );
    }
}
