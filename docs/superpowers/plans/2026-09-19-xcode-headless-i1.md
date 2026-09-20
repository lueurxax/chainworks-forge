# Headless I1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Native execution is recommended for this tightly coupled experimental slice; the user has not selected an execution method yet.

**Goal:** Implement an isolated, development-only open/read experiment that can produce evidence for H1 with Xcode IDE closed, without changing production admission.

**Architecture:** Add one explicitly invoked Cargo example in the existing ACP package, with private modules for disposable projects, host inspection, bounded MCP transport, two tool adapters, evidence and orchestration. Inject host/peer/evidence interfaces for offline tests. The example is not exported from the ACP library or connected to a daemon, provider session, HTTP route or shim.

**Tech Stack:** Rust 2021, existing Tokio/serde/serde_json/anyhow/async-trait/libc/sha2/uuid dependencies, tempfile for tests, macOS Xcode Service through the selected installation's mcpbridge. No new package or network dependency is planned.

**Spec:** [I1 contract](../specs/2026-09-19-xcode-headless-iteration-1.md), within the [R3 parent](../specs/2026-09-19-xcode-headless-mcp-design.md). Read both before execution.

## Status And Provenance

Plan revision 1, 2026-09-19. Draft for user review; no task below has been run.
The [received review](../../evidence/xcode-headless-i1-review-2026-09-19.md)
is Ready for I1 planning only. It is not approval of this previously unwritten
plan, implementation, a live project open, or production migration.

Source HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
Parent MD5: `e9d6a8dad4bbc2d9ab38cbc4dc88142b`.
I1 SHA-256: `509ca0ba4d2f97e5b71505092537f15cf1ab67d9af29962371feab59523c4045`.
The reviewed specifications are intentionally unchanged. The three repeat-review
P2 findings remain open for later production stages, not tasks in this plan.

## Global Constraints

- "Keep the experiment opt-in in a development/test harness."
- "No production flag may disable binding or confinement checks for this path."
- "There is no caller-supplied arbitrary tool/method/argument passthrough."
- "Fixtures contain no secrets, external references, packages, symlinks or build scripts."
- "Do not approve folders/agents, reset permissions or use unsafe allow-all automatically."
- "Close/reap only owned bridge processes."
- "No successful example can be extrapolated to those guarantees."
- Use two newly created roots outside the real checkout and existing run worktrees; never open the Chainworks project as the I1 fixture.
- Only internal `XcodeOpenWorkspace`, `XcodeRead`, MCP initialization/discovery and read-only CLI status are in scope.
- Warm attach only. Never launch the IDE, cold-start/restart/stop the shared service, close workspaces or enable/reset permissions.
- Record local open intent durably before dispatch; uncertain open means stop, no replay, reopen, resume or automatic cleanup.
- Rust verification uses `scripts/cargo-managed`. Swift verification, if separately added to scope, uses `scripts/test-gate.sh`; UI tests are remote-only.
- No daemon deployment, run retry, catalog/frozen snapshot change, production DB migration, commit or push is authorized.
- Respect existing dirty files. Execution setup must assess checkout isolation with `using-git-worktrees`; read/copy the exact pinned uncommitted specs into an isolated checkout if one is used, without silently losing them.

## Review Focus

1. Read-only tasks assigned a shared implementation worktree must not resolve to the repository; explicit missing worktrees fail before open. Task 1 pins this.
2. A same PID with a different start identity, wrong executable or unreadable process state must block dispatch, including reconnect. Tasks 2 and 4 pin this.
3. Notifications, oversized or contradictory result encodings must not reset deadlines, masquerade as success, or cause another open. Tasks 2 and 3 pin this.
4. Disk failure/cancellation immediately around the open write must leave either zero host bytes or durable uncertainty, never retry permission. Task 4 pins this.
5. Raw error messages, foreign workspace names and stale approvals must not reach public evidence or authorize a changed executable/fixture. Tasks 4 and 5 pin this.

## Existing Code And File Map

Verified planning anchors:

- `control-plane/crates/acp/src/xcode_broker.rs` already clears host environment and owns child stdin/stdout, but its session type is private, lease-based and IDE-targeted. Do not reuse it by forging `XcodeTargetSnapshot`, change its visibility, or refactor its production pump for I1.
- `control-plane/crates/acp/src/transport.rs::isolate_process_group` is public and reusable. Its `signal_process_group` helper is crate-private; do not expose it. The example shuts down/reaps only its own `Child` handles.
- Engine effective-root selection includes dedicated/shared strategies; the adapter helper currently checks write-enabled alone. Mirror the intended cases in experimental pure selection, without changing either production caller.
- ACP already depends on required libraries. Locked `libc` 0.2.189 exposes `proc_pidpath`, `proc_pidinfo`, `PROC_PIDTBSDINFO` and `proc_bsdinfo` on macOS; use these instead of custom ABI structs or argv identity.
- Existing ACP tests use synthetic subprocess fixtures; `CHAINWORKS_JUNIE_ACP_LIVE_SMOKE` can opt into a real provider and must be unset in the regression command below.

All paths in the following table are repository-relative and are planned files,
not a claim that they already exist.

