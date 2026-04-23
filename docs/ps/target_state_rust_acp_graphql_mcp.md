# Target State: Rust Control Plane + ACP Runtimes + GraphQL Thin Client + MCP Northbound Control

## 1. Цель документа

Зафиксировать **целевое состояние системы** после архитектурного поворота.

Целевая система должна состоять из:

- **локального Rust-сервера** как единственного владельца доменной логики и orchestration truth,
- **ACP-адаптеров** как southbound интерфейса к агентным рантаймам,
- **GraphQL** как единственного client-facing API для тонкого SwiftUI-клиента,
- **MCP-сервера** как northbound control-plane интерфейса для внешнего управления, автоматизации, создания идей и запуска workflow,
- **SwiftUI-клиента** как тонкой оболочки над серверными projections и командами, без собственной workflow-логики.

---

## 2. Краткая формула целевого состояния

> **Rust daemon = мозг системы**  
> **ACP = южный интерфейс к агентам**  
> **GraphQL = единственный интерфейс для SwiftUI-клиента**  
> **MCP = внешний control plane для агентов и автоматизаций**  
> **SwiftUI = view layer, а не источник workflow-логики**

---

## 3. Базовые принципы

### 3.1 Сервер — единственный владелец истины
Ни UI, ни ACP-рантайм, ни MCP-клиент не владеют workflow truth.

Только Rust-сервер является владельцем:
- run truth,
- stage truth,
- approval truth,
- artifact metadata truth,
- recovery truth,
- session lineage truth,
- runtime binding truth,
- run compaction truth,
- archive / supersession truth.

### 3.2 ACP — транспорт к агентам, но не источник доменной логики
ACP-рантаймы отвечают за:
- запуск агентной сессии,
- prompt lifecycle,
- tool execution,
- stream updates,
- runtime permission surface,
- session state внутри конкретного рантайма.

ACP-рантаймы **не** отвечают за:
- выбор следующего stage,
- approval semantics,
- retry legality,
- recovery policy,
- итоговую report truth,
- domain orchestration.

### 3.3 GraphQL — единственный client-facing API для SwiftUI
SwiftUI-клиент работает **исключительно через GraphQL API**.

GraphQL нужен как:
- read/query слой,
- subscriptions/streaming слой,
- command/mutation façade для UI.

SwiftUI **не использует embedded MCP client** и не разговаривает с MCP напрямую.

### 3.4 MCP — внешний северный control plane
MCP нужен как внешний control-plane интерфейс для:
- внешних агентов,
- automation clients,
- CLI/ops flows,
- создания новых идей,
- запуска и управления runs,
- approvals, retries, resets, comparisons и других операторских действий вне UI.

### 3.5 UI — тонкий, disposable, replaceable
SwiftUI-клиент должен:
- читать projections,
- подписываться на live state,
- инициировать минимальный набор operator actions,
- не принимать workflow decisions,
- не реконструировать run truth “по косвенным признакам”.

---

## 4. Целевая топология

```text
┌────────────────────────────────────────────────────┐
│                  SwiftUI Client                    │
│   Thin UI over GraphQL queries, subscriptions,    │
│                  and mutations                     │
└──────────────────────┬─────────────────────────────┘
                       │
                       │ GraphQL
                       │
┌──────────────────────▼─────────────────────────────┐
│                 Rust Control Plane                 │
│                                                    │
│  - domain engine                                   │
│  - workflow state machine / orchestration          │
│  - run / stage / approval truth                    │
│  - projection builder                              │
│  - artifact index + metadata                       │
│  - session lineage manager                         │
│  - GraphQL server                                  │
│  - MCP server                                      │
│  - ACP runtime manager                             │
└───────────────┬─────────────────────┬──────────────┘
                │                     │
          SQLite / local FS     ACP adapters
                │                     │
                │               Claude Agent ACP
                │               Gemini CLI ACP
                │               Auggie ACP
                │               Junie ACP
                │               ...
```

---

## 5. Основные серверные подсистемы

## 5.1 Domain Engine
Главный внутренний слой, который реализует:
- run lifecycle,
- stage transitions,
- approvals,
- retries,
- recovery rules,
- report semantics,
- session lineage policy,
- proposal loop fidelity,
- MCP policy evaluation,
- runtime binding freeze.

Это **главный мозг** системы.

