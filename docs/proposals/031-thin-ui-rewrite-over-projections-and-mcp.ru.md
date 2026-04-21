# Proposal 031: Переписывание тонкого UI поверх проекций и MCP

| Поле | Значение |
|---|---|
| Дата | 2026-04-01 |
| Статус | Draft - проход усиления контрактов 2026-04-17 |
| Автор | Engineer (single-engineer project) |
| Зависит от | Proposal 027, Proposal 029, Proposal 041, [implemented local daemon lifecycle contract](../reference/local-daemon-lifecycle-supervision-and-packaging.md), Proposal 043 |
| Цель | Выполнить первый видимый пользователю cutover от клиентской workflow-логики к тонкому macOS operator UI поверх GraphQL read models и MCP-backed control commands. |
| Канонический proof lane | Зарегистрированный gate `proposal-031|p031` является единственным доказательством завершения. Ручной bundle из §9.3 является только fail-closed preflight/hold evidence. |

## 1. Зачем существует это proposal

Когда server parity доказан, жизненный цикл daemon productized, GraphQL read contracts явно описаны, а MCP доступен как control surface, UI должен перестать вести себя как полуавтономный workflow runtime.

После P031 macOS app является:

- renderer server-owned read models,
- инициатором audited control commands,
- местом для operator ergonomics и inspection,
- не местом, где решается workflow truth.

P031 является первым proposal в этом roadmap, которое меняет видимую пользователю границу ownership. Поэтому его небезопасно реализовывать по лозунгам. Этот proposal фиксирует read contracts, action contracts, Swift teardown map и cutover proof lane, необходимые для engineering handoff.

## 2. Необсуждаемая позиция по зависимостям

Реализация P031 не должна начинаться, пока все четыре prerequisite lanes не имеют именованного evidence на целевом tree.

| Зависимость | Required evidence перед реализацией P031 | Почему это блокирует P031 |
|---|---|---|
| P027 control-plane runtime foundation | Текущий Rust control-plane gate остается зеленым на том же tree, что и работа P031. | Thin UI не может делегировать truth нестабильному daemon/control-plane stack. |
| P029 MCP northbound auth/control | `./scripts/test-gate.sh proposal-029-mcp` проходит, включая MCP HTTP/stdio auth, capability filtering, journal IDs, GraphQL auth bridge и resource read checks. | P031 маршрутизирует operator mutations через MCP и не должен обходить audited command surface. |
| P041 parity harness | P041 parity artifact для текущих run/stage/approval/artifact/report surfaces зеленый или явно waived с датированным rollback plan. | User-visible cutover невозможен, если server behavior не доказан эквивалентным старому client-owned runtime. |
| Local daemon lifecycle | [Implemented local daemon lifecycle contract](../reference/local-daemon-lifecycle-supervision-and-packaging.md) существует, а `docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log` содержит green `proposal-042` gate evidence для launch, reconnect, unavailable daemon и stale projection states. | Thin UI зависит от daemon availability и нуждается в предсказуемом degraded UX. |
| P043 query/projection contract | P043 read contract landed, либо P031 содержит concrete surface matrix в §5 как binding implementation contract. | UI не может безопасно рендерить server truth без правил fields/freshness/inference. |

Если какая-либо зависимость red, P031 может готовить non-shipping components за feature flags, но не может заменять user-visible production path.

## 3. Ключевые решения

### 3.1 Единый read path

GraphQL является каноническим client read path. UI может использовать GraphQL queries и subscriptions, но не должен читать workflow truth из SwiftData, local compiled plans, local recovery coordinators, filesystem artifacts или MCP report payloads как альтернативных источников truth.

MCP остается control path. MCP read tools могут существовать для agent/operator tooling, но macOS UI читает через GraphQL.

### 3.2 Единый control path

Каждая P031-owned operator mutation должна либо:

- маршрутизироваться через зарегистрированный MCP tool, который попадает в shared command owner и возвращает ожидаемое audit/journal behavior,
- либо быть disabled/deferred в UI с явным owner proposal.

P031 не вводит second-wave MCP tools, если action matrix не помечает действие как `P031-owned` и не задает для него command/capability/journal owner. Значение по умолчанию для отсутствующих MCP tools: **defer and disable**, а не local fallback.

### 3.3 Disposable client state

SwiftData и Swift services могут оставаться только для presentation state, previews или явно ограниченного local cache. Они не должны решать:

- next stage,
- retry legality,
- recovery action,
- settlement truth,
- runtime/session truth,
- artifact/report hierarchy truth.

Любая сохраненная local model должна быть безопасна для удаления без изменения workflow behavior.

## 4. Scope UI surfaces

P031 владеет первым thin-client cutover для следующих surfaces:

| Surface | P031 status | Notes |
|---|---|---|
| Runs home | In scope | Заменить SwiftData `@Query` run truth на GraphQL read model. |
| Run detail | In scope | Рендерить run status, stage summaries, approvals, artifacts, reports из server read models. |
| Stage detail | In scope | Должен использовать server-owned stage projection/readback; без local stage reconstruction. |
| Approval inbox | In scope | Читать pending approvals из server projection; resolve через MCP. |
| Artifact viewer | In scope | Просматривать server artifact index и получать artifact content через server read path. |
| Report viewer | In scope | Рендерить persisted reports/evidence; без local report reconstruction. |
| Experiment comparison view | Deferred for mutation/creation; read-only placeholder allowed | Текущая MCP surface не имеет experiment command owner. UI может показывать только disabled entry state. |
| Daemon lifecycle / adapter state | Daemon lifecycle implemented through `daemonStatus` and `daemonStatusChanged`; broader adapter runtime state deferred unless P043 or a future owner lands a read model | Текущая первая волна MCP/P029 не определяет generic runtime-health tools или adapter-state GraphQL read model beyond daemon lifecycle. |

## 5. GraphQL read-model matrix

Эта matrix является binding для реализации P031. Если при реализации выясняется, что query/type сейчас не exposes required field, правильное исправление - расширить server read model или явно defer UI surface; client не должен infer missing truth.

| UI surface | GraphQL entrypoint | Server owner | Required fields | Freshness / subscription | Forbidden client inference | P031 implementation rule |
|---|---|---|---|---|---|---|
| Runs home | `runs(ideaID:)` and/or `runs` | `db::repos::projections::{list_active_projection,list_by_idea_projection}` -> `GqlRun` | run id, idea id, status, workflow title/id, started/completed timestamps, cancellation summary, current state, delivery/preflight summary where present | Initial query on load; refresh on foreground/reconnect; subscribe to `runStatusChanged(runID:)` only after a run is selected or when a list-level stream exists | Не вычислять active/completed/cancelled state из local SwiftData rows или artifact presence | Заменить `RunsHomeView` `@Query` truth на этот read model. SwiftData может только cache last-rendered presentation, если это feature-flagged и invalidated by server state. |
| Run detail header | `run(id:)` plus `runs` projection where list-derived counters are needed | `db::repos::runs::find_by_id` for canonical run row; projection row for list/detail summaries | run id, status, workflow metadata, workspace/artifact roots, cancellation log, delivery configuration, preflight, frozen metadata/snapshot hashes | Query on navigation; refresh on command completion and `runStatusChanged` | Не derive cancellation settlement, delivery state, drift или frozen metadata из local files | Если `run(id:)` lacks projection-only field required by UI, добавить GraphQL field или pair with projection query; не reconstruct in Swift. |
| Stage list / progress | `stages(runID:)` after server is updated to return projection-backed stage rows or equivalent typed projection fields | Target owner: `db::repos::projections::list_stages_projection`; current gap: `schema.rs` canonical `stages` path leaves projection-only fields empty | stage execution id, stage id, label, status, attempt, settlement kind, owner agent/provider/model, `hasArtifacts`, `hasPendingApproval`, `hasValidationFailure`, evidence/recovery availability | Query with run detail; refresh on `stageEvents(runID:)`; stale badge if stream disconnects | Не infer pending approval из button state, artifacts из filesystem, validation failure из report text или retry legality из local rules | P031 должен либо переключить GraphQL `stages(runID:)` на projection-backed conversion, либо добавить новую stage projection query. Stage detail не может ship на canonical rows с empty projection flags. |
| Stage detail | `stage(id:)` plus `agentExecutions(stageExecutionID:)`; projection-backed stage summary must be available for flags | `db::repos::stages`, `db::repos::agent_executions`, and stage summary projection | canonical stage fields, projection flags, agent execution provenance, MCP truth, validation/evidence/recovery fields | Query on selection; refresh on stage event; show stale/projection-lag state if projection timestamp lags canonical update | Не compute retry/reset/resume eligibility в Swift; render server action availability once available or disable action | Stage detail может combine canonical details with projection flags, но все UI decisions должны приходить из server fields. |
| Approval inbox | `approvalInbox` | `db::repos::projections::list_pending_inbox_projection` / approval inbox projection | approval id/run id/stage id, requested timestamp, status, blocking reason, required operator decision labels | Query on view load and after approve/reject; subscribe when approval stream exists; otherwise poll/refresh bounded by P043 | Не infer approval need только из stage status | Resolve только через `approvals.resolve`; без direct Swift approval mutation. |
| Artifact viewer | `artifacts(runID:)` | artifact index projection / `db::repos::projections::list_artifacts_projection` | artifact id, run/stage/agent ids, name, format, path/URI, checksum/size, report kind/version, pinned flag | Query on run/stage selection; refresh on stage completion or artifact event when available | Не scan artifact directories из client и не infer hierarchy по filename conventions | UI просматривает только server artifact hierarchy. Direct file access может быть explicit open/export affordance после server selection, но не source of truth. |
| Report viewer | `artifacts(runID:)` filtered by report fields, and/or dedicated report read query when P043 adds it | report/artifact projection plus validation/evidence readback owner | report kind/version, validation failure payload availability, failed-stage evidence availability, release report entries, MCP execution truth readback if exposed | Query on report view load; refresh on run completion/stage failure; show stale state when report generation pending | Не reconstruct report/evidence из raw artifacts или local transcript files | Если report payload недоступен через GraphQL, P031 должен add/read server query before shipping report view. MCP `reports.get` не является macOS UI read path. |
| Experiment comparison | Deferred read contract | Future comparison/read projection owner, not P031 first wave | comparison id, compared run ids, metrics, verdict, artifacts | N/A until owner proposal | Не compare runs locally из partial run/artifact state | Только hide или disabled-placeholder. No local compare engine in P031. |
| Daemon lifecycle / adapter state | `daemonStatus`; `daemonStatusChanged` for lifecycle stream. Broader adapter state remains deferred unless P043 or a future owner adds a read model. | Local daemon lifecycle contract / future runtime health read model | daemon reachable/degraded/offline, lifecycle mode, start/stop failure metadata, last change, adapter availability if later owned | Foreground/reconnect and lifecycle stream if available | Не infer daemon health только из random query failures | P031 должен render unavailable/degraded state из the daemon lifecycle read model или keep broader adapter health out of scope. |

