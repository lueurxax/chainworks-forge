# Research: Goose + SwiftUI for Orchestrating an Agent Team

**Snapshot date:** 2026-03-21  
**Context:** a local macOS app built with SwiftUI, many specialized agents, multiple LLM backends (Codex / Claude / Gemini), a custom YAML format for agent descriptions, and a possible later evolution into `SwiftUI client + Rust backend + Temporal`.

---

## 0) Document and example structure

This repository layout is now split so that long-form documents and runnable examples do not live inside one large Markdown file:

```text
docs/
  README.md
  research/
    goose_swiftui_agent_architecture_research.md

examples/
  README.md
  agents/
    proposal-po-reviewer.yaml
  workflows/
    proposal-to-release.yaml
```

Rule of thumb:

- `docs/` is for reasoning, architecture, tradeoffs, and reference material.
- `examples/` is for concrete YAML artifacts that can later become fixtures, templates, or test inputs.

---

## 1) Conclusion in two paragraphs

If I were taking your stack seriously, I would build a **custom control plane** and use **Goose as the execution substrate**, not as the place where all orchestration business logic lives. Goose already gives you exactly what you need to avoid writing a messy runtime from scratch: a documented path for a **custom client through `goose-server` / `goosed` over REST + SSE**, recipe mechanics, structured output via JSON schema, retry, sub-recipes, skills through Summon, permission modes, and tool restrictions.[^goose-custom][^goose-recipe][^goose-skills][^goose-perm]

For your first release, I would **not use ACP as the main transport**. Goose documents ACP as a separate integration path, but it also marks it as **experimental**. For a SwiftUI client, a REST/SSE loop through `goosed` is much calmer: token and event streaming is simpler, debugging is simpler, and approval gates are easier to keep under control. I would leave ACP for phase two, when you actually need editor-style scenarios with unsaved buffers, native diffs, and tight IDE integration.[^goose-custom][^goose-acp]

---

## 2) Which reference implementations to anchor on

This is not "what to install". It is **what exactly is worth borrowing** from each project.

| Reference | What to borrow | Why it matters for you | What not to copy |
|---|---|---|---|
| **Goose** | `goosed` as the local runtime, REST/SSE custom UI path, recipe schema (`settings`, `response`, `retry`, `sub_recipes`), headless/scheduled execution, permissions, Summon `load` / `delegate` | This gives you a ready execution layer and event stream without inventing yet another homemade daemon | Do not tie your whole orchestration model to Goose's internal "autonomous" behavior |
| **Codex** | the idea of **custom agents**, `model_reasoning_effort`, skills as directories with `SKILL.md`, worktree-first execution, app-server as an example of a client/server contract | A very strong reference for your **own YAML DSL** and for isolating writer tasks | Do not build the control plane as an OpenAI-only system if you want multi-backend from day one |
| **Zed ACP** | ACP as an adapter layer, registry/config for external agents, and the practice of keeping "editor separate from runtime" | Useful as a reference for a future ACP adapter or plugin mode | Do not use Zed as the main team orchestrator; ACP is useful there, but not as your primary workflow engine |
| **Continue** | the pattern of "a narrow agent/check for one role", checks/agents as markdown with YAML frontmatter, an event model for PR/CI/scheduled flows | A strong reference for review/security/docs roles and for avoiding the anti-pattern of "one giant reviewer for everything" | Do not collapse your whole system into PR checks; you need a broader control plane |
| **MCP spec** | separation into `tools`, `resources`, and `prompts`, plus local `stdio` and remote HTTP transport | This is the right contract for deterministic integrations in Go/Rust (GitHub, Connect, archives, scanners) | Do not treat MCP Tasks as the durability foundation of the whole system; Tasks are still experimental |
| **Temporal** | parent workflow, activities vs child workflows, task queues, schedules, retry policies, worker versioning, Continue-As-New | A strong reference for **phase two**, once the control layer is mature and you need durability | Do not drag Temporal into the first iteration before stages and artifacts are stable |

### Brief notes on each

**Goose**  
The official Goose docs explicitly describe two custom-interface paths:
1. run `goosed` and talk to it over REST/SSE;
2. integrate through ACP over stdio JSON-RPC.  
There is also a recipe reference with `goose_provider`, `goose_model`, `response.json_schema`, `retry`, and `sub_recipes`, plus headless execution, schedules, and permission modes.[^goose-custom][^goose-recipe][^goose-headless][^goose-schedule]

**Codex**  
Codex is a strong reference specifically for **agent description structure**: custom agents, inherited defaults, `model_reasoning_effort`, skills as separate directories with `SKILL.md`, worktree isolation, approvals, and sandboxing. Even if Codex is not your main runtime, it is very useful as a **DSL and UX pattern reference**.[^codex-subagents][^codex-skills][^codex-worktrees][^codex-approvals][^codex-appserver]