Он также владеет:
- run compaction policy,
- artifact supersession rules,
- archive eligibility rules,
- projection rebuild after compaction,
- canonical compact snapshot emission.

## 5.2 Workflow / Orchestration Layer
На текущем target state orchestration живёт **внутри самого Rust-сервера**.

Важно:
- это не внешний workflow engine;
- это не логика в UI;
- это application-owned orchestration layer.

Он отвечает за:
- progression between states,
- wait points,
- retry/reset/cancel handling,
- policy enforcement,
- adapter invocation scheduling.

## 5.3 ACP Runtime Manager
Слой, который:
- выбирает runtime profile,
- создаёт ACP-сессии,
- привязывает `cwd` / workspace / MCP set,
- следит за requested→effective runtime truth,
- сохраняет runtime receipts/events,
- отдаёт доменному слою структурированную execution truth.

Этот слой **не принимает продуктовые решения**.  
Он исполняет решение domain engine.

## 5.4 Projection Engine
Сервер обязан строить projections/read models для UI и reporting.

Минимальные projections:
- ideas
- runs
- stages
- approvals
- artifacts
- reports
- runtime health
- active sessions
- proposal-loop metrics
- unresolved backlog / score-lift view
- compacted run summary
- archived artifact summary
- compaction report summary

## 5.5 GraphQL Layer
GraphQL публикует:
- query types,
- subscriptions,
- UI-safe mutations / command façades.

Среди обязательных command façades должен существовать и server-owned maintenance command для run compaction.

Важное правило:
- GraphQL для UI — единственный клиентский API,
- но GraphQL mutations не становятся вторым независимым control plane;
- их семантика должна совпадать с server-owned command model.

## 5.6 MCP Server
MCP server публикует доменные команды и ресурсы для **внешних** клиентов.

Минимальный набор tool categories:
- ideas
- runs
- approvals
- stages
- sessions
- artifacts
- reports
- experiments
- runtime health

MCP server не должен публиковать внутренние низкоуровневые мутации вида:
- `set_stage_status`
- `set_run_state`
- `inject_artifact_without_provenance`

Он публикует **доменные действия**, а не внутренние ручки.

Среди них должен быть и explicit compaction command, например:
- `runs.compact`

## 5.7 Persistence Layer
Минимальный target state:
- **SQLite** для server-owned state,
- **локальный файловый store** для artifact contents,
- **SQLite + file paths** для metadata/projections.

Никакой обязательной внешней инфраструктуры.

Важное уточнение по параллелизму:
- SQLite остаётся целевой persistence-моделью для текущего этапа.
- Масштабирование локальных proposal runs достигается не увеличением числа writers, а короткими сериализованными write transactions и executor backpressure.
- Активный run и активное агентное выполнение — разные вещи: run может оставаться активным, пока его следующий agent work item стоит в очереди из-за capacity.

## 5.8 Run Compaction and Artifact Governance
Сервер обязан поддерживать server-owned maintenance command для compaction eligible runs.

Этот слой отвечает за:
- агрессивное уменьшение active artifact surface,
- archive / supersession policy,
- duplicate collapse,
- link repair,
- projection rebuild after compaction,
- emission of canonical compact snapshots,
- compaction reports.

Важное правило target state:
- compaction доступен только для `completed`, `failed`, `blocked` runs;
- compaction не разрешён для `running`, `ready`, `waitingApproval`, `pending`.

Модель может помогать с semantic clustering и human-readable summary,
но destructive apply, archive truth и repair truth всегда остаются server-owned.

---

## 6. Интерфейсы

## 6.1 Southbound: ACP
ACP — единственный стратегический путь к агентным рантаймам.

Target state:
- ACP-совместимые runtime profiles выбираются на уровне backend/runtime profile,
- runtime truth замораживается в run snapshot,
- сервер умеет работать с несколькими ACP-compatible providers.

Первый target set:
- Claude Agent ACP
- Gemini CLI ACP
- Auggie ACP
- Junie ACP

## 6.2 Northbound: MCP
MCP — публичный control-plane интерфейс для внешних клиентов.

Примеры canonical tools:

- `ideas.create`
- `ideas.list`
- `runs.start`
- `runs.get`
- `runs.cancel`
- `approvals.list`
- `approvals.resolve`
- `stages.retry`
- `sessions.reset_agent`
- `artifacts.get`
- `reports.get`
- `reports.compare`
- `runtime.health`
- `experiments.start`
- `runs.compact`