### 5.1 Freshness и stale-state rules

UI должен явно маркировать read freshness:

- `live`: latest query succeeded и active subscription/refresh path healthy.
- `refreshing`: user-visible state is last known server state while a refresh is in flight.
- `stale`: last refresh failed, subscription disconnected или daemon lifecycle reports degraded/offline.
- `unavailable`: daemon cannot be reached или auth/session invalid.

Для stale/unavailable states destructive или state-changing actions disabled, если action matrix явно не помечает их как safe while stale. UI может still allow inspection of last-rendered data, но обязан визуально помечать это как stale.

## 6. MCP operator action matrix

Эта matrix является binding для каждого P031-owned action. UI не должен expose enabled controls для deferred actions.

| UI action | MCP tool | Command/direct owner | Journal/audit behavior | Capability ID | P031 status | UI behavior |
|---|---|---|---|---|---|---|
| Start run | `runs.start` | `Command::StartRun` via `CommandHandler` | Returns `journal_id`; command journal caller surface `mcp`; delivery/preflight truth persists server-side | `CapabilityToolId::RunsStart` | In scope | Enabled only when daemon is live, user principal has capability, and required inputs are valid. |
| Cancel run | `runs.cancel` | `Command::CancelRun` via `CommandHandler` | Returns `journal_id`; cancellation settlement remains server truth | `CapabilityToolId::RunsCancel` | In scope | Enabled for cancellable runs according to server-provided action availability or conservative UI rules that never assert success locally. |
| Approve stage | `approvals.resolve` with `decision=granted` | `Command::ApproveStage` via `CommandHandler` | Returns `journal_id`; approval resolution stored server-side | `CapabilityToolId::ApprovalsResolve` | In scope | Enabled only from pending approval read model. |
| Reject stage | `approvals.resolve` with `decision=rejected` | `Command::RejectStage` via `CommandHandler` | Returns `journal_id`; approval resolution stored server-side | `CapabilityToolId::ApprovalsResolve` | In scope | Enabled only from pending approval read model. |
| Retry stage | `stages.retry` | `Command::RetryStage` via `CommandHandler` | Returns `journal_id`; retry attempt/stage truth is server-owned | `CapabilityToolId::StagesRetry` | In scope | Enabled only when server read model exposes retryable state or a pending failed/blocked state that the server will validate. |
| Queue Steward analysis | `steward.run_analysis` | `Command::RunStewardAnalysis` via `CommandHandler` | Returns `journal_id`; analysis work item server-owned | `CapabilityToolId::StewardRunAnalysis` | Optional in scope if Steward UI is visible | Hidden unless Steward surface is present and principal has capability. |
| List/get runs | `runs.list`, `runs.get` | Direct MCP read helpers | No `journal_id`; not macOS UI canonical read path | `RunsList`, `RunsGet` | Tooling only | UI uses GraphQL instead; MCP reads may remain for CLI/agents. |
| List approvals | `approvals.list` | Direct MCP read helper | No `journal_id`; not macOS UI canonical read path | `ApprovalsList` | Tooling only | UI uses GraphQL `approvalInbox`. |
| Get reports | `reports.get` | Direct MCP report helper | No command journal; report readback only | `ReportsGet` | Tooling only for P031 | UI report view must use GraphQL/server read query, not MCP. |
| Reset agent/session | None in first-wave MCP | P029 defers `sessions.reset_agent` / session lifecycle commands | N/A until owner proposal | No current ID | Deferred | Existing Swift reset controls disabled/removed from P031-owned screens until MCP owner lands. |
| Resume/repair local recovery | None in first-wave MCP | Current Swift services only; no P031 command owner | N/A | No current ID | Deferred or removed | Recovery sheets must not call local `RecoveryCoordinator`/`ExecutionService`; show server-owned retry/reject/cancel actions only. |
| Clone run | None in first-wave MCP | Deferred future clone proposal | N/A | No current ID | Deferred | Hide/disable clone affordance. |
| Compare runs | None in first-wave MCP | P029 defers `reports.compare`/comparison tooling | N/A | No current ID | Deferred | Read-only placeholder allowed; no local compare execution. |
| Run experiment | None in first-wave MCP | Future experiment owner | N/A | No current ID | Deferred | Hide/disable experiment launch. |
| Runtime health action | None in first-wave MCP | Local daemon lifecycle read model, P043 health read model, or future tool | N/A | No current ID | Deferred | Read-only daemon health banner may use the implemented daemon lifecycle read model; broader runtime health remains deferred until owned. |