**Zed ACP**  
This is not your final orchestrator, but it is a good reference for **how to build an external agent bridge**: Zed supports external ACP agents, standalone agent-server extensions, and registration/debugging configs. That is useful as a mental model if you later want a plugin mode or editor integration.[^zed-acp][^zed-external][^zed-agent-server]

**Continue**  
Continue shows well how to think about review/security/docs agents: **one check, one role, one focus**, not a "universal judge of everything". For PR/CI-like stages, it is a very sane reference for format and responsibility boundaries.[^continue-checks][^continue-best][^continue-gh]

**MCP**  
MCP matters not as a trendy term, but as a way to **separate an LLM agent from a deterministic engineering operation**. Git push, upload to Connect, build archive, vulnerability scan, release tagging: those are all good candidates for standalone MCP services in Go/Rust.[^mcp-basic][^mcp-tools][^mcp-resources]

**Temporal**  
I would treat Temporal as a second-stage move, once you already have stable stage contracts and artifact schemas. Its mental model matters from the start: **activities retry by default**, child workflows should not be created "just to organize code", workflow history must stay under control, and long-lived processes eventually need Continue-As-New and worker versioning.[^temporal-workflows][^temporal-child][^temporal-retry][^temporal-worker]

---

## 3) Recommended v1 architecture: SwiftUI control plane + Goose runtime

### 3.1. Primary decision

**Do not make Goose your source of truth.**  
The source of truth should belong to you:

- your YAML catalog of agents and workflows,
- your store for jobs, approvals, and artifacts,
- your worktree manager,
- your deterministic MCP services.

In this model, **Goose** is responsible for:

- LLM runtime,
- tool usage,
- session lifecycle,
- token and event streaming,
- recipe/subrecipe execution,
- structured outputs,
- permission limiting.

### 3.2. Component diagram

```text
┌──────────────────────────── SwiftUI macOS App ────────────────────────────┐
│ Jobs / Runs / Approvals / Live logs / Artifacts / Agent catalog editor    │
└───────────────────────────────┬───────────────────────────────────────────┘
                                │
                                ▼
┌──────────────────────── Local Orchestrator Layer ─────────────────────────┐
│ YAML loader/validator                                                     │
│ Workflow compiler                                                         │
│ Worktree manager                                                          │
│ Artifact bus (JSON + Markdown)                                            │
│ Approval gate                                                             │
│ Goose session adapter (REST/SSE)                                          │
│ Provider/effort adapter                                                   │
└───────────────┬───────────────────────────────┬────────────────────────────┘
                │                               │
                ▼                               ▼
      ┌─────────────────────┐        ┌─────────────────────────────┐
      │      goosed         │        │ Deterministic MCP services  │
      │ sessions/messages   │        │ GitHub / Connect / Build    │
      │ recipes / tools     │        │ Archive / Scanners          │
      └─────────┬───────────┘        └──────────────┬──────────────┘
                │                                    │
                ▼                                    ▼
      ┌─────────────────────┐             ┌─────────────────────────┐
      │ Claude / Codex /    │             │ Git, artifact store,    │
      │ Gemini backends     │             │ external systems        │
      └─────────────────────┘             └─────────────────────────┘
```

### 3.3. Why this is the right shape

1. **SwiftUI** stays UI plus control plane instead of turning into a hand-rolled process manager.
2. **Goose** already gives you a session model and event stream through `goosed`. For SwiftUI this is close to an ideal minimal runtime: open a session, send a message, read SSE, update the UI.[^goose-custom]
3. **MCP services** in Go/Rust isolate the most dangerous side effects: GitHub, build/archive, Connect, security scanners.
4. **Your YAML DSL** remains your public contract. That lets you swap executors later: Goose first, then your own Rust backend, then Temporal, without rewriting half the product logic.

---

## 4) Why REST/SSE through `goosed` is better than ACP in phase one

### Decision

For the first release, I recommend:

- **primary runtime transport:** `goosed` + REST/SSE
- **ACP:** keep it, but only as a future adapter

### Selection table

| Criterion | `goosed` REST/SSE | ACP |
|---|---|---|
| Integration path for custom UI in Goose | Officially documented | Also documented |
| Maturity for your use case | Better suited as the base transport for a standalone desktop client | Marked experimental by Goose |
| SwiftUI complexity | Low: `URLSession`, event streams | Higher: stdio/JSON-RPC bridge, lifecycle, framing |
| Unsaved buffers / native diffs | No | Yes |
| Fit for your first release | **Yes** | Later |

### Practical conclusion