| Path | Responsibility / task |
| --- | --- |
| `control-plane/crates/acp/Cargo.toml` | Register a testable example; no dependency additions; T1 |
| `control-plane/crates/acp/examples/xcode_headless_i1.rs` | Thin explicit CLI entry, no startup work in tests; T1/T5 |
| `control-plane/crates/acp/examples/xcode_headless_i1/mod.rs` | Private module wiring and shared result alias; T1/T5 |
| `control-plane/crates/acp/examples/xcode_headless_i1/project.rs` | Root/project selection, fixture construction and source digests; T1 |
| `control-plane/crates/acp/examples/xcode_headless_i1/host.rs` | Injected host facts, macOS identity/status and launch identity; T2 |
| `control-plane/crates/acp/examples/xcode_headless_i1/protocol.rs` | Owned process, bounded NDJSON, deadlines and handshake; T2 |
| `control-plane/crates/acp/examples/xcode_headless_i1/tools.rs` | Only open/read requests and observed result decoding; T3 |
| `control-plane/crates/acp/examples/xcode_headless_i1/evidence.rs` | Private intent/result storage and redacted evidence; T4 |
| `control-plane/crates/acp/examples/xcode_headless_i1/controller.rs` | Injected sequence, stop rules and experimental observations; T4 |
| `control-plane/crates/acp/examples/xcode_headless_i1/cli.rs` | Prepare/inspect/run separation and exact approval comparison; T5 |
| `control-plane/crates/acp/tests/fixtures/xcode_headless_i1/project.pbxproj` | Minimal static project template; T1 |
| `control-plane/crates/acp/tests/fixtures/xcode_headless_i1/apple-contract.json` | Mechanically derived open/read metadata with source digest; T3 |
| `control-plane/crates/acp/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py` | Offline process fixture, not a selectable live backend; T2/T5 |

Unit tests live next to their owning private modules. Fake host/peer/recorder
implementations are `#[cfg(test)]` only. No changes to `acp/src/lib.rs`, engine,
daemon, auth, DB, GraphQL, Swift, production broker/shim or canonical references.
Extraction into production modules, if warranted, belongs to reviewed I2 work.

## Shared Implementation Decisions

Use `anyhow::Result<T>` internally with static `i1_*` error categories; do not
serialize raw error chains. Types below are private to the example. Public
evidence and private cleanup/approval records are separate outputs.

The first live lane accepts only the captured Xcode build `27A266a`, server
`xcode-tools` version `25317`, negotiated protocol `2025-06-18`, and structurally
matching open/read schemas. A changed installation is a blocked/deviation
record requiring deliberate adapter review, not an automatic compatibility bet.
Use a `2025-06-18` offer; the legacy capture is an offline comparison fixture,
not an automatic reconnect/protocol fallback. Never pin the old research PID.

Plan-specific bounds: 1 MiB per NDJSON line, 8 MiB total received per bridge,
64 nonterminal messages per request, at most 16 tool-list pages, 30 seconds per
request, 5 minutes overall live sequence (plus bounded cleanup), 5 seconds for each owned-child shutdown
stage, 4 KiB retained stderr, and read `offset=1`, `limit=8` for one-line sentinels.
Activity must not extend absolute deadlines. The plan does not alter production
timeout configuration. Bound CLI metadata commands to 5 seconds and 1 MiB output.

Scratch layout is a fresh UUID directory containing `A/Fixture.xcodeproj`,
`B/Fixture.xcodeproj`, their `Sources/Sentinel.txt` files, and a private control
directory. Seed contents are exactly `CW_I1_A\n` and `CW_I1_B\n`. The only read
path is `Fixture/Sources/Sentinel.txt`. That project-organization spelling is a
testable assumption: an unsupported path shape is a recorded deviation, not a
reason to add project enumeration or guess paths at runtime.

There is no live `--resume`, `--retry`, `--tool`, `--project`, `--pid` or alternate
bridge-command option. A prior attempted fixture is never reopened by this
binary. A later operator-authorized investigation is a separate decision.

---

### Task 1: Disposable Project Selection And Fixtures

**Files:** Cargo example registration, entry/module wiring, `project.rs`, static
`project.pbxproj` from the file map. No host or MCP access in this task.

**Interfaces:** Produce these private functions/types for later tasks:

```rust
pub struct FixtureProject {
    pub label: String,
    pub root: std::path::PathBuf,
    pub package: std::path::PathBuf,
    pub read_path: String,
    pub marker: String,
}
pub struct FixturePair { pub projects: [FixtureProject; 2] }
pub fn resolve_execution_root(
    repository: &std::path::Path,
    worktree: Option<&std::path::Path>,
    write_enabled: bool,
    strategy: Option<&str>,
) -> anyhow::Result<std::path::PathBuf>;
pub fn select_project(
    root: &std::path::Path, selector: Option<&std::path::Path>,
) -> anyhow::Result<std::path::PathBuf>;
pub fn create_pair(fresh_root: &std::path::Path) -> anyhow::Result<FixturePair>;
pub fn seed_digests(pair: &FixturePair)
    -> anyhow::Result<std::collections::BTreeMap<String, String>>;
```