Примеры resources:

- `idea://{id}`
- `run://{id}`
- `artifact://{id}`
- `report://{id}`
- `workflow://{id}`

## 6.3 Client-facing: GraphQL
GraphQL publishes read-oriented models such as:

- `idea(id)`
- `ideas(filter, paging)`
- `run(id)`
- `runs(filter, paging)`
- `stage(id)`
- `approvalInbox`
- `artifact(id)`
- `report(id)`
- `runtimeStatus`
- `activeSessions`
- `proposalMetrics(runId)`
- `compactionStatus(runId)`
- `compactionReport(runId)`

Обязательная часть target state:
- **GraphQL subscriptions** для live surfaces.

Минимальные live subscriptions:
- active run updates
- active stage updates
- approval inbox updates
- runtime/session status updates
- report availability / artifact readiness where useful

---

## 7. Роль SwiftUI-клиента

## 7.1 Что клиент должен делать
- показывать список идей, runs, approvals, artifacts, reports;
- открывать детальные представления;
- инициировать минимальный набор operator actions через GraphQL mutations;
- показывать projections, а не вычислять их;
- быть заменяемым;
- уметь запускать `Compact Run` для eligible runs и показывать compacted result.

## 7.2 Что клиент НЕ должен делать
- считать, какой stage следующий;
- решать, можно ли retry/reset/cancel;
- восстанавливать execution truth из частичных артефактов;
- владеть session state как canonical source;
- агрегировать review truth;
- реализовывать recovery semantics;
- использовать MCP напрямую.

## 7.3 Минимально допустимые UI-действия
В target state UI должен уметь очень мало:

- создать идею (через GraphQL mutation)
- стартовать run
- открыть details
- approve / reject
- retry / cancel / reset там, где это разрешил сервер
- compare reports
- открыть artifacts / evidence
- compact completed / failed / blocked runs

И всё это должно проходить через server-owned command semantics.

---

## 8. Командная и query-модель

### Canonical rule
- **Commands / mutations для UI**: GraphQL
- **Commands / control для внешних клиентов**: MCP
- **Reads / projections для UI**: GraphQL

Compaction follows the same split:
- UI triggers run compaction through GraphQL mutation
- external operators/agents trigger it through MCP
- all compaction semantics remain server-owned

### Важное уточнение
MCP остаётся canonical внешним control plane,  
но SwiftUI-клиент **не обязан** использовать MCP как transport.

UI взаимодействует только с GraphQL API, а GraphQL command semantics должны быть согласованы с server-owned domain commands.

---

## 9. Данные и хранение

## 9.1 SQLite
SQLite хранит:
- ideas
- runs
- stages
- approvals
- agent executions
- runtime bindings
- session lineage metadata
- projections
- reports metadata
- experiment metadata
- audit/events metadata where needed
- run compaction records
- artifact supersession / archive pointers
- compact snapshot metadata

## 9.2 File artifact store
Отдельно на диске:
- proposal artifacts
- review artifacts
- reports
- transcripts
- runtime receipts
- evidence bundles
- visual artifacts
- archived compact bundles
- run compaction snapshots

В SQLite хранятся:
- paths
- checksums
- provenance
- ownership
- linkage to runs/stages/agents

---

## 10. Runtime truth и ownership

## 10.1 Источник истины
Server-owned SQLite + domain engine projections.

## 10.2 ACP runtime truth
ACP runtime truth — важная, но subordinate truth.
Она нужна для:
- execution evidence,
- session status,
- runtime receipts,
- permission/tool trace,
- requested→effective adapter truth.

Но ACP runtime **не** является владельцем:
- domain transitions,
- approval semantics,
- run truth,
- report truth.

## 10.3 MCP truth
MCP truth — это не state truth, а **внешняя command truth**:
- какие команды допустимы,
- что requested,
- кем вызвано,
- чем закончилось.

## 10.4 GraphQL truth
GraphQL truth для UI — это:
- read truth over projections,
- command façade truth for the client.

GraphQL не является вторым доменным владельцем.  
Он публикует server-owned truth.

---

## 11. Local-first operational model

Target state intentionally остаётся **локальным**.

### Это означает:
- один локальный Rust daemon
- одна SQLite database
- локальный artifact store
- локальный SwiftUI app
- локальные ACP runtimes
- локальный GraphQL server
- локальный MCP server
- никакой обязательной распределённой инфраструктуры