If you are building **a standalone macOS orchestration app**, not an editor plugin, ACP only adds dust at the start: stdio bridging, state machines, JSON-RPC transport, process lifecycle handling, reconnect logic. Its main benefit, access to unsaved buffers and native editor diffs, is secondary for you right now.[^goose-custom][^goose-acp]

---

## 5) Your YAML must be the main API, not Goose recipe schema

This is probably the most important architecture recommendation in the document.

### Why

Goose recipes already support many useful things:

- `settings.goose_provider`
- `settings.goose_model`
- `response.json_schema`
- `retry`
- `sub_recipes`[^goose-recipe]

But you also have your own product-level requirements:

- a unified `backend` abstraction (`codex`, `claude`, `gemini`, `deterministic-mcp`)
- `effort` as a first-class field
- a reference to a **skill**
- access rights and permission profiles
- file roots and write globs
- artifact ownership
- a human-readable prompt
- future portability into a Rust backend / Temporal

Goose recipe schema **must not** become your product API. It should be a **compilation target schema**.

### Recommended rule

- **Your YAML** = product truth
- **Goose recipe** = runtime artifact compiled on demand by your orchestrator

---

## 6) Proposed YAML DSL

The examples are now separated into standalone files:

- Agent example: [../../examples/agents/proposal-po-reviewer.yaml](../../examples/agents/proposal-po-reviewer.yaml)
- Workflow example: [../../examples/workflows/proposal-to-release.yaml](../../examples/workflows/proposal-to-release.yaml)

Keep the rule strict:

- agent examples live in `examples/agents/`
- workflow examples live in `examples/workflows/`

### 6.1. Agent spec

The agent example above captures the minimum practical shape:

- `agent.id`, `agent.title`, `agent.business_mode`
- `backend.runtime`, `backend.provider`, `backend.model`, `backend.effort`
- `skill.id` and `skill.load`
- `prompt`
- `permissions`
- `output`
- `retry`

### 6.2. Workflow spec

The workflow example above captures the minimum practical shape:

- `workflow.id`, `workflow.title`
- ordered `stages`
- `type` per stage (`single` or `fanout`)
- `agent` or `agents`
- `needs`
- `gate`
- `approval`

### 6.3. How to compile this into Goose runtime

Your compiler should do the following:

1. Use `backend.provider` to set Goose recipe `settings.goose_provider` and `settings.goose_model`.[^goose-recipe]
2. Use `skill.id` to prepare a skill-loading step through Summon `load` or an equivalent bootstrap context, because Goose supports skills via Summon, but your DSL must stay runtime-agnostic.[^goose-skills][^goose-v125]
3. Use `output.schema` to materialize `response.json_schema` so agent handoff happens through JSON rather than vague markdown.[^goose-recipe]
4. Use `permissions` to assemble tool profiles, `.gooseignore`, allowlists, and approval policy.[^goose-perm][^goose-gooseignore][^goose-allowlist]
5. Use `effort` through **your provider adapter**.

### 6.4. The key caveat about `effort`

Goose recipes have explicit fields for provider/model/max_turns, but there is no universal cross-provider `effort` field.  
Therefore:

- `effort` should be **your** field;
- your orchestrator should map it in a **provider-specific** way;
- where a native knob exists, use it;
- where it does not, interpret `effort` through model choice, planning depth, `max_turns`, retry policy, mandatory planner/lead phase, and so on.

Codex is a useful reference here because `model_reasoning_effort` is an explicit part of agent configuration.[^codex-subagents][^codex-config]

My recommendation is to keep exactly four levels:

- `low`
- `medium`
- `high`
- `critical`

Then let the backend adapter decide how those map into Codex / Claude / Gemini.

---

## 7) How to organize the filesystem without losing your mind

This is where systems like this most often break down.

### 7.1. Hard rules

1. **Every write-capable agent works in its own git worktree.**  
   Not "in the shared project folder". Not "one after another". A separate worktree.

2. **Review agents should not read a live writer worktree directly unless necessary.**  
   They should read either the base repository, or a snapshot / diff / artifacts.

3. **Release agents must not edit code at all.**  
   They work with a prepared tree state and deterministic services.

4. **Skills and prompts must be separate from job state.**

5. **Handoff artifacts must always be dual-output:**  
   - human-readable markdown  
   - machine-readable JSON validated by schema

### 7.2. Recommended orchestrator directory structure