- [ ] **Step 1: Register the example and write failing selection tests.**

```toml
[[example]]
name = "xcode_headless_i1"
path = "examples/xcode_headless_i1.rs"
test = true
```

Use `#[path = "xcode_headless_i1/mod.rs"] mod harness;` in the entry. Initial
non-test `main` only returns an explicit not-yet-available CLI error, not a live
probe. Shared `mod.rs` declares only modules delivered so far.

```rust
#[test]
fn i1_shared_read_only_uses_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let worktree = temp.path().join("worktree");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&worktree).unwrap();
    let actual = resolve_execution_root(
        &repo, Some(&worktree), false, Some("shared_implementation_worktree"),
    ).unwrap();
    assert_eq!(actual, worktree.canonicalize().unwrap());
    assert!(resolve_execution_root(&repo, None, false, Some("dedicated")).is_err());
}
```

- [ ] **Step 2: Observe RED through the managed runner.**

From `control-plane`, run `../scripts/cargo-managed test --locked -p acp --example xcode_headless_i1 i1_shared_read_only_uses_worktree`.
The first compile may fail for the new missing functions; after minimal signatures,
confirm the assertion fails for repository fallback, not an unrelated setup error.

- [ ] **Step 3: Implement pure resolution and fixed fixtures.**

```rust
let explicitly_required = matches!(
    strategy, Some("dedicated") | Some("shared_implementation_worktree")
);
if explicitly_required && worktree.is_none() {
    anyhow::bail!("i1_missing_worktree");
}
let candidate = if explicitly_required || write_enabled {
    worktree.unwrap_or(repository)
} else {
    repository
};
let root = candidate.canonicalize()?;
anyhow::ensure!(root.is_dir(), "i1_missing_root");
```

Preserve legacy write-enabled/no-explicit-strategy repository fallback; reject
unrecognized nonempty strategies in the experiment. Explicit selection must
canonicalize inside root; otherwise only one immediate project/workspace package
is accepted. Deny symlink fixture components and ambiguous/missing projects.
`create_pair` requires a nonexistent root, creates files exclusively and never
uses temporary-directory auto-deletion for live fixtures. Test-only callers own
their temp directories. Digest exactly both project files and both sentinels,
separately list added service metadata after a run.

Use this complete target-free template; no package references or build phases:

```text
// !$*UTF8*$!
{
 archiveVersion = 1;
 classes = {};
 objectVersion = 56;
 objects = {
  000000000000000000000001 = {
   isa = PBXProject;
   buildConfigurationList = 000000000000000000000004;
   compatibilityVersion = "Xcode 14.0";
   developmentRegion = en;
   hasScannedForEncodings = 0;
   knownRegions = (en, Base);
   mainGroup = 000000000000000000000002;
   projectDirPath = "";
   projectRoot = "";
   targets = ();
  };
  000000000000000000000002 = {
   isa = PBXGroup;
   children = (000000000000000000000003);
   sourceTree = "<group>";
  };
  000000000000000000000003 = {
   isa = PBXGroup;
   children = (000000000000000000000006);
   path = Sources;
   sourceTree = "<group>";
  };
  000000000000000000000004 = {
   isa = XCConfigurationList;
   buildConfigurations = (000000000000000000000005);
   defaultConfigurationIsVisible = 0;
   defaultConfigurationName = Debug;
  };
  000000000000000000000005 = {
   isa = XCBuildConfiguration;
   buildSettings = {};
   name = Debug;
  };
  000000000000000000000006 = {
   isa = PBXFileReference;
   lastKnownFileType = text;
   path = Sentinel.txt;
   sourceTree = "<group>";
  };
 };
 rootObject = 000000000000000000000001;
}
```

Whether the installed service accepts this minimal project is part of the
experiment, not an established fact. Do not add a real app to make it pass.

- [ ] **Step 4: Add and run the complete project case set.** Cover repository,
legacy fallback, dedicated/shared roots including read-only, deleted required
root, ambiguous candidates, outside-root selection, fixture symlinks, identical
relative names/different markers, and refusal to overwrite an existing root.
Run `../scripts/cargo-managed test --locked -p acp --example xcode_headless_i1 project::`.
Expected: all selected tests pass with zero Apple/process calls.
- [ ] **Step 5: Review the task diff and record the RED/GREEN result.** No commit
unless separately authorized; do not stage unrelated dirty documentation.

### Task 2: Verified Warm Host And Bounded Owned Transport

**Files:** `host.rs`, `protocol.rs`, `fake_mcpbridge.py`, module declarations.

**Interfaces:** `HostInspector` is injected; `Bridge` is private, never an ACP
backend/lease. Produce the following contracts (derive equality for generation):

