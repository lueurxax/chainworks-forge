use super::*;

const TOOLS: [&str; 6] = [
    "runs.continuation_preview",
    "runs.continue_blocked",
    "runs.continuation_get",
    "runs.continuation_activate",
    "runs.continuation_reconcile",
    "runs.continuation_abort",
];

fn explicit(class: PrincipalClass, tools: &[&str]) -> Principal {
    Principal::from_entry(&PrincipalEntry {
        id: "p039-test".into(),
        class,
        surface_policies: Some(SurfacePolicies {
            graphql: None,
            mcp: Some(McpPolicy {
                allowed_tools: tools.iter().map(|s| s.to_string()).collect(),
            }),
        }),
        ..Default::default()
    })
}

#[test]
fn p039_six_exact_names_are_independent_explicit_capabilities() {
    let mut ids = BTreeSet::new();
    for name in TOOLS {
        let principal = explicit(PrincipalClass::Operator, &[name]);
        assert!(
            is_tool_allowed(&principal, name),
            "missing explicit mapping: {name}"
        );
        assert_eq!(principal.tool_capabilities.len(), 1);
        ids.extend(principal.tool_capabilities);
        for other in TOOLS.into_iter().filter(|other| *other != name) {
            assert!(!is_tool_allowed(
                &explicit(PrincipalClass::Operator, &[name]),
                other
            ));
        }
    }
    assert_eq!(ids.len(), 6);
}

#[test]
fn p039_default_narrow_and_namespace_grants_do_not_expand() {
    for principal in [
        Principal::new("default", PrincipalClass::Operator),
        explicit(PrincipalClass::Operator, &["runs.start"]),
        explicit(PrincipalClass::Operator, &["runs.*"]),
        explicit(PrincipalClass::Operator, &[]),
    ] {
        for name in TOOLS {
            assert!(!is_tool_allowed(&principal, name));
        }
    }
}

#[test]
fn p039_v3_names_validate_without_expanding_a_narrow_allowlist() {
    for names in [TOOLS.to_vec(), vec!["runs.start"]] {
        let file: PrincipalTableFile = serde_json::from_value(serde_json::json!({
            "schema_version": 3,
            "principals": [{"id": "p039-explicit", "token": "fixture-token-not-a-live-credential",
                "class": "operator", "surface_policies": {"mcp": {"allowed_tools": names}}}]
        }))
        .unwrap();
        validate_v2_principals(&file.principals).unwrap();
        validate_v3_principals(&file.principals).unwrap();
        let principal = Principal::from_entry(&file.principals[0]);
        for name in TOOLS {
            assert_eq!(is_tool_allowed(&principal, name), names.contains(&name));
        }
    }
}

#[test]
fn p039_mutations_require_global_operator_and_exact_capability() {
    for name in [TOOLS[1], TOOLS[3], TOOLS[4], TOOLS[5]] {
        let id = capability_tool_id_for_name(name).unwrap();
        let global = explicit(PrincipalClass::Operator, &[name]);
        assert!(require_p039_mutation(&global, id).is_ok());
        for scope in [Some(vec![]), Some(vec!["source".into()])] {
            let mut scoped = global.clone();
            scoped.run_scope = scope;
            assert!(require_p039_mutation(&scoped, id).is_err());
            assert!(!is_tool_allowed(&scoped, name));
        }
        for class in [
            PrincipalClass::ReadOnlyOperator,
            PrincipalClass::Observer,
            PrincipalClass::Agent,
        ] {
            let mut other = explicit(class, &[name]);
            other.tool_capabilities.insert(id);
            assert!(require_p039_mutation(&other, id).is_err());
            assert!(!is_tool_allowed(&other, name));
        }
        for caller in [
            CallerClass::Automation,
            CallerClass::Observer,
            CallerClass::DeveloperBreakGlass,
        ] {
            let mut other = global.clone();
            other.caller_class_override = Some(caller);
            assert!(require_p039_mutation(&other, id).is_err());
            assert!(!is_tool_allowed(&other, name));
        }
        assert!(
            require_p039_mutation(&explicit(PrincipalClass::Operator, &["runs.start"]), id)
                .is_err()
        );
    }
    for name in [TOOLS[0], TOOLS[2], "runs.start"] {
        let id = capability_tool_id_for_name(name).unwrap();
        assert!(require_p039_mutation(&explicit(PrincipalClass::Operator, &[name]), id).is_err());
    }
}

#[test]
fn p039_reads_are_explicit_capabilities_not_global_mutation_grants() {
    for name in [TOOLS[0], TOOLS[2]] {
        for class in [
            PrincipalClass::Operator,
            PrincipalClass::ReadOnlyOperator,
            PrincipalClass::Observer,
            PrincipalClass::Agent,
        ] {
            let mut principal = explicit(class.clone(), &[name]);
            principal.run_scope = Some(vec!["source".into()]);
            assert!(is_tool_allowed(&principal, name));
            // Run access still belongs to the service's existing source/successor checks.
            assert!(!is_tool_allowed(&explicit(class, &["runs.get"]), name));
        }
    }
}

#[test]
fn p039_boundary_namespace_match_does_not_grant_tool_authority() {
    use boundary::{BoundaryPolicy, PolicyDecision, PolicyMode};
    let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
    let narrow = explicit(PrincipalClass::Operator, &["runs.start"]);
    for name in TOOLS {
        assert!(matches!(
            policy.evaluate("agent_operator", "mcp_tools_call", Some(name)),
            PolicyDecision::Allow { .. }
        ));
        assert!(!is_tool_allowed(&narrow, name));
    }
}

#[test]
fn p039_replay_requires_fresh_auth_after_revocation_disable_or_rescope() {
    let token = "p039-fixture-only-token-xxxxxxxxxxxxxxxx";
    let entry = PrincipalEntry {
        token: token.into(),
        id: "p039".into(),
        class: PrincipalClass::Operator,
        surface_policies: Some(SurfacePolicies {
            graphql: None,
            mcp: Some(McpPolicy {
                allowed_tools: vec![TOOLS[1].into()],
            }),
        }),
        ..Default::default()
    };
    let source = LivePrincipalSource::new(PrincipalTable {
        entries: vec![entry.clone()],
    });
    let id = capability_tool_id_for_name(TOOLS[1]).unwrap();
    let fingerprint = token_fingerprint(token);
    assert!(require_p039_mutation(&source.resolve_bearer(token).unwrap(), id).is_ok());
    for changed in [
        PrincipalEntry {
            disabled: Some(true),
            ..entry.clone()
        },
        PrincipalEntry {
            expires_at_ms: Some(1),
            ..entry.clone()
        },
        PrincipalEntry {
            token: "replacement-fixture-token-xxxxxxxxxxxxxxxx".into(),
            ..entry.clone()
        },
    ] {
        source.replace(PrincipalTable {
            entries: vec![changed],
        });
        assert!(source.resolve_bearer(token).is_err());
        assert!(source
            .resolve_principal_by_id_and_token_fingerprint("p039", &fingerprint)
            .is_err());
    }
    for scope in [vec![], vec!["source".into()]] {
        source.replace(PrincipalTable {
            entries: vec![PrincipalEntry {
                run_scope: Some(scope),
                ..entry.clone()
            }],
        });
        let current = source
            .resolve_principal_by_id_and_token_fingerprint("p039", &fingerprint)
            .unwrap();
        assert!(require_p039_mutation(&current, id).is_err());
    }
    source.replace(PrincipalTable { entries: vec![] });
    assert!(source.resolve_bearer(token).is_err());
}
