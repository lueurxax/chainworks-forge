# Chainworks Forge — Code Examples & Patterns

## Example 1: Minimal Workflow YAML

```yaml
schema_version: 1
workflow:
  id: minimal_example
  uses_agent_catalog: ./agents.yaml
  description: Minimal workflow demonstrating core concepts
  initial_state: propose
  states:
    propose:
      label: "Draft proposal"
      owner: proposal_writer
      run:
        sequence:
          - agent: proposal_writer
            task: draft_proposal
            inputs: [input.idea]
            outputs: [proposal_current]
      transitions:
        - to: approve
          when: exists('proposal_current')
    
    approve:
      label: "Approve proposal"
      type: manual_gate
      owner: lead
      approval: required
      transitions:
        - to: done
          when: approval.granted == true
    
    done:
      label: "Complete"
      type: end
      owner: system
```

## Example 2: Workflow with Loops

```yaml
workflow:
  id: revision_loop
  states:
    draft:
      owner: writer
      run:
        sequence:
          - agent: writer
            task: write
            outputs: [document]
      transitions:
        - to: review
          when: exists('document')
    
    review:
      owner: reviewer
      run:
        sequence:
          - agent: reviewer
            task: review
            inputs: [document]
            outputs: [review_feedback]
      transitions:
        - to: revise
          when: exists('review_feedback')
    
    revise:
      owner: writer
      loop:
        counter: revision_count
        max: 5
      run:
        sequence:
          - agent: writer
            task: revise
            inputs: [document, review_feedback]
            outputs: [document]
      transitions:
        - to: review
          when: vars.revision_count < vars.max
        - to: done
          when: vars.revision_count >= vars.max
    
    done:
      type: end
```

## Example 3: Dynamic Parallel (Fan-Out)

```yaml
states:
  parallel_reviews:
    owner: orchestrator
    run:
      dynamic_parallel:
        selector_artifact: reviewer_assignment  # JSON file listing reviewers
        output_contract: review_v1              # Expected output schema
        inputs: [proposal_current]
        outputs: [review_summary]
    transitions:
      - to: next_stage
        when: exists('review_summary')
```

The `reviewer_assignment.json`:
```json
{
  "reviewers": [
    {"id": "ux_reviewer", "agent": "ux_agent"},
    {"id": "architect_reviewer", "agent": "architect"},
    {"id": "product_reviewer", "agent": "product_lead"}
  ]
}
```

## Example 4: Conditional Transitions

```yaml
transitions:
  # Simple existence check
  - to: next_state
    when: exists('output_artifact')
  
  # Field comparison
  - to: success_path
    when: artifact.status == 'approved'
  
  # Numeric comparison
  - to: approved
    when: artifact.score >= 8.5
  
  # Variable comparison
  - to: continue
    when: vars.attempt_count < vars.max_attempts
  
  # Compound logic
  - to: final_review
    when: |
      (artifact.score >= 8.5) AND 
      (exists('security_report')) AND 
      (vars.tests_passed == true)
  
  # Fallback with OR
  - to: publish
    when: |
      exists('approved_version') OR 
      vars.force_publish == true
```

## Example 5: Swift Code — Reading a Workflow

```swift
// In RunPlanCompiler.swift
func previewCompile(
    workflowPath: String,
    catalogPath: String
) throws -> RunPlan {
    // 1. Parse YAML files
    let workflowYAML = try YAMLParser.parse(workflowPath)
    let catalogYAML = try YAMLParser.parse(catalogPath)
    
    // 2. Validate syntax
    try YAMLValidator.validateAll(
        workflow: workflowYAML,
        catalog: catalogYAML
    )
    
    // 3. Resolve agent bindings
    let resolvedAgents = try AgentResolver.resolve(
        catalogYAML.agents,
        backend_profiles: catalogYAML.backend_profiles
    )
    
    // 4. Build executable states
    var states: [String: ExecutableState] = [:]
    for (stateID, stateDefn) in workflowYAML.states {
        states[stateID] = ExecutableState(
            id: stateID,
            owner: stateDefn.owner,
            runBlock: try buildRunBlock(stateDefn.run),
            transitions: try buildTransitions(stateDefn.transitions)
        )
    }
    
    // 5. Return compiled plan
    return RunPlan(
        states: states,
        initialStateID: workflowYAML.initial_state,
        agentBindings: resolvedAgents,
        variables: workflowYAML.variables,
        provenance: try computeProvenance(workflowPath, catalogPath)
    )
}
```

## Example 6: Swift Code — Executing a State