### 6.1 Command result handling

Для in-scope MCP command tools:

- UI displays command accepted/failed based on MCP response, not local mutation side effects.
- UI records/links `journal_id` when returned.
- UI refreshes GraphQL read model after command completion.
- UI does not optimistically mutate workflow truth; it may show temporary pending UI state clearly labeled as pending server confirmation.
- Capability denial, unknown tool, unauthorized или daemon unavailable responses produce visible non-destructive error states and leave local read model unchanged.

## 7. Swift local-state teardown and ownership map

Реализация P031 должна включать Swift inventory update, который maps current owners to final ownership. Таблица ниже - initial required map from current evidence.

| Current owner | Current responsibility | P031 target | Required change | Guardrail |
|---|---|---|---|---|
| `RunsHomeView` `@Query` / SwiftData run source | Runs list/home state and local sheet routing | GraphQL `runs` read model | Replace workflow truth reads with GraphQL view model. Keep only selection, presentation filters, and transient sheet state locally. | Static/search check fails if P031-owned Runs Home uses `@Query` for workflow truth. |
| SwiftData run/stage/approval/artifact models | Local persisted workflow truth/cache | Presentation/cache only or delete | Any retained models must be derived from server read models and safe to delete. | Tests/previews cannot be the only source of production state. |
| `RecoverySheet` | Direct retry/resume/reset/clone actions via local services | MCP-backed in-scope actions only; deferred actions disabled | Replace retry with `stages.retry`; approve/reject/cancel routed to MCP where surfaced; reset/resume/clone removed/deferred. | No direct `RecoveryCoordinator`, `RunPlanCompiler`, or `ExecutionService` mutation call from P031-owned screen. |
| `BlockedRunRecoveryView` | Direct recovery mutation path | Same as `RecoverySheet` | Remove local mutation path; render server-owned recovery/evidence state and MCP-supported actions. | Static/search guard for direct service calls. |
| `RecoveryCoordinator` | Client-owned recovery orchestration | Out of P031-owned UI path | Keep only for legacy feature-flag rollback if needed; not imported by thin screens. | Feature flag defaults to thin path; legacy path has rollback owner and expiry. |
| `RunPlanCompiler` | Local workflow plan compilation | Server/control-plane owner | Remove from thin UI path. UI may display workflow metadata from GraphQL only. | No compile call from P031-owned UI. |
| `ExecutionService` | Client-owned execution/mutation runtime | Server/MCP command owner | Remove from P031-owned UI. | No start/retry/resume/reset/clone call from P031-owned UI. |
| Comparison/report sheets | Local compare/report presentation and sheet state | GraphQL report read models; comparison deferred | Report viewer uses server read path; comparison launch disabled until owner lands. | No local comparison as workflow truth. |
| Previews/test seeds | Local sample data for SwiftUI previews/tests | Presentation-only fixtures | Keep only as fixtures; must not imply production source of truth. | Names and docs mark preview-only state clearly. |

