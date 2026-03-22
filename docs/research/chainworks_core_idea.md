# Chainworks — core idea

> This document captures product vision and positioning.
> It is not a Problem Statement and not an MVP scope doc.
> Implementation scope lives in `/Users/user/Documents/Chainworks Forge/docs/ps/chainworks-forge-mvp.md`.

## In one sentence

**Chainworks** is a local control plane for agent-driven engineering work: a product that moves an idea through proposal, review, implementation, audit, and release using specialized agents, explicit workflows, durable artifacts, and hard quality gates.

## What problem the product solves

Most AI development tools break down in one of three ways:

1. **One universal agent for everything.** It can draft, code, review, and release, but responsibility becomes blurred and quality becomes unstable.
2. **A pile of chats and IDE assistants.** Individual steps work, but the process loses structure: it is unclear which artifact is current, who decided what, and why code diverged from intent.
3. **Heavy orchestration systems.** They are powerful, but they force the user to build an operating model first: memory, permissions, retries, queues, observability, and execution boundaries.

**Chainworks** sits in the gap between chat-first AI tooling and heavyweight orchestration infrastructure.

## Core product thesis

The product should think in terms of **stages, roles, artifacts, and gates**, not in terms of chat turns.

A user brings an idea. The system then:

- turns it into a proposal,
- runs independent reviews,
- forces the proposal to mature,
- starts implementation,
- checks implementation against the approved proposal,
- blocks release until required gates are passed,
- records the outcome as durable artifacts.

That means **Chainworks manages engineering execution**, not just AI conversation.

## The primary object in the system

The primary object is not a chat session and not a single agent.
The primary object is a **Run**.

A Run is one end-to-end workflow execution from idea to decision.

Each Run contains:

- the original idea,
- the selected workflow,
- the current stage,
- active and completed agents,
- artifacts produced at each stage,
- review findings and unresolved issues,
- approvals and release gates,
- execution history, costs, and logs,
- final outcome.

This is important because the product must feel operational, not conversational.

## What Chainworks orchestrates

### 1. Roles

Agents are intentionally specialized. Typical roles include:

- proposal writer,
- PO reviewer,
- UX reviewer,
- UI reviewer,
- architect reviewer,
- coder,
- proposal-vs-implementation auditor,
- security checker,
- pre-push reviewer,
- git/release agent,
- docs guardian,
- lead/orchestrator.

The key idea is that each role owns a narrow responsibility. No single agent should implicitly become product manager, architect, implementer, reviewer, and releaser at the same time.

### 2. Policy

Each role is bound to explicit execution policy:

- provider / model family,
- reasoning effort,
- skill or prompt package,
- allowed tools,
- filesystem and network permissions,
- read-only vs write-capable execution mode,
- input and output contract.

So the system knows not only **who** is running, but also **how** that role is allowed to work.

### 3. Workflow

Execution is an explicit state machine, not an improvised conversation.

A typical flow is:

- Idea received
- Proposal drafted
- Proposal reviewed
- Proposal refined
- Implementation started
- Implementation refined
- Implementation audited
- Final review / security check
- Manual release decision
- Workflow complete

The important part is not the exact names. The important part is that the workflow is explicit, inspectable, and repeatable.

### 4. Artifacts

The system is artifact-first.

Important artifacts include:

- `idea.md`,
- `proposal.md`,
- independent review reports,
- aggregated review result,
- implementation notes,
- implementation audit,
- security report,
- pre-push review,
- release manifest,
- documentation update report.

If a stage produces no durable artifact, it becomes hard to verify, replay, or audit.

## Why multiple models matter

Different roles benefit from different model strengths:

- structural reasoning,
- precise code editing,
- long architectural analysis,
- UX critique,
- document production,
- low-cost routine execution.

Because of that, **Chainworks is multi-backend by design**. The product should be able to bind a role to the provider that best fits that class of work, while keeping one control plane and one workflow model.

## Why YAML matters

Agent and workflow definitions should be declarative.

YAML is useful here because it gives the product:

1. a human-readable source of truth for roles and workflows,
2. versioned configuration in git,
3. separation between control-plane policy and runtime implementation,
4. a compile target into normalized runtime objects.

In practice, YAML is not just config. It is a DSL for describing how agent work is organized.

## What the user should feel

The user should not feel that they “just chatted with AI”.

They should feel something closer to this:

> I started a run. The system routed the work through the right specialists, forced the proposal to mature, kept the artifacts organized, surfaced disagreements clearly, and would not let the process move into release without the required checks and approvals.

Emotionally, this is closer to a mix of:

- IDE,
- review board,
- build/release pipeline,
- project control console.

## What the product must not become

To preserve the idea, Chainworks must not become:

### A chat-first AI client

The center of the product is not dialogue. It is run state, workflow progress, artifacts, findings, and approvals.

### A black box

The user must always be able to answer:

- which agent acted,
- what it produced,
- why the workflow is blocked,
- which gate failed,
- what changed since the previous stage.

### An “autonomous company in a box” fantasy

Important side effects must stay behind explicit gates. Release is a governed transition, not an implicit continuation.

### A system with blurry permissions

Review roles should default to read-only. Write-capable agents should work in isolated execution contexts. Release-capable agents should have the narrowest possible permission profile.

## Architectural principles

The architecture should stay anchored to a few product principles:

1. **Local-first control plane** — the system should feel close to the repo, files, logs, and runtime.
2. **Roles over universality** — a set of narrow, well-bounded agents is preferable to one overly broad agent.
3. **Artifacts over chat history** — durable outputs are required for quality control.
4. **Explicit gates over implicit trust** — approvals and release transitions must be visible and auditable.
5. **Least privilege by default** — permissions are granted because a role needs them, not because it is convenient.
6. **Isolated execution for writers** — write-capable agents should not share one uncontrolled workspace.
7. **Replaceable runtime** — the runtime may evolve, but the product model should remain stable.

## Consequences for the MVP

This vision does **not** mean the MVP must implement the whole future platform.

The first slice should prove the operating model, not every feature idea.

That is why the MVP in `/Users/user/Documents/Chainworks Forge/docs/ps/chainworks-forge-mvp.md` focuses on:

- a local SwiftUI control plane,
- YAML-defined workflows,
- one active run per idea,
- visible stage progression,
- agent inspection,
- explicit approval gates,
- durable run state in SwiftData,
- multi-provider execution limited to Codex and Claude Code,
- readable completion reports.

Additional providers such as Gemini belong to post-MVP provider adapter extensions, not to the first implementation slice.

In other words, the MVP validates the product thesis that **workflow clarity + role specialization + artifact discipline** produce a better engineering experience than ad hoc chat orchestration.

## Related documents

- Product / MVP scope: `/Users/user/Documents/Chainworks Forge/docs/ps/chainworks-forge-mvp.md`
- Architecture research: `/Users/user/Documents/Chainworks Forge/docs/research/goose_swiftui_agent_architecture_research.md`