```text
.orchestrator/
  agents/
    lead-orchestrator.yaml
    proposal-po-reviewer.yaml
    proposal-ux-reviewer.yaml
    proposal-ui-reviewer.yaml
    proposal-architect-reviewer.yaml
    code-writer.yaml
    proposal-writer.yaml
    proposal-implementation-audit.yaml
    security-checker.yaml
    github-commit-push.yaml
    prepush-code-review.yaml
    connect-publisher.yaml
    docs-guardian.yaml

  workflows/
    proposal-to-release.yaml

  schemas/
    proposal_review_v1.json
    implementation_audit_v1.json
    security_findings_v1.json
    prepush_review_v1.json
    release_manifest_v1.json

  skills/
    proposal-review-triad/
    proposal-implementation-audit/
    docs-quality-guardian/

  jobs/
    jobs.sqlite

  worktrees/
    job-001/
      code-writer/
      proposal-writer/
      docs-guardian/
      release/

  artifacts/
    job-001/
      plan/
      reviews/
      audits/
      security/
      docs/
      release/
```

### 7.3. What else you should do immediately

- Generate `.gooseignore` automatically for each job/worktree.
- Keep a **hard deny list** on top of that for `.env`, `.env.*`, and `secrets.*`. Goose protects them by default if no `.gooseignore` file exists, so if you start generating your own ignore files, make sure to add those patterns back.[^goose-gooseignore]
- Keep a separate allowlist for shell commands through `GOOSE_ALLOWLIST`, especially for build/release/security stages.[^goose-allowlist]
- Do not overinflate the tool surface. Goose itself recommends keeping the active tool set modest; more than about 25 tools in one session is already a bad idea.[^goose-toolperm]

### 7.4. Worktree policy

For your use case, the rule is simple:

- **Lead**: no worktree, only reads and writes artifacts
- **Review / audit / security**: read-only, no mutable worktree of their own
- **Code writer**: separate worktree
- **Proposal writer**: separate worktree or doc-only sandbox
- **Docs guardian**: separate worktree or doc-only workspace
- **Commit/push**: separate release worktree
- **Build/publish**: separate release worktree

It is useful to look both at Goose's tutorial on isolated development environments and at Codex worktrees as a reference for isolating parallel tasks.[^goose-isolated][^codex-worktrees]

---

## 8) What communication between agents should look like

### Main rule

**Do not let agents talk to each other through free-form text.**  
That quickly turns into a swamp of long dialogues where nobody can tell which version of truth is current.

### Use an artifact bus instead

Each agent should write only:

1. a **machine-readable report** validated by schema
2. a **human summary** in markdown
3. an **optional patch manifest** if it changed something
4. an **approval request** if it reached a manual-confirmation threshold

### Handoff formats

Example schema names:

- `proposal_review_v1`
- `implementation_audit_v1`
- `security_findings_v1`
- `prepush_review_v1`
- `docs_guardian_report_v1`
- `release_manifest_v1`

### The role of the lead

The lead agent should do only four things:

1. plan stage order;
2. trigger fan-out / fan-in;
3. read artifacts and decide who runs next;
4. raise human approval where side effects exist.

**The lead must not:**

- write code,
- push to GitHub,
- edit documentation,
- build archives.

In other words, the lead is a **router and transition judge**, not a universal executor.

### Why this matters especially here

Goose supports subagents/subrecipes, but if you are building **your own external orchestrator**, it is better not to rely on internal "automatic teamwork" as the main mechanism. External orchestration gives you:

- approvals,
- separate access rights,
- deterministic artifacts,
- understandable tracing,
- a cleaner future migration to Temporal.[^goose-subrecipes][^goose-parallel]

---

## 9) Weak points and how to handle them

| Weak point | Why it will hurt | What to do |
|---|---|---|
| **Making ACP the primary transport too early** | You will drown in transport glue and lifecycle work before the product model is stable | Start with `goosed` REST/SSE; add ACP later |
| **Treating Goose recipe schema as your API** | Migration into Rust backend/Temporal becomes painful later | Keep your own YAML DSL and compile into runtime targets |
| **One shared workspace for writer agents** | File conflicts, unclear ownership, races, false regressions | One writer = one worktree |
| **Too broad a tool surface** | The agent gets noisy, makes worse decisions, and reaches where it should not | Minimal access profiles; do not overexpand tools |
| **Free-text handoff between agents** | You cannot validate transition quality automatically | Only JSON schema + markdown summary |
| **LLM performs commit/push/upload directly through a shared shell** | This is the ugliest class of failures: expensive side effects and repository damage | Move GitHub/Connect/build operations into deterministic MCP services |
| **Secrets leak into worktrees and context** | Leaks, prompt injection, accidental publication | `.gooseignore`, deny globs, secrets only through env/MCP boundaries |
| **One "reviewer for everything"** | Too much noise, too little accountability, hard to compare outputs | Split review agents into narrow roles; Continue checks reinforce this pattern |
| **Temporal introduced too early** | You will model durability before you understand the actual task lifecycle | Temporal comes after stage contracts are stable |
| **Thinking about Temporal history and payload size too late** | Once workflow volume grows, history bloats and debugging becomes unpleasant | Claim-check pattern, small payloads, Continue-As-New, worker versioning |