### 7.1 Transition and rollback

P031 может использовать feature flag для staged rollout:

- `legacy`: existing SwiftData/local-service path remains for rollback only.
- `thin-read`: GraphQL read models drive P031 screens; local mutation path disabled.
- `thin-control`: GraphQL reads plus MCP commands drive P031 screens.

Rollback may return from `thin-control` to `thin-read` only by disabling MCP action affordances. Rollback to `legacy` is allowed only before deleting legacy state/services and must be documented in the release note. No rollback mode may write conflicting workflow truth while thin mode is active.

## 8. UX and degraded-state contract

P031 должен preserve operator confidence while removing local intelligence.

| State | UI behavior | Action behavior |
|---|---|---|
| Daemon live | Render live GraphQL read models; show normal action availability. | In-scope MCP actions enabled by capability/action state. |
| Daemon reconnecting | Keep last server state visible with `refreshing` label. | Disable destructive/state-changing actions. |
| Daemon unavailable | Show last known state as `stale` if available; otherwise show empty unavailable state with recovery guidance. | Disable all MCP actions; offer reconnect/open diagnostics only. |
| Projection lag | Show canonical run status if available plus explicit `projection updating` label for projection-derived fields. | Disable actions that depend on missing/stale projection flags. |
| MCP unauthorized/capability denied | Show action-level error and principal/capability hint. | Do not retry automatically; do not mutate local state. |
| Command accepted but read model not refreshed yet | Show pending command receipt with `journal_id`. | Keep previous read model until GraphQL confirms new truth. |

## 9. Cutover gate and evidence bundle

### 9.1 Hold criteria

Реализация P031 должна stop перед user-visible cutover, если любое из следующего true:

- `proposal-029-mcp` is red.
- P041 parity evidence is missing or red for run/stage/approval/artifact/report surfaces.
- local daemon lifecycle evidence is missing for live/reconnect/unavailable states.
- P043/P031 read matrix fields are not exposed through GraphQL.
- Any P031-owned enabled operator mutation bypasses MCP.
- Any P031-owned screen reads workflow truth from SwiftData or direct local services.
- UI degraded-state smoke fails for daemon unavailable or stale projection states.

### 9.2 Rollback criteria

Rollback from thin mode is required if:

- GraphQL read model diverges from server projection/canonical truth in a P041 parity check.
- MCP command succeeds but GraphQL refresh cannot observe resulting server truth within the P043 freshness budget.
- Daemon lifecycle causes repeated unavailable state on normal app launch.
- Operator can trigger a local client-owned mutation from a P031-owned screen.

Rollback action is feature-flag downgrade to `thin-read` or `legacy` depending on migration phase, plus disabling affected MCP action affordances until the server/read contract is fixed.

### 9.3 Canonical proof lane

Реализация P031 должна добавить `proposal-031|p031` в `scripts/test-gate.sh`.

Пока этот gate не существует, для P031 нет green completion path. Ручной bundle ниже является только fail-closed preflight bundle: каждая команда должна пройти, и каждый prerequisite evidence artifact уже должен существовать с green/ready verdict. Missing artifacts, TODO placeholders, shell comments или deferred prerequisite evidence являются red.

Required prerequisite evidence artifacts:

| Artifact | Required proof |
|---|---|
| `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p041-parity.md` | P041 run, stage, approval, artifact, and report parity evidence is green for the read surfaces P031 consumes. |
| `docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log` | Local daemon live, reconnecting, unavailable, and degraded/offline lifecycle evidence is green for P031 UI states; `proposal-042` is retained only as the historical gate alias. |
| `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p043-read-contract.md` | P043/P031 GraphQL read-contract evidence is green for every field named in the §5 matrix. |

Manual preflight bundle:

```bash
./scripts/test-gate.sh proposal-029-mcp
./scripts/test-gate.sh ui-smoke
test -s docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p041-parity.md
test -f docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log
test -s docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p043-read-contract.md
rg -q 'Status: \\(Ready\\|Ready with Risks\\)' docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p041-parity.md
rg -q "Proposal 042 control-plane gate passed" docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log
rg -q 'Status: \\(Ready\\|Ready with Risks\\)' docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p043-read-contract.md
```

Итоговый gate `proposal-031` должен включать:

| Gate bucket | Required proof |
|---|---|
| GraphQL read contract | Runs home, run detail, stage list/detail, approval inbox, artifact viewer, and report viewer queries return the matrix-required fields from server read models. |
| Projection parity | GraphQL readbacks match DB projection rows for runs/stages/approvals/artifacts/reports where a projection owner exists. |
| MCP action routing | Start/cancel/approve/reject/retry actions call MCP tools, return/handle `journal_id`, and refresh GraphQL state. |
| Deferred action disablement | Reset, resume, clone, compare, experiment, and runtime-health actions are hidden/disabled unless their owner proposal has landed. |
| Swift teardown/static guard | P031-owned screens do not import/call `RecoveryCoordinator`, `RunPlanCompiler`, `ExecutionService`, or SwiftData `@Query` as workflow truth. |
| Daemon degraded UX | App renders live, refreshing, stale, unavailable, unauthorized, and command-pending states. |
| UI smoke | Runs home -> run detail -> stage detail -> approval/retry/cancel/report inspection smoke path. |

## 10. Migration plan

### Phase 1 - Read-only thin screens

- Build GraphQL-backed view models for in-scope read surfaces.
- Keep legacy path behind feature flag.
- Do not enable MCP mutations yet unless read refresh is proven.
- Add stale/unavailable UI states.

### Phase 2 - MCP control cutover

- Route in-scope actions through MCP.
- Display `journal_id` / command receipt where returned.
- Disable deferred actions.
- Refresh GraphQL read models after command completion.

### Phase 3 - Local truth teardown

- Remove or quarantine SwiftData workflow truth stores from P031-owned screens.
- Remove direct local service calls from P031-owned screens.
- Keep only presentation state, previews, and feature-flagged rollback code with an expiry.

## 11. Non-goals

P031 **не**:

- переопределяет workflow semantics,
- добавляет new orchestration behavior,
- выбирает runtime backends,
- retires GraphQL mutations from the control plane,
- implements deferred MCP tools for sessions, experiments, comparison, clone, or runtime health unless a later owner proposal lands first,
- creates a second control plane,
- or makes MCP the macOS UI read path.

## 12. Acceptance criteria

P031 complete, когда:

1. Every in-scope UI read surface renders from the §5 GraphQL read-model contract.
2. Stage list/detail either consumes projection-backed stage rows or documents and tests a server-owned equivalent that fills projection decision fields.
3. Every enabled P031-owned operator mutation routes through an in-scope MCP tool from §6.
4. Deferred actions are hidden/disabled with clear operator messaging.
5. P031-owned screens no longer use SwiftData or direct Swift services as workflow truth.
6. Removing the client does not destroy workflow truth or prevent the daemon/control plane from progressing.
7. The UI exposes live/refreshing/stale/unavailable/pending-command states from §8.
8. The registered `proposal-031|p031` gate exists in `scripts/test-gate.sh` and `./scripts/test-gate.sh proposal-031` is green on the same tree.
9. Rollback/hold criteria from §9.1 and §9.2 are documented in the release handoff.

## 13. Final recommendation

P031 должен сделать UI намеренно меньшим по ответственности и более ясным по поведению.

Proposal является implementation-ready только когда engineers can trace every visible field to a GraphQL read owner, every enabled action to an MCP command owner, and every removed local path to a Swift teardown item. Система больше не должна зависеть от того, что client жив, корректен и stateful для workflow execution truth.
