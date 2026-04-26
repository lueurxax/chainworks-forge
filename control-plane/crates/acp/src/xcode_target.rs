use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use domain::xcode_runtime::XcodeRuntimeFailureClass;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeTargetSelectionInput {
    pub workspace_root: String,
    #[serde(default)]
    pub runtime_profile_id: Option<String>,
    #[serde(default)]
    pub xcode_pid_selector: Option<String>,
    #[serde(default)]
    pub permission_profile_id: Option<String>,
    pub broker_contract_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProbeContext {
    #[serde(default)]
    pub expected_gui_uid: Option<u32>,
    #[serde(default)]
    pub operator_home: Option<String>,
    #[serde(default)]
    pub darwin_tmpdir: Option<String>,
    #[serde(default)]
    pub developer_dir: Option<String>,
    #[serde(default)]
    pub candidate_xcodes: Vec<XcodeProcessCandidate>,
}

#[derive(Clone, Debug, Default)]
pub struct LocalXcodeHostProbeConfig {
    pub expected_gui_uid: Option<u32>,
    pub operator_home: Option<String>,
    pub darwin_tmpdir: Option<String>,
    pub developer_dir: Option<String>,
    pub workspace_roots: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeProcessCandidate {
    pub pid: i64,
    pub uid: u32,
    #[serde(default)]
    pub workspace_identity: Option<String>,
    #[serde(default)]
    pub app_path: Option<String>,
    #[serde(default)]
    pub developer_dir: Option<String>,
    #[serde(default)]
    pub operator_home: Option<String>,
    #[serde(default)]
    pub darwin_tmpdir: Option<String>,
    #[serde(default = "default_alive")]
    pub alive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeTargetSnapshot {
    pub xcode_pid: i64,
    pub workspace_identity: String,
    pub developer_dir: String,
    pub operator_home: String,
    pub darwin_tmpdir: String,
    pub selection_confidence: XcodeTargetSelectionConfidence,
    #[serde(default)]
    pub runtime_profile_id: Option<String>,
    #[serde(default)]
    pub permission_profile_id: Option<String>,
    pub broker_contract_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeTargetSelectionConfidence {
    ExplicitPid,
    WorkspaceMatch,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XcodeTargetResolver;

impl XcodeTargetResolver {
    pub fn resolve(
        &self,
        input: &XcodeTargetSelectionInput,
        host: &HostProbeContext,
    ) -> Result<XcodeTargetSnapshot> {
        let live_candidates = host
            .candidate_xcodes
            .iter()
            .filter(|candidate| candidate.alive)
            .collect::<Vec<_>>();

        let (candidate, confidence) = if let Some(pid) = input
            .xcode_pid_selector
            .as_deref()
            .and_then(parse_pid_selector)
        {
            let Some(candidate) = live_candidates
                .iter()
                .copied()
                .find(|candidate| candidate.pid == pid)
            else {
                bail!("xcode_target_not_found: no live Xcode process matches pid {pid}");
            };
            (candidate, XcodeTargetSelectionConfidence::ExplicitPid)
        } else {
            let workspace_identity =
                selector_workspace_identity(input).unwrap_or_else(|| input.workspace_root.as_str());
            let matches = live_candidates
                .iter()
                .copied()
                .filter(|candidate| {
                    candidate.workspace_identity.as_deref() == Some(workspace_identity)
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [candidate] => (*candidate, XcodeTargetSelectionConfidence::WorkspaceMatch),
                [] => bail!(
                    "xcode_target_not_found: no live Xcode process matches workspace '{workspace_identity}'"
                ),
                _ => bail!(
                    "xcode_target_ambiguous: {} live Xcode processes match workspace '{workspace_identity}'",
                    matches.len()
                ),
            }
        };

        if let Some(expected_uid) = host.expected_gui_uid {
            if candidate.uid != expected_uid {
                bail!(
                    "host_env_unavailable: selected Xcode pid {} belongs to uid {}, expected GUI uid {}",
                    candidate.pid,
                    candidate.uid,
                    expected_uid
                );
            }
        }

        let operator_home = candidate
            .operator_home
            .as_deref()
            .or(host.operator_home.as_deref())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "host_env_unavailable: operator home unavailable for Xcode pid {}",
                    candidate.pid
                )
            })?;
        let darwin_tmpdir = candidate
            .darwin_tmpdir
            .as_deref()
            .or(host.darwin_tmpdir.as_deref())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "host_env_unavailable: Darwin tmpdir unavailable for Xcode pid {}",
                    candidate.pid
                )
            })?;
        let developer_dir = candidate
            .developer_dir
            .as_deref()
            .or(host.developer_dir.as_deref())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "host_env_unavailable: developer dir unavailable for Xcode pid {}",
                    candidate.pid
                )
            })?;

        Ok(XcodeTargetSnapshot {
            xcode_pid: candidate.pid,
            workspace_identity: candidate
                .workspace_identity
                .clone()
                .unwrap_or_else(|| input.workspace_root.clone()),
            developer_dir: developer_dir.to_string(),
            operator_home: operator_home.to_string(),
            darwin_tmpdir: darwin_tmpdir.to_string(),
            selection_confidence: confidence,
            runtime_profile_id: input.runtime_profile_id.clone(),
            permission_profile_id: input.permission_profile_id.clone(),
            broker_contract_hash: input.broker_contract_hash.clone(),
        })
    }
}

