# Workspace Isolation Risk (Goose Backend)

## Context

Chainworks uses Goose as an execution substrate for multi-agent workflows.
Each Run operates on a project workspace with multiple agents performing read/write and side-effect operations.

During testing, a critical issue was observed:
When multiple sessions (projects) run concurrently, working directories may leak between sessions,
causing agents to operate on incorrect files or mix project contexts.

This document formalizes the risk, its root causes, and required architectural constraints.

---

## Problem Summary

Observed behavior:
- Agents from Project A start accessing or modifying files from Project B
- Shell commands execute in an unexpected directory
- Requirements and artifacts from different runs become mixed

This is NOT a prompt-level issue.
This is a runtime/session isolation problem.

---

## Root Cause (Hypothesis)

Based on Goose behavior and known issues:

1. Working directory is not strictly bound to session context
2. Some tools/extensions rely on process-level `cwd`
3. Shared backend process may reuse mutable global state
4. Session metadata (`working_dir`) is not always respected by tool execution

Result:
-> "current directory" becomes a hidden global state
-> multiple runs interfere with each other

---

## Risk Level

**Severity: Critical**

Impact:
- Corrupted artifacts
- Incorrect commits / pushes
- Cross-project contamination
- Loss of determinism
- Broken trust in the system

---

## Design Principle

> No agent operates in an implicit environment.

Everything must be explicit:
- workspace
- repo
- artifact location
- permissions

---

## Required Architecture Constraints

### 1. Run = Isolation Boundary

Each Run must define:

```text
RunWorkspace {
  run_id
  project_id
  workspace_root (absolute path)
  artifact_root
}
```

This is the ONLY valid execution context.

---

### 2. No Global Working Directory

FORBIDDEN:
- relying on process `cwd`
- implicit filesystem context

REQUIRED:
- every operation receives `workspace_root` explicitly

---

### 3. One Run = One Session Context

Options:
- separate backend process per run (preferred)
- or strict session containerization

Never share mutable runtime state between active runs.

---

### 4. Controlled Side Effects (MCP / Services)

All side effects must go through controlled interfaces:

- git commit/push
- archive build
- Connect distribution
- documentation updates

Each must require:

```text
target_workspace: absolute_path
```

No generic shell execution without context.

---

### 5. Worktree Strategy

- Writers -> dedicated writable workspace
- Reviewers -> read-only snapshot
- No concurrent write agents in same workspace

---

### 6. Snapshot Before Execution

Before Run starts:

```text
RunPlanSnapshot {
  workflow_version
  agent_catalog_version
  backend_profiles
  workspace_root
}
```

Execution MUST use snapshot, not live config.

---

### 7. Runtime Guardrails

Before ANY filesystem action:

```text
if path not under workspace_root:
    block execution
    mark run as "blocked"
```

---

### 8. Resume Policy

Auto-resume allowed ONLY for:
- pure compute stages
- read-only stages

NEVER auto-resume:
- git operations
- distribution
- external side effects

---

## Failure Modes (What This Prevents)

Without this design:
- Agent commits code to wrong repo
- Proposal review reads wrong project files
- Logs reference inconsistent state
- Debugging becomes impossible

With this design:
- Each Run is deterministic
- Cross-project contamination is impossible
- Debugging is local and traceable

---

## Final Rule

> There is no "current directory" in Chainworks.

Only:

```text
RunWorkspace(id, absolute_path)
```

Everything else is a bug.

---

## Status

- Risk identified: yes
- Mitigation defined: yes
- Implementation required: yes
