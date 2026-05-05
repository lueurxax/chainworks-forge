# Chainworks Forge — Common Commands

## 🛠️ Building & Testing

### Swift App (macOS)

```bash
# List available test gates
./scripts/test-gate.sh list

# Build only (compile check)
./scripts/test-gate.sh build

# Run fast tests (default inner loop)
./scripts/test-gate.sh fast

# Run guardrails (lints, no build)
./scripts/test-gate.sh guardrails

# Run full test suite
./scripts/test-gate.sh full

# Run specific proposal gate
./scripts/test-gate.sh proposal-027

# Clean build artifacts
./scripts/clean-build-caches.sh
```

### Rust Control-Plane

```bash
# Navigate to control-plane
cd control-plane

# Build debug binary
cargo build

# Build release binary
cargo build --release

# Run all tests
cargo test

# Run tests for specific crate
cargo test -p engine

# Run specific test
cargo test -p engine test_name_pattern

# Check code
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy

# Go back to root
cd ..
```

## 🏃 Running the App

### From Terminal

```bash
# Open Xcode project
open "Chainworks Forge.xcodeproj"

# In Xcode: Product → Run (or Cmd+R)
# On first launch: Settings → Configure Provider
```

### From Terminal (CLI)

```bash
# Build and run
open "Chainworks Forge.xcodeproj"
xcodebuild -scheme "Chainworks Forge" -configuration Debug build

# But better to use test-gate:
./scripts/test-gate.sh build
```

## 🚀 Running Rust Daemon

```bash
cd control-plane

# Set up environment
export DATABASE_URL="sqlite:///.chainworks/control-plane.db?mode=rwc"
export GRAPHQL_ADDR="127.0.0.1:4000"
export RUST_LOG=info

# Run daemon
./target/debug/control-plane 2>/tmp/cw.log &

# Check if running
ps aux | grep control-plane

# View logs
tail -f /tmp/cw.log

# Stop daemon
pkill control-plane
```

## 📁 Navigation Commands

```bash
# Go to repo root
cd "Chainworks Forge"

# View repository structure
ls -la

# Find Swift files in Engine
find "Chainworks Forge/Engine" -name "*.swift" -type f

# Find test files
find "Chainworks ForgeTests" -name "*Tests.swift" -type f

# Find YAML examples
find examples -name "*.yaml" -type f

# Find reference docs
find docs/reference -name "*.md" -type f

# List all proposal docs
ls docs/proposals/
```

## 📊 Code Search

```bash
# Find all uses of RunPlanCompiler
grep -r "RunPlanCompiler" --include="*.swift"

# Find all WorkflowOrchestrator
grep -r "WorkflowOrchestrator" --include="*.swift"

# Find artifact storage calls
grep -r "ArtifactStorage" --include="*.swift"

# Find transition evaluator usage
grep -r "TransitionEvaluator" --include="*.swift"

# Find Rust engine impl
grep -r "struct Orchestrator" control-plane --include="*.rs"

# Find all TODO comments
grep -r "TODO:" --include="*.swift"

# Find FIXME comments
grep -r "FIXME:" --include="*.swift"
```

## 🔍 Understanding Code

### Using Xcode

```bash
# Open project
open "Chainworks Forge.xcodeproj"

# In Xcode:
# Cmd+Shift+O — Open file by name
# Cmd+F — Search in current file
# Cmd+Shift+F — Search across project
# Cmd+Click — Jump to definition
# Cmd+Option+Click — Show quick help
# Cmd+B — Build
# Cmd+U — Test
```

### Using Command Line

```bash
# Find definition of function
grep -n "func executeStateMachine" Chainworks\ Forge/Engine/*.swift

# Find all references to a class
grep -r "class Run" --include="*.swift"

# Show file with line numbers
cat -n "Chainworks Forge/Models/Run.swift" | head -50

# Count Swift files
find "Chainworks Forge" -name "*.swift" -type f | wc -l

# Count Rust files
find control-plane -name "*.rs" -type f | wc -l

# Show file size
ls -lh "Chainworks Forge/Engine/WorkflowOrchestrator.swift"
```

## 📝 Working with Documentation

### Reading Docs

```bash
# Open docs in browser
open docs/README.md

# View specific reference doc
cat docs/reference/current-system-baseline.md | head -100

# View proposal
cat docs/proposals/031-thin-graphql-read-boundary.md | head -200

# Search docs for keyword
grep -r "RunPlanSnapshot" docs --include="*.md"

# View examples
cat examples/workflows/full-mvp-live.yaml

# View agents
cat examples/agents/agents.yaml
```