```swift
// In WorkflowOrchestrator.swift
func executeState(_ stateID: String, plan: RunPlan) async throws {
    let executableState = plan.states[stateID]!
    
    // 1. Create stage execution
    let stage = StageExecution(
        id: UUID(),
        stateID: stateID,
        executedAt: Date(),
        agentExecutions: []
    )
    
    // 2. Execute run block
    for phase in executableState.runBlock.phases {
        switch phase.type {
        case .sequential:
            for task in phase.tasks {
                let execution = try await executeAgent(
                    task.agentID,
                    inputs: task.inputs,
                    plan: plan
                )
                stage.agentExecutions.append(execution)
            }
        
        case .parallel:
            let executions = try await withTaskGroup(
                of: AgentExecution.self
            ) { group in
                for task in phase.tasks {
                    group.addTask { [weak self] in
                        try await self?.executeAgent(
                            task.agentID,
                            inputs: task.inputs,
                            plan: plan
                        ) ?? AgentExecution()
                    }
                }
                return try await group.reduce(into: []) {
                    $0.append($1)
                }
            }
            stage.agentExecutions.append(contentsOf: executions)
        }
    }
    
    // 3. Save stage
    try modelContext.insert(stage)
    try modelContext.save()
    
    // 4. Evaluate transitions
    let nextStateID = try evaluateTransitions(
        executableState.transitions,
        context: StateEvaluationContext(stage: stage, plan: plan)
    )
    
    // 5. Loop or finish
    if let next = nextStateID {
        try await executeState(next, plan: plan)
    } else {
        run.completedAt = Date()
        run.status = .completed
    }
}
```

## Example 7: Swift Code — Persisting an Artifact

```swift
// In ArtifactStorage.swift
func store(
    name: String,
    content: String,
    mimeType: String,
    for run: Run
) throws -> Artifact {
    // 1. Resolve artifact path from config
    let pathTemplate = Agent
        Catalog.artifacts[name] ?? 
        "\.chainworks/artifacts/\(name)"
    
    let finalPath = try resolvePath(
        pathTemplate,
        for: run
    )
    
    // 2. Create directory
    try FileManager.default.createDirectory(
        atPath: (finalPath as NSString).deletingLastPathComponent,
        withIntermediateDirectories: true
    )
    
    // 3. Write to disk
    try content.write(
        toFile: finalPath,
        atomically: true,
        encoding: .utf8
    )
    
    // 4. Create metadata record
    let artifact = Artifact(
        id: UUID(),
        name: name,
        runID: run.id,
        path: finalPath,
        mimeType: mimeType,
        contentHash: SHA256(content),
        createdAt: Date()
    )
    
    return artifact
}
```

## Example 8: Agent Catalog with Backend Profiles

```yaml
backend_profiles:
  claude_standard:
    provider: claude
    model: claude-3-5-sonnet-20241022
    effort: standard
    temperature: 0.5
    max_turns: 10

  codex_experimental:
    provider: codex
    model: codex-latest
    effort: experimental
    temperature: 0.7
    max_turns: 20

agents:
  proposal_writer:
    backend_profile: claude_standard
    prompt: |
      You are a proposal writer. Create a detailed proposal for the idea.
      Use professional language and clear structure.
      Output MUST be valid markdown.
    inputs: [idea_brief]
    outputs: [proposal_current]
    permission_profile: allow_once

  code_reviewer:
    backend_profile: codex_experimental
    prompt: |
      You are a code reviewer. Analyze the implementation and provide feedback.
      Consider: functionality, performance, security, testing, documentation.
      Output MUST be JSON with fields: status, issues, recommendations.
    inputs: [implementation_code, implementation_tests]
    outputs: [code_review_report]
    permission_profile: require_confirmation
```

## Example 9: Transition with Complex Logic

```yaml
transitions:
  - to: approved_path
    when: |
      (artifact.review_score >= 8.5) AND
      (artifact.security_status == "pass") AND
      (artifact.test_coverage >= 0.8) AND
      (
        exists('final_approval') OR 
        vars.auto_approve == true
      )
  
  - to: revision_needed
    when: |
      (artifact.review_score < 8.5) AND
      (vars.revision_count < vars.max_revisions)
  
  - to: failed
    when: |
      (vars.revision_count >= vars.max_revisions) OR
      (artifact.security_status == "fail")
```

## Example 10: Test Pattern

```swift
// In RunPlanCompilerTests.swift
@Test func testCompileMinimalWorkflow() throws {
    // 1. Setup
    let workflowPath = "examples/workflows/minimal.yaml"
    let catalogPath = "examples/agents/agents.yaml"
    let compiler = RunPlanCompiler()
    
    // 2. Execute
    let plan = try compiler.previewCompile(
        workflowPath: workflowPath,
        catalogPath: catalogPath
    )
    
    // 3. Assert
    #expect(plan.initialStateID == "propose")
    #expect(plan.states.count == 3)
    #expect(plan.agentBindings["proposal_writer"] != nil)
    
    // 4. Verify transitions
    let proposeState = try #require(plan.states["propose"])
    let transitions = proposeState.transitions
    #expect(transitions.count == 1)
    #expect(transitions.first?.toStateID == "approve")
}
```