```rust
pub struct ServiceGeneration {
    pub uid: u32, pub pid: i32,
    pub start_sec: u64, pub start_usec: u64,
    pub developer_dir: std::path::PathBuf,
    pub executable: std::path::PathBuf,
    pub bundle_id: String, pub build: String,
}
pub struct HostExpectation {
    pub uid: u32,
    pub developer_dir: std::path::PathBuf,
    pub executable: std::path::PathBuf,
    pub bundle_id: String,
}
pub fn select_service(
    expected: &HostExpectation, candidates: &[ServiceGeneration],
) -> anyhow::Result<ServiceGeneration>;
pub struct HostFacts {
    pub generation: ServiceGeneration,
    pub xcode_build: String,
    pub ide_absent: bool, pub enabled: bool,
    pub running: bool, pub unsafe_allow_all: bool,
    pub launch_identity: serde_json::Value,
}
#[async_trait::async_trait]
pub trait HostInspector: Send {
    async fn inspect(&mut self) -> anyhow::Result<HostFacts>;
}
pub fn validate_host(facts: &HostFacts) -> anyhow::Result<()>;
#[derive(Clone, Copy)]
pub struct ExitRecord { pub code: Option<i32>, pub forced: bool }
pub struct Bridge {
    child: Option<tokio::process::Child>,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
    stderr_task: Option<tokio::task::JoinHandle<Vec<u8>>>,
    next_id: u64,
    received_bytes: usize,
    deadline: tokio::time::Instant,
    allowed_packages: std::collections::BTreeSet<std::path::PathBuf>,
    exit: Option<ExitRecord>,
}
```

`Bridge::spawn(&HostFacts, &FixturePair, deadline: tokio::time::Instant) -> Result<Bridge>`
is async. Its example-private `pub(super)` method
`exchange(&mut self, method: &str, params: Value) -> Result<Value>` is used only by the
handshake/tool adapters. `Bridge::close(&mut self) -> Result<ExitRecord>` is async
and idempotent. The live CLI never accepts command, PID or environment overrides.
Define `LiveHost::new() -> Result<LiveHost>` as the implementation of
`HostInspector`, with selected-installation/user expectation captured once.
`select_service` requires exactly one candidate matching that independent
expectation; `validate_host` checks availability, IDE absence and pinned Xcode
build. Neither derives expected UID or installation from the candidate itself. Store
the Xcode installation build separately from the service bundle build and MCP
server version; they are not interchangeable version strings.

`launch_identity` contains only stable executable paths/digests, signing
identities, UID and launcher-executable chain. Unsigned/ad-hoc signatures have
explicit recorded categories. Per-invocation harness/parent PIDs, start times
and observation timestamps are recorded separately and are not equality inputs
for approval. The service PID/start identity remains an exact equality input.

- [ ] **Step 1: Write failing host-decision and bounded-decoder tests.**

Construct `HostFacts` in tests from fixed synthetic values, then change one
fact at a time. Define `decode_line(bytes: &[u8]) -> Result<Value>` in protocol:

```rust
#[test]
fn i1_transport_rejects_oversized_line() {
    let bytes = vec![b' '; 1024 * 1024 + 1];
    assert!(decode_line(&bytes).is_err());
}
```

Test disabled/down/unsafe/IDE-present, wrong UID/executable/install, ambiguous
candidates, unreadable native metadata and reused PID with changed start time.
Use synthetic facts only, not a live host inspection in ordinary tests.
- [ ] **Step 2: Observe RED.** Run each new module filter separately with
`../scripts/cargo-managed test --locked -p acp --example xcode_headless_i1 host::`
and the same command ending in `protocol::`.
- [ ] **Step 3: Implement host observation and transport.** Select the configured
developer directory or `xcode-select -p`, then use that installation consistently.
Candidate process names only narrow inspection; canonical `proc_pidpath`, native
UID/start fields and bundle metadata establish identity. Treat permission-denied
inspection as unknown/blocking, never as service absence. Status is bounded JSON
from the selected `mcp-server status --format json`; only explicit booleans count.
Non-macOS live calls return unsupported before spawning. Record actual example,
bridge and parent launch/signing identities; ad-hoc/unsigned is an observed value,
not a packaged approval claim.

```rust
let mut command = tokio::process::Command::new(&bridge_executable);
command.env_clear()
    .env("HOME", &home).env("TMPDIR", &tmpdir)
    .env("DEVELOPER_DIR", &facts.generation.developer_dir)
    .env("USER", &account).env("LOGNAME", &account)
    .env("PATH", &minimal_host_path)
    .env("MCP_XCODE_PID", facts.generation.pid.to_string())
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true);
acp::transport::isolate_process_group(&mut command);
```

Here `bridge_executable` comes from the selected installation, `home/account`
from the actual host user, and `tmpdir` from validated host context; these are
not arbitrary request arguments. Omit `MCP_XCODE_SESSION_ID`. Drain stderr with
a 4-KiB retained cap rather than blocking the pipe. Read NDJSON incrementally
with a limit before allocating an unbounded line. Correlate monotonically
increasing IDs; fail on malformed envelopes, unsolicited responses or unexpected
requests. Count known notifications under the absolute timeout, retaining only
sanitized categories. `notifications/tools/list_changed` aborts the experiment
instead of silently continuing under a changed contract. Close stdin and wait;
on timeout kill only the owned child, then await reaping. Never signal service PID.
- [ ] **Step 4: Verify transport with a synthetic subprocess.** The test-only
fixture speaks JSON-RPC on stdio, selected by hardcoded test scenarios: normal,
notification flood, oversized line, malformed JSON, wrong ID, EOF, timeout and
exit-1-on-stdin-close. It logs request method/tool/label for assertions, never
opens projects. Test shutdown/timeout reaping and no inherited secret variable.
Use Tokio duplex for most decoder tests; keep process tests for lifecycle/env.
- [ ] **Step 5: Run both module filters GREEN and inspect the diff.** A nonzero
bridge exit after validated responses is recorded separately, not relabeled as
failed RPC or exit-zero evidence. No host/service probe has run in this task.