pub fn target_resolver_failure_class(error: &anyhow::Error) -> XcodeRuntimeFailureClass {
    let message = error.to_string();
    if message.contains("xcode_target_not_found") {
        XcodeRuntimeFailureClass::XcodeTargetNotFound
    } else if message.contains("xcode_target_ambiguous") {
        XcodeRuntimeFailureClass::XcodeTargetAmbiguous
    } else if message.contains("host_env_unavailable") {
        XcodeRuntimeFailureClass::HostEnvUnavailable
    } else {
        XcodeRuntimeFailureClass::BrokerInfrastructure
    }
}

pub fn probe_local_xcode_host(config: LocalXcodeHostProbeConfig) -> HostProbeContext {
    let expected_gui_uid = config.expected_gui_uid.or_else(current_uid);
    let operator_home = config.operator_home.or_else(|| {
        dirs::home_dir()
            .and_then(|path| path.into_os_string().into_string().ok())
            .filter(|value| !value.is_empty())
    });
    let darwin_tmpdir = config
        .darwin_tmpdir
        .or_else(|| std::env::var("TMPDIR").ok())
        .filter(|value| !value.is_empty());
    let developer_dir = config
        .developer_dir
        .or_else(|| std::env::var("DEVELOPER_DIR").ok())
        .or_else(xcode_select_developer_dir)
        .filter(|value| !value.is_empty());

    let ps_output = command_stdout_utf8("ps", &["-axo", "pid=,uid=,command="]).unwrap_or_default();
    let mut candidate_xcodes = parse_xcode_process_candidates(
        &ps_output,
        &config.workspace_roots,
        operator_home.as_deref(),
        darwin_tmpdir.as_deref(),
        developer_dir.as_deref(),
    );
    let applescript_workspace_identity = if candidate_xcodes.len() == 1 {
        discover_xcode_workspace_identity_from_documents(&config.workspace_roots)
    } else {
        None
    };
    for candidate in &mut candidate_xcodes {
        if candidate.workspace_identity.is_none() {
            candidate.workspace_identity =
                discover_xcode_workspace_identity(candidate.pid, &config.workspace_roots)
                    .or_else(|| applescript_workspace_identity.clone());
        }
    }

    HostProbeContext {
        expected_gui_uid,
        operator_home,
        darwin_tmpdir,
        developer_dir,
        candidate_xcodes,
    }
}