### Операционный выбор
На текущем этапе принимается режим:

> **single-process singleton**

То есть:
- без зоопарка сервисов,
- без оркестрации набора локальных демонов,
- без обязательного multi-process control plane.

### Допустимые ограничения target state
- потеря части in-flight состояния допустима, если canonical domain truth и archive semantics остаются внятными;
- horizontal scalability не является текущей целью;
- exactly-once guarantees не являются текущим обязательным свойством;
- multi-user and remote deployment are out of scope.

### Целевой локальный параллелизм

Текущий product target для локального single-process singleton:

- **5 active runs** должны работать стабильно без ручного babysitting.
- **10 active runs** допустимы как bounded stretch target, если executor backpressure держит количество одновременно активных agent executions в пределах capacity.
- **20 одновременно активных agent executions** являются целевым потолком для review fan-out, но только через явные global/per-run/provider caps.

Начальная capacity-модель:

- глобальный лимит активных agent executions: 20;
- per-run лимит активных agent executions: 4;
- provider caps по умолчанию: Gemini 4, Codex 10, Claude 8, Auggie 1, Junie 1.
- ACP provider subprocess запускается в отдельной process group; `session/close` вызывается даже после transport error в `session/prompt`, чтобы не оставлять MCP/plugin descendants после idle timeout.
- Sleep/wake ноутбука и смена Wi-Fi/network path считаются host interruption epoch: running ACP executions, пересёкшие такой epoch, должны закрываться, классифицироваться как retryable host interruption, и переочередиться с jitter/backoff под теми же capacity caps, а не превращаться в массовые permanent provider failures.

Когда capacity закончилась, правильное поведение системы:

- не падать и не помечать work как failed только из-за capacity;
- оставить work pending/backpressured;
- показать причину backpressure в projections/GraphQL/MCP;
- продолжить выполнение, когда освободится слот.
- после завершения последнего `InvokeAgent` надежно разбудить/запланировать `AdvanceRun` или finalizer, чтобы stage не оставался `running` без активных agents и без pending work.

Это означает, что целевая система должна выдерживать 5-10 active runs как operator workload и до 20 внешних agent processes только в рамках capacity-модели, а не через бесконтрольный fan-out.

---

## 12. Security / trust model

Даже для локального режима нужны базовые инварианты:

- только сервер принимает продуктовые решения;
- MCP server должен уметь разделять хотя бы:
  - operator clients
  - automation/agent clients
  - read-only clients
- ACP runtime requested vs effective capabilities должны логироваться;
- artifact provenance и command provenance должны быть queryable;
- UI не должен иметь “секретных” команд, которых нет в server control plane.

---

## 13. Наблюдаемость

Target state должен поддерживать:

### 13.1 Runtime observability
- ACP session status
- runtime adapter choice
- provider/model truth
- requested/effective MCP set
- session lineage state
- resets / retries / compactions

### 13.2 Product observability
- run progression
- stage settlement
- approvals
- blocked reasons
- recovery recommendations
- proposal-loop quality metrics
- report truth
- compaction status
- compact snapshot truth
- archive/supersession truth

### 13.3 Operator observability
- active run timeline
- approval inbox
- unresolved backlog
- failed-stage evidence
- runtime health panel
- compaction report
- compacted run summary
- optional archived-artifact access path

### 13.4 GraphQL live UX
Поскольку subscriptions считаются критически необходимыми, thin client должен иметь live surfaces без постоянного polling:
- run updates
- stage updates
- approvals
- session/runtime status

---

## 14. Migration intent

Target state задаёт **куда прийти**, а не как именно выполнить переход.

High-level путь:
1. Сначала серверная копия логики появляется без изменения клиента.
2. Потом MCP northbound control plane становится доступен.
3. Потом GraphQL projections и subscriptions стабилизируются.
4. Потом SwiftUI-клиент становится thin client.
5. Server-owned maintenance commands such as run compaction become the only valid compaction path.
6. Потом старый client-owned orchestration слой умирает.

---

## 15. Что НЕ входит в target state

Target state не требует сейчас:

- распределённого сервера,
- Temporal,
- Kafka/NATS/Redis,
- внешней workflow-платформы,
- Postgres как обязательной замены SQLite,
- multi-region deployment,
- server cloud migration,
- сложной auth federation,
- строгих exactly-once guarantees,
- полного отказа от локального режима,
- 10-20 одновременно активных agent executions,
- compaction для running runs.

---

## 16. Признаки того, что target state достигнут

Систему можно считать достигшей target state, когда одновременно выполняются следующие свойства:

1. Вся orchestration/domain логика живёт в Rust-сервере.
2. SwiftUI-клиент не владеет workflow truth.
3. ACP — единственный стратегический southbound интерфейс к агентам.
4. MCP — canonical внешний northbound control plane.
5. GraphQL — единственный client-facing API для SwiftUI.
6. GraphQL subscriptions обеспечивают live thin-client UX.
7. SQLite + local artifact store достаточны для текущего продукта.
8. Система работает как single-process singleton.
9. UI можно переписать или заменить без риска потерять workflow semantics.
10. Внешний агент-клиент может управлять системой через MCP без участия UI.
11. Server-owned projections достаточны, чтобы UI не реконструировал правду сам.
12. Server-owned run compaction exists for `completed`, `failed`, and `blocked` runs.
13. Compaction emits a canonical compact snapshot, preserves archive truth, and materially reduces active artifact noise.
14. 5 active runs работают стабильно на локальном Rust/SQLite daemon без SQLite lock failures, stale running executions и ручного babysitting.
15. До 10 active runs могут находиться в системе одновременно, при этом surplus agent work явно queued/backpressured, а не запускается бесконтрольно.
16. До 20 active agent executions могут выполняться одновременно только под global/per-run/provider caps и с доказательством через proposal-061 gate.

---

## 17. Разъяснение по runtime capability classes

Ранее оставался открытый вопрос: нужно ли закреплять capability classes вроде:
- `lifecycle-capable`
- `control-capable`
- `operator-grade`

### Что это значит простыми словами
Это не про пользовательские роли и не про UI.
Это просто способ классифицировать ACP runtime adapters по их зрелости.

Например:

- **lifecycle-capable**  
  умеет создать сессию, отправить prompt и отменить её

- **control-capable**  
  кроме этого, даёт достаточно truth для session/load, runtime mutation, requested→effective MCP

- **operator-grade**  
  кроме этого, даёт достаточно observability для live timeline, permission callbacks, tool visibility, recovery/report surfaces

### Решение для target state
Эти классы **не нужно фиксировать в target state как жёсткий пользовательский контракт**.

Target state должен зафиксировать только более простую и практичную вещь:

> сервер знает и публикует effective capabilities каждого runtime profile.

А вот конкретная taxonomy capability classes может жить на уровне:
- runtime profile implementation
- ACP adapter architecture
- compatibility matrix
- transport/probe docs

То есть target state не требует заранее прибить именно эти три названия.  
Но он требует, чтобы система **понимала и публиковала, что конкретный runtime реально умеет**.

---

## 18. Решённые вопросы

### Q1. Должен ли SwiftUI-клиент отправлять mutations напрямую через embedded MCP client?
**Нет.**
SwiftUI-клиент работает исключительно только с GraphQL API.

### Q2. Нужны ли GraphQL subscriptions?
**Да, критически необходимы.**

### Q3. Нужен ли единый command journal / audit log как first-class projection?
**Нет, на текущем этапе достаточно логов и run-scoped evidence.**

### Q4. Нужно ли закрепить runtime capability classes уже в target state?
**Нет, не как жёсткую taxonomy target state.**
Но сервер должен публиковать effective runtime capabilities.

### Q5. Должен ли локальный сервер быть строго single-process singleton?
**Да.**
На текущем этапе оставляем single-process singleton и не связываемся с ворохом сервисов и их оркестрацией.

---

## 19. Итог

Целевая система — это **локальный control plane**, а не “толстый клиент с агентами”.

Сервер становится мозгом.
ACP становится южной интеграцией.
GraphQL становится единственным интерфейсом для SwiftUI.
MCP становится внешним интерфейсом управления и автоматизации.
SwiftUI остаётся удобной оболочкой, но перестаёт быть источником решений.

Это даёт:
- более чистый центр системы,
- лучшую заменяемость UI,
- лучшую автоматизируемость,
- более явную product truth,
- server-owned maintenance operations вроде run compaction,
- и гораздо более здоровую архитектуру для дальнейшего роста.