### Task 3: Two Explicit Tool Adapters And Contract Checking

**Files:** `tools.rs`, `protocol.rs` handshake integration, `apple-contract.json`.

**Interfaces:** Produce typed observations, not production binding types:

```rust
pub struct OpenResult { pub id: String, pub path: Option<String> }
pub struct ReadResult {
    pub content: String, pub file_path: String,
    pub file_size: u64, pub total_lines: u64,
    pub lines_read: u64, pub start_line: u64,
}
pub enum MappingStatus { Observed, Unverified }
pub fn mapping_status(project: &FixtureProject, result: &OpenResult)
    -> anyhow::Result<MappingStatus>;
pub fn decode_open(result: &serde_json::Value) -> anyhow::Result<OpenResult>;
pub fn decode_read(result: &serde_json::Value) -> anyhow::Result<ReadResult>;
pub fn verify_sentinel(project: &FixtureProject, result: &ReadResult)
    -> anyhow::Result<()>;
#[async_trait::async_trait]
pub trait Peer: Send {
    async fn initialize(&mut self) -> anyhow::Result<()>;
    async fn open(&mut self, project: &FixtureProject) -> anyhow::Result<OpenResult>;
    async fn read(&mut self, project: &FixtureProject, id: &str)
        -> anyhow::Result<ReadResult>;
    async fn close(&mut self) -> anyhow::Result<ExitRecord>;
}
#[async_trait::async_trait]
pub trait PeerFactory: Send {
    async fn connect(
        &mut self, facts: &HostFacts, pair: &FixturePair,
        deadline: tokio::time::Instant,
    )
        -> anyhow::Result<Box<dyn Peer>>;
}
```

Implement `Peer for Bridge` and `LivePeerFactory`; fakes exist only in tests.
All shared type imports use the private modules that define them above.

- [ ] **Step 1: Write failing decoding and scope tests.**

```rust
#[test]
fn i1_optional_path_does_not_fabricate_mapping() {
    let payload = serde_json::json!({
        "isError": false,
        "structuredContent": {"workspaceIdentifier": "workspace1"}
    });
    let opened = decode_open(&payload).unwrap();
    assert_eq!(opened.id, "workspace1");
    assert!(opened.path.is_none());
    assert!(decode_open(&serde_json::json!({"isError": true})).is_err());
}
```

Add cases for null/empty ID, absent versus null/mismatched path, JSON-RPC error,
MCP `isError`, wrong/missing/negative read fields and conflicting structured/text
results. Assert fixed request bodies always carry the fixture path and returned ID.
- [ ] **Step 2: Observe RED.** Run `../scripts/cargo-managed test --locked -p acp --example xcode_headless_i1 tools::`.
- [ ] **Step 3: Derive metadata mechanically and implement adapters.** Read the
two preserved captures with a JSON parser; verify their saved SHA-256 values and
equal tool definitions. Write only open/read entries plus their capture digests
to `apple-contract.json`; do not hand-edit Apple's metadata. Resolve the source
captures from the approved checkout, not `/private/tmp` research helpers.

Initialize with `protocolVersion=2025-06-18`, empty capabilities, and client name
`Chainworks I1`/version `1`; send initialized once, list bounded pages and compare
the two input/output schemas structurally. Require unique tool names and pinned
server identity. Use these exact internal call shapes:

```rust
serde_json::json!({
    "name": "XcodeOpenWorkspace", "arguments": {"path": project.package}
});
serde_json::json!({
    "name": "XcodeRead",
    "arguments": {
        "workspaceIdentifier": id, "filePath": project.read_path,
        "offset": 1, "limit": 8
    }
});
```

The bridge instance receives the fixed pair allowlist on construction and
rejects any project outside it. No public raw-call method or arbitrary JSON CLI
argument is added. Decode `structuredContent`, or one JSON text item if that is
the actual result encoding; if both exist they must agree. Never treat free
text as a path/ID mapping. Unknown shapes stop with a bounded deviation record;
do not invent stable Apple permission-error codes from prose.

For a present path require canonical equality; mismatch/null/malformed path
stops that target. Absence remains unverified and allows only controlled sentinel
reads. Optional status mapping is used only if an actually observed, explicit
ID/path association has a tested parser; the currently captured empty list cannot
supply one. Leave unfamiliar populated shapes unverified rather than guessing.