fn parse_pid_selector(selector: &str) -> Option<i64> {
    selector
        .strip_prefix("pid:")
        .unwrap_or(selector)
        .parse::<i64>()
        .ok()
}

fn selector_workspace_identity(input: &XcodeTargetSelectionInput) -> Option<&str> {
    input
        .xcode_pid_selector
        .as_deref()
        .and_then(|selector| selector.strip_prefix("workspace:"))
}

fn default_alive() -> bool {
    true
}

fn current_uid() -> Option<u32> {
    command_stdout_utf8("id", &["-u"]).and_then(|value| value.trim().parse::<u32>().ok())
}

fn xcode_select_developer_dir() -> Option<String> {
    command_stdout_utf8("xcode-select", &["-p"]).map(|value| value.trim().to_string())
}

fn command_stdout_utf8(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string()
    })
}

fn parse_xcode_process_candidates(
    ps_output: &str,
    workspace_roots: &[String],
    operator_home: Option<&str>,
    darwin_tmpdir: Option<&str>,
    developer_dir: Option<&str>,
) -> Vec<XcodeProcessCandidate> {
    ps_output
        .lines()
        .filter_map(parse_xcode_ps_line)
        .map(|(pid, uid, command)| XcodeProcessCandidate {
            pid,
            uid,
            workspace_identity: workspace_identity_from_command(&command, workspace_roots),
            app_path: xcode_app_path_from_command(&command),
            developer_dir: developer_dir.map(str::to_string),
            operator_home: operator_home.map(str::to_string),
            darwin_tmpdir: darwin_tmpdir.map(str::to_string),
            alive: true,
        })
        .collect()
}

fn parse_xcode_ps_line(line: &str) -> Option<(i64, u32, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let pid_token = trimmed.split_whitespace().next()?;
    let pid = pid_token.parse::<i64>().ok()?;
    let rest = trimmed[pid_token.len()..].trim_start();
    let uid_token = rest.split_whitespace().next()?;
    let uid = uid_token.parse::<u32>().ok()?;
    let command = rest[uid_token.len()..].trim_start().to_string();
    is_xcode_process_command(&command).then_some((pid, uid, command))
}

fn is_xcode_process_command(command: &str) -> bool {
    command == "Xcode" || command.starts_with("Xcode ") || command.contains("/Contents/MacOS/Xcode")
}

fn xcode_app_path_from_command(command: &str) -> Option<String> {
    let marker = ".app/Contents/MacOS/Xcode";
    let marker_start = command.find(marker)?;
    let app_end = marker_start + ".app".len();
    let prefix = &command[..app_end];
    let path_start = prefix.rfind(" /").map(|index| index + 1).unwrap_or(0);
    Some(prefix[path_start..].to_string())
}

fn workspace_identity_from_command(command: &str, workspace_roots: &[String]) -> Option<String> {
    workspace_roots
        .iter()
        .filter(|root| !root.is_empty())
        .find(|root| command.contains(root.as_str()))
        .cloned()
        .or_else(|| {
            command
                .split_whitespace()
                .find(|part| part.ends_with(".xcworkspace") || part.ends_with(".xcodeproj"))
                .map(str::to_string)
        })
}

fn discover_xcode_workspace_identity(pid: i64, workspace_roots: &[String]) -> Option<String> {
    let pid = pid.to_string();
    let output = command_stdout_utf8("lsof", &["-p", &pid, "-Fn"])?;
    parse_lsof_workspace_identity(&output, workspace_roots)
}

fn discover_xcode_workspace_identity_from_documents(workspace_roots: &[String]) -> Option<String> {
    let script = r#"tell application "Xcode" to get path of documents"#;
    let output = command_stdout_utf8("osascript", &["-e", script])?;
    workspace_identity_from_xcode_document_paths(&output, workspace_roots)
}