### My strongest recommendations

1. **Agent 9 and Agent 11 must not be "just another LLM with shell access".**  
   They are best implemented as separate MCP services in Go/Rust.

2. **The lead must not have write access to code.**

3. **You cannot pass `effort` into Goose "as is".**  
   That belongs in your adapter layer.

4. **Any stage that can trigger an external side effect must pass through an approval gate.**

5. **Security and audit reports should always require structured output.**

---

## 10) Mapping your 13 agents by mode, backend, effort, and permissions

Below I am assuming **12 specialized agents + 1 lead**.

### 10.1. Access profile legend

- `RO_REVIEW`: read-only access to repository and artifacts, no shell write, no git, no network
- `RO_VERIFY`: read access plus limited analysis/test/grep commands, but no code writes
- `DOC_WRITE`: write access only to `docs/**`, `proposals/**`, `*.md`, no git push
- `CODE_WRITE`: write access to `src/**`, `tests/**`, selected configs, only inside a dedicated worktree, no push
- `RELEASE_GIT`: commit/tag/push through deterministic GitHub MCP, no arbitrary source edits
- `RELEASE_PUBLISH`: build/archive/upload through deterministic services, no source edits
- `ORCH`: read everything, write only into `artifacts/**`, no code writes and no release side effects

### 10.2. Agent matrix

| Agent | Business mode | Backend | Effort | Skill | Goose permission mode | Access profile | Practical purpose |
|---|---|---|---|---|---|---|---|
| **Lead / Orchestrator** | orchestration | `claude-acp` | `high` | `orchestrator-core` | `chat` | `ORCH` | Plans stages, reads artifacts, triggers fan-out/fan-in, raises approvals |
| **1. Proposal reviewer (PO)** | proposal_review | `claude-acp` | `high` | `proposal-review-triad/po` | `chat` | `RO_REVIEW` | Business value, scope, acceptance criteria, prioritization |
| **2. Proposal reviewer (UX)** | proposal_review | `gemini-acp` | `medium` | `proposal-review-triad/ux` | `chat` | `RO_REVIEW` | User flow, friction, scenarios, edge cases |
| **3. Proposal reviewer (UI)** | proposal_review | `gemini-acp` | `medium` | `proposal-review-triad/ui` | `chat` | `RO_REVIEW` | Screen-level consistency, states, visual contracts |
| **4. Proposal reviewer (Architect)** | proposal_review | `claude-acp` | `high` | `proposal-review-triad/architect` | `chat` | `RO_REVIEW` | Architecture, risks, dependencies, data boundaries |
| **5. Code writer** | implementation | `codex-acp` | `high` | `code-writer/core` | `smart_approve` | `CODE_WRITE` | Writes code and tests in a dedicated worktree |
| **6. Proposal writer** | proposal_authoring | `claude-acp` | `high` | `proposal-writer/core` | `smart_approve` | `DOC_WRITE` | Rewrites/normalizes the proposal after review fan-in |
| **7. Proposal vs implementation audit** | audit | `codex-acp` | `high` | `proposal-implementation-audit` | `approve` | `RO_VERIFY` | Compares code against proposal / acceptance criteria |
| **8. Security checker** | security | `claude-acp` | `high` | `security-checker/core` | `approve` | `RO_VERIFY` | Looks for security issues, unsafe defaults, secret exposure; may run limited scanners |
| **9. Commit & push to GitHub** | release_git | `github-mcp` (+ optional `codex-acp` for commit message) | `low` | `github-commit-push` | `approve` | `RELEASE_GIT` | Performs commit/tag/push only through a deterministic service |
| **10. Pre-push code review** | prepush_review | `claude-acp` | `medium` | `prepush-review/core` | `approve` | `RO_VERIFY` | Final barrier before push: readability, risks, test coverage, obvious regressions |
| **11. Build archive & push to Connect** | release_publish | `build-mcp` + `connect-mcp` | `low` | `connect-publisher` | `approve` | `RELEASE_PUBLISH` | Builds the archive and publishes it to Connect through a deterministic pipeline |
| **12. Docs guardian** | docs | `claude-acp` | `medium` | `docs-quality-guardian` | `smart_approve` | `DOC_WRITE` | Maintains README, changelog, docs consistency, usage notes |

### 10.3. Why this distribution makes sense

- I would keep **Claude** on roles where composition, feedback phrasing, architecture reasoning, and policy reasoning matter most.
- I would keep **Codex** on roles that need confident code navigation, patch thinking, and implementation/audit.
- I would keep **Gemini** on UX/UI review roles as one of the external design-oriented reviewers, to avoid monoculture and get a different angle of critique.
- I would not let a free-form LLM agent handle **GitHub push** and **Connect publish** at all: only deterministic services.