Sentinel verification accepts one cat-n-style line with numeric prefix `1`, a
tab delimiter, and exactly the fixture marker. Preserve the bounded original
representation in private evidence; another representation is a contract
deviation requiring a test and deliberate adaptation. Require read path equality,
one returned line, start line 1, and metadata consistent with the seeded file.
- [ ] **Step 4: Run GREEN and add schema-drift tests.** Changed input/output
schema, server version, duplicate name, list-change notification or tool-list
overflow yields no `tools/call`. Missing workspace scope is never forwarded.
Run the tools and protocol filters; all tests use saved/synthetic data only.
- [ ] **Step 5: Record the adapter limits and review the diff.** No mutation
tools, general JSON Schema framework or 54-tool production manifest is built.

### Task 4: Durable Experimental Stop Rules And Controller

**Files:** `evidence.rs`, `controller.rs` and module declarations.

**Interfaces:** Consume `FixturePair`, `HostInspector`, `PeerFactory`, open/read
results. Define `ExperimentalWorkspaceObservation` with fixture label/requested
package, service generation, returned ID/path, mapping status and successful
read records. It has no conversion into a production binding/readiness ticket.

```rust
pub trait Recorder: Send {
    fn begin_open(&mut self, project: &FixtureProject, generation: &ServiceGeneration)
        -> anyhow::Result<()>;
    fn finish_open(&mut self, label: &str, result: &OpenResult) -> anyhow::Result<()>;
    fn unknown_open(&mut self, label: &str) -> anyhow::Result<()>;
}
pub enum CaseStatus { Pass, Fail, Blocked, NotRun, Unverified }
pub enum Outcome { Supported, Partial, Refuted, Blocked }
pub struct Report {
    pub cases: [CaseStatus; 8],
    pub observations: Vec<ExperimentalWorkspaceObservation>,
    pub unresolved_open: bool,
}
pub fn classify(report: &Report) -> Outcome;
pub async fn run_sequence(
    pair: &FixturePair, approved_host: &HostFacts, host: &mut dyn HostInspector,
    peers: &mut dyn PeerFactory, record: &mut dyn Recorder,
) -> anyhow::Result<Report>;
```

`LocalRecorder::create(private_dir: &Path) -> Result<LocalRecorder>` exclusively
creates its owned record directory. Intent/result files are versioned and retain
fixture label, selected generation, request digest and timestamps privately.
Opening an existing recorder for dispatch is prohibited; a read-only inspection
function may report its unresolved state but cannot resume it.

- [ ] **Step 1: Write failing durable-intent and sequence tests.**

```rust
#[test]
fn i1_open_intent_cannot_be_issued_twice() {
    let temp = tempfile::tempdir().unwrap();
    let mut record = LocalRecorder::create(&temp.path().join("record")).unwrap();
    let pair = create_pair(&temp.path().join("fixtures")).unwrap();
    let generation = ServiceGeneration {
        uid: 1001, pid: 42, start_sec: 1, start_usec: 0,
        developer_dir: "/fixture/Developer".into(),
        executable: "/fixture/Developer/Xcode Service".into(),
        bundle_id: "com.apple.dt.mcp-server".into(), build: "fixture".into(),
    };
    record.begin_open(&pair.projects[0], &generation).unwrap();
    assert!(record.begin_open(&pair.projects[0], &generation).is_err());
    assert!(LocalRecorder::create(&temp.path().join("record")).is_err());
}
```

Define test-only scripted implementations of the three traits. Their request
log contains typed `Open(label)`, `Read(label,id)` and `Close` events, and injected
host generations/evidence failures. Assert successful sequence is exactly two
opens, A/B/A reads, close/connect/initialize, then A/B/A reads with the same
returned IDs and no additional open. Stop before each next operation if authority
or a required result changes. Test IDs need not resemble production identifiers.
- [ ] **Step 2: Observe RED.** Run the `evidence::` and `controller::` example
filters separately through managed Cargo.
- [ ] **Step 3: Implement the one-shot record and sequence.** Use exclusive
0600 files under a 0700 private directory; hold an exclusive nonblocking local
kernel lock for the attempt. Sync intent and containing directory before open
bytes; result writes also sync. Never reuse or overwrite an attempt directory.
An existing intent without a validated terminal record means uncertainty even
if the final summary was never written. This is an experimental interlock, not
the production DB journal or cross-daemon coordinator.

```rust
record.begin_open(project, &facts.generation)?;
let result = peer.open(project).await;
match result {
    Ok(opened) => {
        record.finish_open(&project.label, &opened)?;
        observations.push(opened);
    }
    Err(error) => {
        let _ = record.unknown_open(&project.label);
        return Err(error);
    }
}
```

The unfulfilled intent itself remains authoritative when `unknown_open` cannot
write. A failed terminal-result write stops all subsequent dispatch. Cancellation
can drop an in-flight request but cannot remove the intent; killing a bridge is
not proof the service operation stopped. All sequence exits close/reap owned
bridges through one cleanup owner and record unresolved status. Never call open
from a retry/error branch. Reconnect is deliberate only after successful reads,
not a failure-recovery mechanism.

