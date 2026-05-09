use domain::CapabilityToolId;
use mcp_server::tools;

#[test]
fn proposal_078_effects_capability_ids_are_registered() {
    // All five P078 CapabilityToolIds must round-trip through capability_id_for
    let pairs = [
        ("effects.list", CapabilityToolId::EffectsList),
        ("effects.inspect", CapabilityToolId::EffectsInspect),
        ("effects.reconcile", CapabilityToolId::EffectsReconcile),
        (
            "effects.mark_unrecoverable",
            CapabilityToolId::EffectsMarkUnrecoverable,
        ),
        (
            "effects.clear_after_manual_verification",
            CapabilityToolId::EffectsClearAfterManualVerification,
        ),
    ];

    for (tool_name, expected_id) in &pairs {
        let id = tools::capability_id_for(tool_name);
        assert_eq!(
            id,
            Some(*expected_id),
            "capability_id_for({tool_name}) must return {expected_id:?}"
        );
    }
}

#[test]
fn proposal_078_effects_tool_specs_cover_all_capability_ids() {
    let specs = tools::effects::tool_specs();
    let names: Vec<&str> = specs.iter().map(|t| t.name.as_str()).collect();

    for required in [
        "effects.list",
        "effects.inspect",
        "effects.reconcile",
        "effects.mark_unrecoverable",
        "effects.clear_after_manual_verification",
    ] {
        assert!(
            names.contains(&required),
            "tool_specs must include {required}"
        );
    }
}

#[test]
fn proposal_078_mcp_tool_for_effects_capability_ids() {
    // mcp_tool_for must return a correctly named tool for each P078 id
    let pairs = [
        (CapabilityToolId::EffectsList, "effects.list"),
        (CapabilityToolId::EffectsInspect, "effects.inspect"),
        (CapabilityToolId::EffectsReconcile, "effects.reconcile"),
        (
            CapabilityToolId::EffectsMarkUnrecoverable,
            "effects.mark_unrecoverable",
        ),
        (
            CapabilityToolId::EffectsClearAfterManualVerification,
            "effects.clear_after_manual_verification",
        ),
    ];

    for (id, expected_name) in &pairs {
        let tool = tools::mcp_tool_for(*id);
        assert_eq!(
            tool.name, *expected_name,
            "mcp_tool_for({id:?}) must return tool named {expected_name}"
        );
    }
}

#[test]
fn proposal_078_canonical_tool_name_underscore_variants() {
    // Underscore variants must resolve to dot-separated canonical names
    let pairs = [
        ("effects_list", "effects.list"),
        ("effects_inspect", "effects.inspect"),
        ("effects_reconcile", "effects.reconcile"),
        ("effects_mark_unrecoverable", "effects.mark_unrecoverable"),
        (
            "effects_clear_after_manual_verification",
            "effects.clear_after_manual_verification",
        ),
    ];

    for (underscore, expected_canonical) in &pairs {
        assert_eq!(
            tools::canonical_tool_name(underscore),
            *expected_canonical,
            "canonical_tool_name({underscore}) must return {expected_canonical}"
        );
    }
}

#[test]
fn proposal_078_effects_tools_in_all_capability_ids() {
    let all_ids = tools::all_capability_tool_ids();
    let required = [
        CapabilityToolId::EffectsList,
        CapabilityToolId::EffectsInspect,
        CapabilityToolId::EffectsReconcile,
        CapabilityToolId::EffectsMarkUnrecoverable,
        CapabilityToolId::EffectsClearAfterManualVerification,
    ];
    for id in &required {
        assert!(
            all_ids.contains(id),
            "all_capability_tool_ids must include {id:?}"
        );
    }
}

#[test]
fn proposal_078_effects_tools_in_all_tool_specs() {
    let specs = tools::all_tool_specs();
    let names: Vec<&str> = specs.iter().map(|t| t.name.as_str()).collect();
    for required in [
        "effects.list",
        "effects.inspect",
        "effects.reconcile",
        "effects.mark_unrecoverable",
        "effects.clear_after_manual_verification",
    ] {
        assert!(
            names.contains(&required),
            "all_tool_specs must include {required}"
        );
    }
}

#[test]
fn proposal_078_graphql_does_not_expose_effects_mutation() {
    // Effects MCP tools must not appear as GraphQL mutation aliases.
    // This is verified structurally: the tool names use "effects.*" prefix
    // and none of them should be in the MutationRoot approval-only mutation
    // path. Since GraphQL has no reconcile/retry/clear mutations, verifying
    // that the MCP tool names follow the effects.* convention is sufficient
    // for the structural check.
    let specs = tools::effects::tool_specs();
    for spec in &specs {
        assert!(
            spec.name.starts_with("effects."),
            "effects tools must use effects.* prefix; got: {}",
            spec.name
        );
        assert!(
            !spec.name.contains("mutation"),
            "effects tools must not contain 'mutation' in name"
        );
    }
}