### 10.4. What I would change only later

Two variants are worth testing later:

1. Enable a Goose **lead/worker** scheme for the **Code writer**:  
   `lead = claude-acp`, `worker = codex-acp`, if you actually see better outcomes from separating planning and implementation.[^goose-leadworker]

2. Split **Security checker** into two agents:  
   - static scanner wrapper  
   - policy/security reviewer  
   But that is not phase one.

---

## 11) How I would define permission profiles

### 11.1. `RO_REVIEW`

```yaml
profile: RO_REVIEW
filesystem:
  read_roots:
    - repo_root
    - artifact_root
  write_globs:
    - artifacts/**
tools:
  - developer.read
  - developer.search
network: none
git:
  commit: false
  push: false
```

### 11.2. `CODE_WRITE`

```yaml
profile: CODE_WRITE
filesystem:
  read_roots:
    - worktree_root
    - artifact_root
  write_globs:
    - src/**
    - tests/**
    - configs/selected/**
    - artifacts/**
tools:
  - developer.read
  - developer.edit
  - developer.search
  - developer.terminal_limited
network: restricted
git:
  commit: false
  push: false
```

### 11.3. `RELEASE_GIT`

```yaml
profile: RELEASE_GIT
filesystem:
  read_roots:
    - release_worktree
    - artifact_root
  write_globs:
    - artifacts/release/**
tools:
  - github_mcp.commit
  - github_mcp.push
  - github_mcp.tag
network:
  allow:
    - github.com
git:
  commit: true
  push: true
shell: none
```

### 11.4. `RELEASE_PUBLISH`

```yaml
profile: RELEASE_PUBLISH
filesystem:
  read_roots:
    - release_worktree
    - artifact_root
  write_globs:
    - artifacts/release/**
tools:
  - build_mcp.archive
  - connect_mcp.upload
network:
  allow:
    - connect.internal
shell: none
git:
  commit: false
  push: false
```

---

## 12) What to use from Goose directly, and what not to use as the foundation

### Use directly

- `goosed` + REST/SSE custom UI path[^goose-custom]
- recipes as a target schema[^goose-recipe]
- `response.json_schema`[^goose-recipe]
- `retry`[^goose-recipe]
- `sub_recipes` and, if needed, parallel fan-out as a reference[^goose-recipe][^goose-parallel]
- headless `goose run` / schedules for automation later[^goose-headless][^goose-schedule]
- skills through Summon `load`[^goose-skills][^goose-v125]
- permission modes, tool permissions, `.gooseignore`, allowlist[^goose-perm][^goose-toolperm][^goose-gooseignore][^goose-allowlist]

### Do not use as architectural foundation

- ACP as the **primary** transport in phase one[^goose-acp]
- internal "autonomous team formation" as a substitute for your orchestrator
- recipe schema as your public API
- a shared shell for release/publish side effects

---

## 13) Evolution into Rust backend + Temporal

When to move: **not immediately**. First, these things should stabilize:

- the list of stages;
- artifact schemas;
- access profiles;
- retries / approvals;
- worktree policy.

Once those are stable, you can move orchestration into a Rust backend and add Temporal.

### 13.1. What that would look like

```text
SwiftUI App
   │
   ▼
Rust Orchestrator API
   │
   ├── Temporal client
   │     ├── Parent workflow: one user job / one feature run
   │     ├── Child workflow: proposal review fanout
   │     ├── Activities: codegen, audit, scanners, git, build, upload
   │     └── Signals/Updates: approvals, cancel, re-run stage
   │
   ├── Goose adapter
   ├── Worktree manager
   ├── Artifact store
   └── MCP service clients
```

### 13.2. My recommended mapping model in Temporal

- **Parent workflow** = one job / one feature run
- **Activities** = actual agent session calls, shell/MCP side effects, artifact analysis
- **Child workflows** = only where you truly need a separate durable lifecycle  
  For example:
  - proposal review fan-out
  - release pipeline
  - recurring docs/security maintenance

Temporal explicitly says child workflows should not be used merely as a way to "organize code"; when in doubt, an activity is often the better choice.[^temporal-child]

### 13.3. What to pay attention to early

- Activities retry by default, which is very convenient for flaky scanners and uploads.[^temporal-retry]
- Do not bloat workflow history: store large reports outside the history, use the claim-check pattern, and keep only references/identifiers in workflow state.[^temporal-worker]
- For long-lived processes, plan for Continue-As-New and worker versioning from the start.[^temporal-worker]
- Schedules are useful for `docs-guardian`, periodic security checks, and maintenance jobs.[^temporal-schedule]

### 13.4. Why this evolution can be low-pain

If in version one you already:

- keep your own YAML DSL,
- use an artifact bus,
- isolate side effects in MCP services,
- normalize access profiles,