Reinspect service generation and IDE absence before every tool dispatch and
both sides of reconnect. Compare the first and subsequent observations to
`approved_host`, not merely to a new baseline taken after CLI approval checks.
Validate source/project digests before and after.
Set one overall deadline at sequence entry, pass it to both peer connections,
and use the smaller of request/overall deadlines. Reconnect never restarts the
budget. On a sequence error, the CLI still finalizes a failure/uncertainty report
from the retained records; failure to write that report leaves intent records
and a non-success exit, never a default Supported verdict.
Record only fixture-matching status mapping; no foreign workspace inventory is
persisted. Export known fixture paths as labels, opaque IDs as local aliases,
and unexpected fields/errors as categories plus hashes, not raw strings.
- [ ] **Step 4: Fault-inject every boundary and run GREEN.** Cover failed intent
creation/sync (zero open calls), crash after intent (unresolved, no second open),
lost response, partial stdin write, malformed terminal response, result-sync
failure, cancellation, changed service/IDE appearance, conflicting mapping,
duplicate IDs for different fixtures and reconnect failure. Count dispatches,
not only errors. Inject tests against owned fake transports/files only; do not
probe service path races or third-party projects. Also verify public output
does not contain a planted foreign path, raw error secret or environment value.
- [ ] **Step 5: Review outcome classification.** Supported requires all eight
cases pass, including structured mapping and offline proofs; uncertainty forbids
Supported. Routing success with unverified mapping/reconnect deviation is
Partial. Contradictory path/data is Refuted. Missing permission/host prerequisite
is Blocked. Untested cases remain NotRun, never default Pass.
An uncertain transport outcome alone does not refute H1; preserve Blocked or
Partial with uncertainty according to the subset actually observed.

### Task 5: Explicit CLI, Offline Regression And Approval Package

**Files:** `cli.rs`, entry/module wiring, fake subprocess integration tests;
future offline evidence under the I1 attempt directory. No daemon registration.

**Interfaces:** Produce `cli(args: Vec<OsString>) -> Result<()>` and three closed
subcommands. `PreparedAttempt` is the private JSON record containing schema
version, UUID, generated fixture paths/digests, source identity and evidence path.
`Inspection` adds current generation, launch identity/executable digests and
status. SHA-256 of the exact inspection file bytes is the approval comparison;
it is not a production JCS authority digest or OS permission grant.

- [ ] **Step 1: Write failing CLI and approval tests.**

```rust
#[tokio::test]
async fn i1_cli_does_not_accept_arbitrary_tool_calls() {
    let args = vec!["run".into(), "--tool".into(), "XcodeWrite".into()];
    assert!(cli(args).await.is_err());
}
```

