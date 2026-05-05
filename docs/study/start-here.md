# 🚀 Start Here — Chainworks Forge Repository Study

Welcome! This file gets you oriented in **5 minutes**.

## What is Chainworks Forge?

**Chainworks Forge** is a macOS application that orchestrates multi-agent engineering workflows. The main idea:

```
Idea → Workflow → Agents → Artifacts → Approvals → Delivery
```

It is **not a chat app** — it is a deterministic workflow engine where:
- Workflows are defined in YAML
- Multiple agents (Claude, Gemini, Codex) work on tasks
- Artifacts are stored on disk
- You explicitly approve progress at gates
- Code is delivered to your repository

## 5-Minute Quickstart

### Step 1: Build & Test (2 min)

```bash
cd "Chainworks Forge"
./scripts/test-gate.sh build    # Build
./scripts/test-gate.sh fast     # Quick tests
```

### Step 2: Choose Your Interest (1 min)

Pick **ONE**:
- 🔧 Execution Engine? → Read `docs/reference/workflow-execution-engine.md`
- 🤖 Providers & Agents? → Read `examples/agents/agents.yaml`
- 📦 Artifacts & Storage? → Read `docs/study/code-examples.md`
- 🎨 User Interface? → Browse `Chainworks Forge/Views/`
- 🦀 Rust Daemon? → Read `docs/reference/rust-control-plane.md`

### Step 3: Explore the Code (2 min)

```bash
open "Chainworks Forge.xcodeproj"  # Open in Xcode
# Cmd+Shift+O to search for files
# Cmd+F to search within a file
```

## 📚 Study Materials in docs/study/

| File | Purpose | Time |
|------|---------|------|
| **start-here.md** | This file | 5 min |
| **README.md** | Navigation guide | 10 min |
| **quick-reference.md** | Quick lookups | 5 min |
| **getting-started.md** | Learning tracks | 20 min |
| **repository-study.md** | Detailed structure | 60 min |
| **architecture-diagrams.md** | Visual flows | 45 min |
| **code-examples.md** | Practical patterns | 60 min |
| **common-commands.md** | CLI commands | 20 min (ref) |

## 🎯 Which File To Read Next?

### If you have **5 minutes:**
→ **docs/study/quick-reference.md**

### If you have **30 minutes:**
→ **docs/study/quick-reference.md** → **docs/study/getting-started.md** (setup section)

### If you have **1–2 hours:**
→ **docs/study/README.md** → pick a specialization → follow that track

### If you have **a full day:**
→ Read everything in order: README.md → getting-started.md → architecture-diagrams.md → code-examples.md

## 🏗️ The 30-Second Repository Overview

```
Chainworks Forge/           ← macOS app (Swift/SwiftUI)
  Engine/                   ← Execution, compilation
  Models/                   ← Data models
  Views/                    ← User interface
  Providers/                ← Agent providers

control-plane/              ← Rust daemon (parity)
  crates/engine/            ← State machine
  crates/db/                ← SQLite database
  crates/graphql-server/    ← GraphQL API

docs/reference/             ← Authoritative truth
examples/                   ← Working examples
```

## 💡 5 Key Concepts

1. **Run** — One execution of a workflow (not a chat!)
2. **Stage** — A state in the workflow where agents work
3. **Agent** — A worker (Claude, Gemini, Codex, etc.)
4. **Artifact** — An output file (proposal, code, review, etc.)
5. **Approval Gate** — Manual pause requiring explicit OK to continue

## 🔥 Top 3 Files to Understand First

1. **examples/workflows/full-mvp-live.yaml** — See a complete real workflow
2. **examples/agents/agents.yaml** — See how agents are defined
3. **Chainworks Forge/Engine/RunPlanCompiler.swift** — See how YAML becomes executable

## ✅ Verify Your Setup

```bash
./scripts/test-gate.sh build    # Should succeed
./scripts/test-gate.sh fast     # Should succeed
open "Chainworks Forge.xcodeproj"  # Should open Xcode
```

## 🚨 Important: Read Official Docs Too

These study materials are **introductory**. For authoritative truth, always check:

- `docs/README.md` — Official documentation index
- `docs/reference/current-system-baseline.md` — What's implemented at HEAD
- `docs/reference/workflow-execution-engine.md` — Execution engine details

## 🎓 Learning Paths

### Path A: Deep Understanding (6–8 hours)
1. `docs/study/README.md`
2. `docs/study/getting-started.md`
3. Choose a specialization track and follow it
4. Explore code in Xcode

### Path B: Quick Overview (30 minutes)
1. This file (start-here.md)
2. `docs/study/quick-reference.md`
3. `docs/study/architecture-diagrams.md` (System Overview section)
4. Pick ONE code file to explore

### Path C: Task-Focused (1–2 hours)
1. `docs/study/quick-reference.md`
2. `docs/study/getting-started.md` (your specialization section)
3. `docs/study/architecture-diagrams.md` (relevant sections)
4. `docs/study/code-examples.md` (relevant examples)
5. Dive into actual code

## 📞 Quick Links

| Need | Resource |
|------|----------|
| Navigation | `docs/study/README.md` |
| Quick lookup | `docs/study/quick-reference.md` |
| Setup | `docs/study/getting-started.md` |
| Visual explanations | `docs/study/architecture-diagrams.md` |
| Code patterns | `docs/study/code-examples.md` |
| CLI commands | `docs/study/common-commands.md` |

---

**Ready to dive in? → Read `docs/study/README.md` next**

Good luck! 🚀
