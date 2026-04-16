use domain::commands::Command;

/// Per-variant redaction of serialized Command JSON before it is written
/// to the command_journal table. Removes sensitive fields that should not
/// be persisted in plain text.
///
/// Called inside CommandHandler::handle right after serde_json::to_string.
pub fn redact_for_journal(cmd: &Command, payload_json: &str) -> String {
    // Parse, redact sensitive fields, re-serialize.
    // For now, the only sensitive field pattern is operator comments
    // on approval/rejection commands — these may contain free-text
    // that the operator typed. We redact the comment field value
    // but keep the key so audit readers know a comment was present.
    let mut value: serde_json::Value = match serde_json::from_str(payload_json) {
        Ok(v) => v,
        Err(_) => return payload_json.to_string(),
    };

    match cmd {
        Command::ApproveStage(_) | Command::RejectStage(_) => {
            if let Some(obj) = value.as_object_mut() {
                // The Command enum serializes as { "ApproveStage": { ... } }
                for (_, inner) in obj.iter_mut() {
                    if let Some(inner_obj) = inner.as_object_mut() {
                        if inner_obj.contains_key("comment") {
                            inner_obj.insert(
                                "comment".to_string(),
                                serde_json::Value::String("[REDACTED]".to_string()),
                            );
                        }
                    }
                }
            }
        }
        _ => {}
    }

    serde_json::to_string(&value).unwrap_or_else(|_| payload_json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::commands::{ApproveStageCmd, Command};
    use domain::ids::RunId;

    #[test]
    fn redacts_approval_comment() {
        let cmd = Command::ApproveStage(ApproveStageCmd {
            run_id: RunId::new(),
            stage_id: "test_stage".into(),
            comment: Some("LGTM - sensitive feedback".into()),
        });
        let raw = serde_json::to_string(&cmd).unwrap();
        let redacted = redact_for_journal(&cmd, &raw);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("sensitive feedback"));
    }

    #[test]
    fn preserves_non_sensitive_commands() {
        let cmd = Command::CancelRun(domain::commands::CancelRunCmd {
            run_id: RunId::new(),
        });
        let raw = serde_json::to_string(&cmd).unwrap();
        let redacted = redact_for_journal(&cmd, &raw);
        assert_eq!(raw, redacted);
    }
}