Add tests that default/no subcommand does nothing beyond usage, prepare never
constructs a host/bridge, inspect never constructs a bridge, and run rejects
missing approval, edited fixtures, a changed source/launch identity or an
already-attempted record before any open.
- [ ] **Step 2: Observe RED.** Run `../scripts/cargo-managed test --locked -p acp --example xcode_headless_i1 cli::`.
- [ ] **Step 3: Implement the closed CLI.**

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    harness::cli(std::env::args_os().skip(1).collect()).await
}
```

`prepare --scratch-parent PATH --evidence-root PATH` generates a UUID, a fresh
fixed-layout fixture pair and private attempt record without Apple calls.
Reject scratch locations within the checkout, `.chainworks`, existing run roots
or symlink descendants; canonicalize a system temporary-directory alias before
checking it. Only fixtures generated by this prepare path can reach live run.

`inspect --attempt-file PATH` performs bounded read-only host/status/signature
inspection and writes `Inspection`. It prints a redacted summary and privileged
fixture/cleanup locations separately for explicit authorization. Failed identity
or disabled/unsafe status produces Blocked, no bridge or permission command.

`run --attempt-file PATH --approved-plan-digest HEX` verifies that digest and
the exact current fixture/source/executable/service identities, acquires the
attempt interlock, and invokes `run_sequence`. No status change or new consent
silently refreshes an approval. The flag records the user's reviewed scope; it
does not fabricate OS consent. User approval must actually precede the command.
Derive the interlock/record path from the generated fixture root, not a
caller-editable alternative recorder location. A copied attempt file cannot
reopen those same fixtures. Compare stable launch identities, not the expected
new harness PID of the run invocation; compare the service generation exactly.

Emit a bounded redacted report with per-case outcomes and an evidence manifest;
keep cleanup paths and raw host identity private. Source provenance includes
HEAD plus sorted file/digest records for all build inputs, including dirty and
new example files, Cargo files and capture inputs. No dirty build is labeled
solely by HEAD. Changed code invalidates the inspection digest/approval.

- [ ] **Step 4: Run full offline verification.** All commands below run from
`control-plane`; tests must neither connect to Apple nor start a provider:

```bash
../scripts/cargo-managed test --locked -p acp --example xcode_headless_i1
env -u CHAINWORKS_JUNIE_ACP_LIVE_SMOKE ../scripts/cargo-managed test --locked -p acp --lib --tests
../scripts/cargo-managed check --locked -p acp --example xcode_headless_i1
../scripts/cargo-managed fmt --all -- --check
```

Keep ignored tests ignored. If unrelated baseline formatting fails, record it
without rewriting unrelated files. Extend the fake subprocess to cover the
complete two-connection sequence and prove no open after reconnect/error. Check
that no production source exports/imports the example and no Cargo default
binary or dependency was added. Capture exact selections, exit statuses, source
digest and test results as offline evidence; do not infer live I1-02 through
I1-06 from these tests.
- [ ] **Step 5: Review the complete implementation and pause at the live gate.**
Record remaining deviations and seek the chosen execution method's final code
review. Do not run Task 6 merely because offline tests pass. No automated retry,
service permission mutation, deployment or commit is part of this handoff.

### Task 6: Separately Authorized Live Experiment And Closeout

**Files:** Generated per-attempt evidence only:
`docs/evidence/xcode-headless/iteration-1/<attempt-id>/experiment.md`, bounded
redacted exchanges, offline evidence references and file-digest manifest.
Private scratch/control records remain outside the repository. No source edits
during an approved live attempt; changes require a new inspection/approval.

- [ ] **Step 1: Prepare and inspect the actual fixture pair.** After plan approval,
implementation and offline review, from `control-plane`:

```bash
../scripts/cargo-managed run --locked -p acp --example xcode_headless_i1 -- prepare --scratch-parent /private/tmp --evidence-root ../docs/evidence/xcode-headless/iteration-1
../scripts/cargo-managed run --locked -p acp --example xcode_headless_i1 -- inspect --attempt-file "${ATTEMPT_FILE:?set from the prepare output}"
```

`ATTEMPT_FILE` is the exact private path returned by prepare, not a chosen live
project. The executable prints the resolved paths, observed launch identity and
inspection digest. If sandbox visibility is insufficient, record unknown and
use the normal permission-review path for this specific observation; do not
interpret sandbox failure as a disabled service.
- [ ] **Step 2: Obtain explicit user authorization for this inspection.** Present
the two fixture paths, actual development launch identity, expected two opens,
six reads, bridge reconnect, possible consent/indexing, no automatic replay and
left-open fixtures. Ask for the live action, not a blanket folder/agent grant.
Handle any OS consent through the operator; never run approve/allow-folder/reset.
If denied or unavailable, record Blocked without a tools call and close I1 with
that limitation if the user chooses not to proceed.
- [ ] **Step 3: Run once under the approved scope.** Only after that authorization:

```bash
../scripts/cargo-managed run --locked -p acp --example xcode_headless_i1 -- run --attempt-file "${ATTEMPT_FILE:?}" --approved-plan-digest "${APPROVED_PLAN_DIGEST:?set from the approved inspection}"
```

Observe the normal H1 sequence, not an adversarial cross-project test. On a
denial/timeout/discrepancy stop; do not rerun this command to collect a pass.
Never stop another bridge/service or open the IDE to recover.
- [ ] **Step 4: Read back evidence and report retained state.** Verify owned
children reaped, source/project digests unchanged, IDE absence before/after,
recorded service identity, metadata additions, open-intent outcomes and the
remaining fixture/workspace paths. Missing post-state stays unknown. Redact
before placing evidence in the repository, then verify every manifest digest.
- [ ] **Step 5: Evaluate H1 and update documentation from evidence.** Combine
offline I1-01/07/08 and live I1-02 through I1-06 without substituting one for the
other. Record Supported/Partial/Refuted/Blocked, concrete expected/actual deltas
and next decision. An adapted parser, fixture layout or hypothesis needs an
explicit new revision and regression case; retain the failed observation.
Update the parent/disposition only now, with new hashes, without retroactively
changing the reviewed R3 evidence. Do not mark I2-I4, P1-05 or any future P2 fixed.

## Coverage And Execution Handoff

| I1 requirement | Implementation / verification |
| --- | --- |
| I1-01 exact root/project | T1 selection and same-named fixture tests |
| I1-02 warm attach/IDE absent | T2 synthetic identity checks; T6 actual host evidence |
| I1-03 two bounded opens | T3 exact adapters; T4 intent-before-send; T6 observed results |
| I1-04 mapping separate from routing | T3 optional/conflicting path cases; T6 observed mapping or Unverified |
| I1-05 explicit A/B/A reads | T3 sentinel validation; T4 scripted ordering; T6 actual reads |
| I1-06 reconnect without reopen | T2 child lifecycle; T4 sequence; T6 actual second bridge |
| I1-07 denials/uncertainty | T2/T3 failures; T4 fault boundaries and dispatch counts |
| I1-08 source/runtime/privacy boundaries | T1 seed digests; T4 redaction/cleanup; T5 no production wiring; T6 post-state |
| Evidence, adaptation and closeout | T5 source/test evidence; T6 manifest and per-case decision |

The implementation lane can finish T1-T5 without live authorization; it must
report H1 not run at that boundary. A blocked/partial live attempt can complete
an experiment with evidence but cannot confirm H1 or release readiness.

Plan self-review must verify coverage, interface names, bounds, absence of
production wiring, and separation of the live gate before handoff. Recommended
execution is Native: these five implementation tasks share private experimental
interfaces and make no production change; one complete independent code review
is still required before live use. Subagent-driven execution is an alternative
with per-task implementation/review. The user must review this plan and select
one method before implementation begins.