then the move to Rust + Temporal is mostly **a replacement of the orchestration layer**, not a rewrite of the whole system.

---

## 14) Step-by-step implementation without unnecessary heroics

### Phase 1

Assemble the minimum viable stack:

1. SwiftUI desktop app
2. local adapter to `goosed` over REST/SSE
3. YAML loader/validator
4. artifact bus (`json + md`)
5. worktree manager
6. approval UI
7. 10 key agents:
   - Lead
   - 4 proposal reviewers
   - Proposal writer
   - Code writer
   - Audit
   - Pre-push review
   - Docs guardian

### Phase 2

Add deterministic integrations:

1. GitHub MCP
2. Build/archive MCP
3. Connect publisher MCP
4. Security scanner wrappers
5. headless/scheduled runs for maintenance

### Phase 3

Add durability and scale:

1. Rust orchestrator
2. Temporal
3. task queues by task class
4. retries, updates, versioning
5. more detailed observability and cost accounting

---

## 15) The shortest recommendation

If I cut through this without sentiment, I would do it like this:

- **SwiftUI**: UI, approvals, live trace, artifacts
- **your YAML DSL**: the only public contract for describing agents and workflows
- **Goose (`goosed`)**: runtime execution of agent tasks over REST/SSE
- **MCP in Go/Rust**: all dangerous side effects
- **worktree per writer**
- **JSON-schema handoffs**
- **Temporal later, once stage design stops moving**

That gives you a system that:

- can be built quickly,
- can still be understood a month later,
- can later move onto a Rust backend without open-heart surgery.

---

## 16) Sources

### Goose