### File Structure

```bash
# Show directory tree (first 2 levels)
find . -maxdepth 2 -type d | head -20

# Show all Swift files in Engine
ls -la "Chainworks Forge/Engine/"

# Show all test files
ls "Chainworks ForgeTests/"

# Show all Rust crates
ls control-plane/crates/

# Show example workflows
ls examples/workflows/

# Show example agents
ls examples/agents/
```

## 🧪 Testing Specific Components

```bash
# Test only RunPlanCompiler
./scripts/test-gate.sh proposal-021

# Test only WorkflowOrchestrator
./scripts/test-gate.sh proposal-024

# Test proposal delivery
./scripts/test-gate.sh proposal-007

# Run fast tests once more
./scripts/test-gate.sh fast

# List what proposal-XXX tests exist
./scripts/test-gate.sh list | grep proposal
```

## 🐛 Debugging

```bash
# View test output log
cat test_output.log

# Check for recent errors
tail -100 test_output.log

# View daemon logs (if running)
tail -f /tmp/cw.log

# Check database
sqlite3 .chainworks/control-plane.db "SELECT * FROM command_journal LIMIT 5;"

# View Swift app logs
log stream --predicate 'eventMessage contains[c] "Chainworks"'

# Check process info
ps aux | grep -E "control-plane|Forge"
```

## 🔐 Git Operations (Careful!)

```bash
# View current branch
git branch

# View recent commits
git log --oneline -10

# View changes
git status

# View diff
git diff

# DO NOT RUN THESE WITHOUT PERMISSION:
# git reset --hard
# git checkout -- .
# git clean -fd

# Safe operations:
git fetch
git log
git show <commit>
git diff HEAD
```

## 📦 Managing Dependencies

```bash
# Swift/Xcode (via Package Manager in Xcode)
# Add dependency: File → Add Packages...

# Rust dependencies
cd control-plane
cargo add <crate_name>           # Add dependency
cargo update                      # Update lock file
cargo remove <crate_name>        # Remove dependency
cd ..

# Check outdated
cd control-plane && cargo outdated && cd ..
```

## 🧹 Cleanup

```bash
# Clean build artifacts (safe)
./scripts/clean-build-caches.sh

# Remove Xcode derived data (safe)
rm -rf ~/Library/Developer/Xcode/DerivedData/

# Remove Rust build (safe)
cd control-plane && cargo clean && cd ..

# DO NOT DO:
# rm -rf .chainworks/        # Run data!
# rm *.db                     # Database!
# git reset --hard           # Without permission!
```

## 🔗 Useful Links (in this repo)

```bash
# Documentation index
cat docs/README.md

# Current system baseline
cat docs/reference/current-system-baseline.md

# Architecture decisions
cat docs/reference/architecture-decisions.md

# Workflow execution engine
cat docs/reference/workflow-execution-engine.md

# ACP runtime transport
cat docs/reference/acp-runtime-transport.md

# Full example workflow
cat examples/workflows/full-mvp-live.yaml

# Full example agents
cat examples/agents/agents.yaml

# MVP sign-off
cat docs/reference/mvp-sign-off.md

# Test gates documentation
cat docs/reference/test-gates.md
```

## 🎯 Developer Workflow

```bash
# 1. Start your day
cd "Chainworks Forge"
git fetch                          # Get latest
./scripts/test-gate.sh fast        # Quick validation

# 2. Make changes
# ... edit files in Xcode ...

# 3. Test your changes
./scripts/test-gate.sh fast        # Run tests
git status                         # Review changes

# 4. Before commit (if you have permission)
./scripts/test-gate.sh guardrails  # Lint check
git diff                           # Review diff

# 5. Document changes (optional)
# Update relevant docs/reference/*.md files
```

## 💾 Common File Locations

```
Key files:
  RunPlanCompiler           → Chainworks Forge/Engine/RunPlanCompiler.swift
  WorkflowOrchestrator      → Chainworks Forge/Engine/WorkflowOrchestrator.swift
  Run model                 → Chainworks Forge/Models/Run.swift
  Tests                     → Chainworks ForgeTests/
  Examples                  → examples/
  Documentation             → docs/
  Rust engine               → control-plane/crates/engine/src/
  Rust daemon               → control-plane/crates/daemon/src/
  Rust database             → control-plane/crates/db/src/
  GraphQL server            → control-plane/crates/graphql-server/src/
```