fn parse_lsof_workspace_identity(lsof_output: &str, workspace_roots: &[String]) -> Option<String> {
    let open_paths = lsof_output
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .collect::<Vec<_>>();
    workspace_roots
        .iter()
        .filter(|root| !root.is_empty())
        .find(|root| {
            open_paths
                .iter()
                .any(|path| path_is_under(path, root) || *path == root.as_str())
        })
        .cloned()
        .or_else(|| {
            open_paths
                .iter()
                .find(|path| path.ends_with(".xcworkspace") || path.ends_with(".xcodeproj"))
                .map(|path| (*path).to_string())
        })
}

fn workspace_identity_from_xcode_document_paths(
    document_paths: &str,
    workspace_roots: &[String],
) -> Option<String> {
    let paths = document_paths
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    workspace_roots
        .iter()
        .filter(|root| !root.is_empty())
        .find(|root| {
            paths
                .iter()
                .any(|path| path_is_under(path, root) || *path == root.as_str())
        })
        .cloned()
        .or_else(|| {
            paths
                .iter()
                .find(|path| path.ends_with(".xcworkspace") || path.ends_with(".xcodeproj"))
                .map(|path| (*path).to_string())
        })
}

fn path_is_under(path: &str, root: &str) -> bool {
    Path::new(path).starts_with(Path::new(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_prefers_explicit_live_pid() {
        let snapshot = XcodeTargetResolver
            .resolve(
                &input(Some("pid:42")),
                &host(vec![candidate(41, Some("/workspace")), candidate(42, None)]),
            )
            .unwrap();

        assert_eq!(snapshot.xcode_pid, 42);
        assert_eq!(
            snapshot.selection_confidence,
            XcodeTargetSelectionConfidence::ExplicitPid
        );
        assert_eq!(snapshot.workspace_identity, "/workspace");
    }

    #[test]
    fn resolver_matches_workspace_without_newest_pid_heuristic() {
        let snapshot = XcodeTargetResolver
            .resolve(
                &input(None),
                &host(vec![
                    candidate(100, Some("/other")),
                    candidate(200, Some("/workspace")),
                ]),
            )
            .unwrap();

        assert_eq!(snapshot.xcode_pid, 200);
        assert_eq!(
            snapshot.selection_confidence,
            XcodeTargetSelectionConfidence::WorkspaceMatch
        );
    }

    #[test]
    fn resolver_fails_closed_when_workspace_is_missing_or_ambiguous() {
        let missing = XcodeTargetResolver
            .resolve(&input(None), &host(vec![candidate(1, Some("/other"))]))
            .unwrap_err();
        assert_eq!(
            target_resolver_failure_class(&missing),
            XcodeRuntimeFailureClass::XcodeTargetNotFound
        );

        let ambiguous = XcodeTargetResolver
            .resolve(
                &input(None),
                &host(vec![
                    candidate(1, Some("/workspace")),
                    candidate(2, Some("/workspace")),
                ]),
            )
            .unwrap_err();
        assert_eq!(
            target_resolver_failure_class(&ambiguous),
            XcodeRuntimeFailureClass::XcodeTargetAmbiguous
        );
    }

    #[test]
    fn resolver_fails_closed_on_gui_uid_or_host_env_mismatch() {
        let uid_mismatch = XcodeTargetResolver
            .resolve(
                &input(Some("pid:9")),
                &HostProbeContext {
                    expected_gui_uid: Some(501),
                    candidate_xcodes: vec![XcodeProcessCandidate {
                        uid: 502,
                        ..candidate(9, Some("/workspace"))
                    }],
                    ..host(Vec::new())
                },
            )
            .unwrap_err();
        assert_eq!(
            target_resolver_failure_class(&uid_mismatch),
            XcodeRuntimeFailureClass::HostEnvUnavailable
        );

        let missing_home = XcodeTargetResolver
            .resolve(
                &input(None),
                &HostProbeContext {
                    operator_home: None,
                    candidate_xcodes: vec![XcodeProcessCandidate {
                        operator_home: None,
                        ..candidate(3, Some("/workspace"))
                    }],
                    ..host(Vec::new())
                },
            )
            .unwrap_err();
        assert_eq!(
            target_resolver_failure_class(&missing_home),
            XcodeRuntimeFailureClass::HostEnvUnavailable
        );
    }

    #[test]
    fn local_probe_parser_collects_live_xcode_processes_without_newest_selection() {
        let workspace_roots = vec![
            "/Users/gui/Work/App".to_string(),
            "/Users/gui/Work/Other".to_string(),
        ];
        let ps_output = "\
          99 501 /Applications/Xcode.app/Contents/MacOS/Xcode /Users/gui/Work/App/App.xcworkspace
         120 501 /Applications/Utilities/Terminal.app/Contents/MacOS/Terminal
         150 502 /Applications/Xcode.app/Contents/MacOS/Xcode /Users/gui/Work/Other/Other.xcodeproj
        ";

        let candidates = parse_xcode_process_candidates(
            ps_output,
            &workspace_roots,
            Some("/Users/gui"),
            Some("/var/folders/gui/T/"),
            Some("/Applications/Xcode.app/Contents/Developer"),
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].pid, 99);
        assert_eq!(
            candidates[0].workspace_identity.as_deref(),
            Some("/Users/gui/Work/App")
        );
        assert_eq!(
            candidates[0].app_path.as_deref(),
            Some("/Applications/Xcode.app")
        );
        assert_eq!(candidates[1].pid, 150);
        assert_eq!(candidates[1].uid, 502);
        assert_eq!(
            candidates[1].workspace_identity.as_deref(),
            Some("/Users/gui/Work/Other")
        );
    }

    #[test]
    fn lsof_workspace_parser_prefers_declared_workspace_roots() {
        let workspace_roots = vec!["/Users/gui/Work/App".to_string()];
        let lsof_output = "\
p4242
n/Users/gui/Work/App/App.xcodeproj/project.pbxproj
n/Applications/Xcode.app/Contents/MacOS/Xcode
        ";

        assert_eq!(
            parse_lsof_workspace_identity(lsof_output, &workspace_roots).as_deref(),
            Some("/Users/gui/Work/App")
        );
    }

    #[test]
    fn xcode_document_paths_parser_prefers_declared_workspace_roots() {
        let workspace_roots = vec!["/Users/gui/Work/App".to_string()];
        let document_paths = "\
/Users/gui/Work/App/App.xcodeproj, /E4262B49-42DE-4A9E-AD50-8E2C86983AE2, /Users/gui/Work/App/Sources/App.swift";

        assert_eq!(
            workspace_identity_from_xcode_document_paths(document_paths, &workspace_roots)
                .as_deref(),
            Some("/Users/gui/Work/App")
        );
    }

    fn input(selector: Option<&str>) -> XcodeTargetSelectionInput {
        XcodeTargetSelectionInput {
            workspace_root: "/workspace".to_string(),
            runtime_profile_id: Some("runtime-profile".to_string()),
            xcode_pid_selector: selector.map(str::to_string),
            permission_profile_id: Some("permission-profile".to_string()),
            broker_contract_hash: "contract-hash".to_string(),
        }
    }

    fn host(candidate_xcodes: Vec<XcodeProcessCandidate>) -> HostProbeContext {
        HostProbeContext {
            expected_gui_uid: Some(501),
            operator_home: Some("/Users/gui".to_string()),
            darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
            developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
            candidate_xcodes,
        }
    }

    fn candidate(pid: i64, workspace_identity: Option<&str>) -> XcodeProcessCandidate {
        XcodeProcessCandidate {
            pid,
            uid: 501,
            workspace_identity: workspace_identity.map(str::to_string),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }
    }
}