- [Goose custom interfaces / CUSTOM_DISTROS](https://github.com/block/goose/blob/main/CUSTOM_DISTROS.md)
- [Goose ACP clients](https://block.github.io/goose/docs/guides/acp-clients/)
- [Goose recipe reference](https://block.github.io/goose/docs/guides/recipes/recipe-reference/)
- [Goose running tasks](https://block.github.io/goose/docs/guides/running-tasks/)
- [Goose headless mode](https://block.github.io/goose/docs/tutorials/headless-goose/)
- [Goose schedules](https://block.github.io/goose/docs/guides/schedules/)
- [Goose using skills](https://block.github.io/goose/docs/guides/context-engineering/using-skills/)
- [Goose subagents guide](https://block.github.io/goose/docs/guides/subagents/)
- [Goose subrecipes in parallel](https://block.github.io/goose/docs/tutorials/subrecipes-in-parallel/)
- [Goose permissions](https://block.github.io/goose/docs/guides/goose-permissions/)
- [Goose tool permissions](https://block.github.io/goose/docs/guides/managing-tools/tool-permissions/)
- [Goose .gooseignore](https://block.github.io/goose/docs/guides/using-gooseignore/)
- [Goose allowlist](https://block.github.io/goose/docs/guides/allowlist/)
- [Goose isolated development environments](https://block.github.io/goose/docs/tutorials/isolated-development-environments/)
- [Goose lead/worker tutorial](https://block.github.io/goose/docs/tutorials/lead-worker/)
- [Goose v1.25.0 release notes](https://block.github.io/goose/blog/2026/02/23/goose-v1-25-0/)

### Codex

- [Codex app-server](https://developers.openai.com/codex/app-server/)
- [Codex subagents / custom agents](https://developers.openai.com/codex/subagents/)
- [Codex config basic](https://developers.openai.com/codex/config-basic/)
- [Codex skills](https://developers.openai.com/codex/skills)
- [Codex app features](https://developers.openai.com/codex/app/features/)
- [Codex worktrees](https://developers.openai.com/codex/app/worktrees/)

### Zed

- [Zed ACP](https://zed.dev/acp)
- [Zed external agents](https://zed.dev/docs/ai/external-agents)
- [Zed agent server extensions](https://zed.dev/docs/extensions/agent-servers)

### Continue

- [Continue checks reference](https://docs.continue.dev/checks/reference)
- [Continue checks best practices](https://docs.continue.dev/checks/best-practices)
- [Continue GitHub integration](https://docs.continue.dev/mission-control/integrations/github)
- [Continue beyond checks / Mission Control](https://docs.continue.dev/mission-control/beyond-checks)

### MCP

- [MCP basic architecture](https://modelcontextprotocol.io/specification/2025-06-18/basic)
- [MCP transports](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [MCP tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
- [MCP resources](https://modelcontextprotocol.io/specification/2025-06-18/server/resources)
- [MCP prompts](https://modelcontextprotocol.io/specification/2025-06-18/server/prompts)
- [MCP Tasks (experimental)](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks)

### Temporal

- [Temporal workflows](https://docs.temporal.io/workflows)
- [Temporal child workflows](https://docs.temporal.io/child-workflows)
- [Temporal retry policies](https://docs.temporal.io/encyclopedia/retry-policies)
- [Temporal schedules](https://docs.temporal.io/schedule)
- [Temporal task queues](https://docs.temporal.io/task-queue)
- [Temporal worker best practices](https://docs.temporal.io/best-practices/worker)
- [Temporal worker versioning](https://docs.temporal.io/worker-versioning)

---

## 17) Footnotes

[^goose-custom]: Goose documents custom UI integration through `goose-server` / `goosed`, OpenAPI, and session endpoints, and separately describes the ACP path.
[^goose-acp]: In Goose docs, ACP clients are described as experimental; ACP also brings benefits for editor-style integration, including client-side file operations, visibility into unsaved changes, native diffs, and several concurrent sessions with isolated state.
[^goose-recipe]: Goose recipe reference documents YAML/JSON recipes, `settings.goose_provider`, `settings.goose_model`, `response.json_schema`, `retry`, `sub_recipes`, and related fields.
[^goose-headless]: Goose headless mode and `goose run` are designed for non-interactive task execution and automation scenarios.
[^goose-schedule]: Goose supports schedules for recipe-based automation.
[^goose-skills]: Goose skills are loaded through Summon; the docs recommend keeping skills narrow and testable.
[^goose-v125]: In Goose v1.25.0, the older fragmented skills/subagents mechanisms were consolidated into Summon with `load` and `delegate`, and sandboxing plus per-recipe provider/model editing in the GUI were added.
[^goose-perm]: Goose supports permission modes: Completely Autonomous, Manual Approval, Smart Approval, and Chat Only.
[^goose-toolperm]: Goose documents granular tool permissions and also notes that too many tools in one session reduce quality.
[^goose-gooseignore]: `.gooseignore` limits Developer extension access to files; if a custom `.gooseignore` is absent, Goose protects `.env`, `.env.*`, and `secrets.*` by default.
[^goose-allowlist]: Goose supports command allowlisting through `GOOSE_ALLOWLIST`.
[^goose-isolated]: Goose has a dedicated tutorial on isolated development environments using branches, containers, and isolated environments.
[^goose-subrecipes]: Goose recipes support `sub_recipes` for larger workflows.
[^goose-parallel]: Goose separately documents parallel execution for subrecipes as an experimental capability using isolated worker processes.
[^goose-leadworker]: Goose supports lead/worker model routing through separate provider/model settings for first-pass planning and worker execution.

[^codex-appserver]: Codex app-server is an open-source interface layer for custom rich clients; it uses JSON-RPC over stdio/WebSocket and supports streamed agent events.
[^codex-subagents]: Codex custom agents / subagents use a schema with `model`, `model_reasoning_effort`, sandbox mode, MCP servers, and skills; unspecified fields are inherited from the parent.
[^codex-config]: Codex config includes an explicit reasoning-effort knob.
[^codex-skills]: In Codex, a skill is a directory containing `SKILL.md` and optional extra resources; metadata is loaded first, then the full skill as needed.
[^codex-approvals]: Codex documents approvals and sandboxing for commands and filesystem/network access.
[^codex-worktrees]: The Codex app documents worktrees as a way to run parallel tasks in one project without collisions.

[^zed-acp]: Zed promotes ACP as an open protocol for external coding agents.
[^zed-external]: Zed supports external agents, including Claude/Codex/Gemini integrations, and documents feature-support differences between them.
[^zed-agent-server]: Zed supports packaging and registering custom agent servers through extensions.

[^continue-checks]: Continue checks use markdown files with YAML frontmatter and live in `.continue/checks/` or `.agents/checks/`.
[^continue-best]: Continue recommends one check per concern and positions checks as an intermediate layer between linters/tests and humans.
[^continue-gh]: Continue documents GitHub/PR-oriented scenarios and event-driven automation through Mission Control.

[^mcp-basic]: MCP defines the basic architecture, lifecycle, and the separation of client/server responsibilities.
[^mcp-tools]: MCP tools are model-controlled functions with input schema and metadata.
[^mcp-resources]: MCP resources are application-controlled context with a URI model.
[^mcp-tasks]: MCP Tasks were introduced as an experimental utility for deferred result retrieval and stateful task management.

[^temporal-workflows]: Temporal defines workflows as code-driven step sequences with a durable execution model.
[^temporal-child]: Temporal explicitly warns against using child workflows merely for "code organization"; activities are often the better choice.
[^temporal-retry]: Temporal retry policies apply to activities by default, not to workflows.
[^temporal-schedule]: Temporal schedules are separate entities for time-based workflow execution.
[^temporal-worker]: Temporal worker best practices cover task queues, history growth, worker versioning, payload limits, and Continue-As-New.
