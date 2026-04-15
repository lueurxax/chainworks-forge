# JWT Authentication Module Proposal

## Status
Refined proposal for idea `JWT Authentication Module`.

This version is intended to be the single review and implementation document. It incorporates the current idea, makes the key trade-offs explicit, and resolves the recorded reviewer feedback without silently collapsing disagreements.

## Executive Summary
The control-plane daemon currently exposes state-changing and read-capable HTTP surfaces without caller authentication:

- `POST /mcp`
- `GET /graphql`
- `POST /graphql`
- `GET /graphql/ws`

The daemon also binds to `0.0.0.0:4000` by default today. That means any local process and, in some environments, any reachable LAN client can read and mutate ideas, runs, approvals, and reports without proving identity.

This proposal adds authentication and minimal role-based authorization to daemon HTTP surfaces while keeping the existing Swift subprocess stdio ACP path unchanged. The MVP uses short-lived JWT access tokens, opaque rotating refresh tokens, CLI-first bootstrap, and a narrow `admin` / `operator` RBAC model. It does not attempt to solve internet-grade deployment, TLS termination, OAuth, or a broad account-management system in this milestone.

## Problem
Current agent permission profiles constrain what an agent may do inside a run. They do not authenticate or authorize the human or client that:

- creates ideas
- starts runs
- resolves approvals
- reads reports
- invokes MCP tools over HTTP

That is a real security gap now, not a future concern, because the daemon already exposes first-party control surfaces that are intended for dogfooding and future client use.

## Trigger, Users, and Threat Model

### Trigger
Authentication is now a blocking gap because the daemon has already crossed from internal implementation detail into a real operator-facing control surface.

### Target users in the next 6 months
- `admin`: bootstraps auth, rotates keys, manages users, performs break-glass recovery
- `operator`: creates ideas, starts and observes runs, resolves approvals, reads reports

Expected distinct human identities remain small, likely 1 to 3. That supports a narrow MVP. It does not justify leaving the daemon unauthenticated.

### Threat tiers for this milestone

| Tier | In scope | Required control |
|---|---|---|
| T1: same-host untrusted process | yes | bearer auth on all daemon HTTP control surfaces |
| T2: LAN-reachable daemon | yes | auth-enabled daemon binds loopback by default unless explicit insecure override is set |
| T3: internet or reverse-proxied deployment | no | deferred until TLS and deployment hardening are owned explicitly |

Locked decision:
- This milestone does not treat authenticated plaintext traffic on non-loopback interfaces as an acceptable default.
- If auth is enforced and TLS is absent, loopback is the default bind posture.
- Non-loopback under auth requires an explicit insecure override and should be treated as an exceptional dev-only posture.

## Goals
1. Require auth on all daemon HTTP control surfaces when auth mode is enforced.
2. Issue short-lived JWT access tokens and long-lived opaque refresh tokens.
3. Implement refresh-token rotation safely under SQLite WAL concurrency.
4. Add minimal RBAC that cleanly separates `admin` from `operator` without building a full IAM system.
5. Keep current Swift subprocess stdio ACP runtime flows unchanged unless a client is explicitly using daemon HTTP.
6. Define concrete UX for bootstrap, login, re-auth, forbidden actions, degraded secure storage, and long-lived subscriptions.
7. Support signing-key rotation and operational recovery on day one.

## Non-Goals
1. OAuth2, OIDC, SSO, MFA, password reset, or self-service registration.
2. TLS termination inside the daemon.
3. Service-to-service credentials or machine API keys.
4. Fine-grained ABAC, resource ownership rules, or policy engines.
5. A broad account-management UI.
6. Changes to subprocess stdio ACP transport between the Swift app and agent runtimes.
7. Internet-facing deployment support.

## Scope Boundary

### In scope
- daemon HTTP auth for MCP and GraphQL
- login, refresh, and session revoke endpoints
- bootstrap-admin CLI flow
- refresh-token rotation and replay handling
- minimal RBAC enforcement
- GraphQL WebSocket auth at connect time
- key rotation support for the access-token signer
- app-side UX for re-auth and secure-storage degradation when the app is acting as a daemon HTTP client

### Out of scope
- stdio subprocess auth
- per-agent runtime credentialing
- external identity providers
- transport encryption
- admin console UX beyond bootstrap and basic error handling

## UX and UI Notes

### Request-path reality
- Current Swift ACP runtime traffic is subprocess stdio, not daemon HTTP.
- This proposal authenticates daemon HTTP consumers:
  - external MCP HTTP clients calling `/mcp`
  - external or future first-party GraphQL clients calling `/graphql` and `/graphql/ws`
  - any future Swift daemon mode if the app is pointed at the daemon
- MCP stdio mode remains trusted parent-process execution and receives an implicit admin principal.

### Bootstrap and login
Locked decision: bootstrap is CLI-first.

Flow:
1. Operator starts daemon with auth enabled.
2. If no users exist, `POST /auth/login` returns `428 bootstrap_required`.
3. UI shows a non-modal explanation plus a copyable command such as `control-plane auth bootstrap-admin`.
4. Bootstrap creates the first active `admin` user transactionally.
5. Operator logs in with username and password.

### Credential prompt
- Use the existing sheet pattern, not `NSAlert`.
- Match the app’s current setup/add-provider sheet conventions.
- Include username field, `SecureField` for password, submit, and cancel.
- Minimum size: width `420`, height `240`.

Accessibility identifiers:
- `auth-credential-prompt`
- `auth-credential-username-field`
- `auth-credential-secret-field`
- `auth-credential-submit`
- `auth-credential-cancel`

### 401 and 403 behavior
- `401 Unauthorized` means missing, expired, revoked, or invalid auth.
- `403 Forbidden` means authenticated but not allowed.

Locked decision:
- `401` shows a non-modal banner first.
- The credential sheet opens only when the operator initiates re-auth or a blocking mutation requires it.
- `403` never opens the credential prompt.
- Client-side affordance gating may use the roles claim, but server-side authorization remains authoritative.

### Mid-run auth expiry
Runs are daemon-owned. Losing client auth must not kill active runs.

Locked decision:
- The app enters degraded read-only mode after a `401` on a protected daemon request.
- The app queues exactly one retry for client-originated, idempotent-intent mutations:
  - `start run`
  - `cancel run`
  - `approve`
  - `reject`
- After successful re-auth, the app retries the queued mutation once.
- If the retry fails again, the queued action is dropped and the operator gets a concrete error.

### Secure storage failure
- Preferred token storage is `KeychainSecretStore`.
- If Keychain is unavailable, fall back to in-memory session storage only.
- Show a visible warning that auth state will not survive app restart.

### GraphQL WebSocket behavior
Locked decision:
- authenticate at `connection_init`
- allow the socket to live for the authenticated connection lifetime, bounded by a configurable max connection TTL
- do not revalidate on every access-token expiry tick
- require a fresh access token on reconnect
- close the socket on server-side session revocation

Rationale:
- This avoids freezing monitoring during long-running operations.
- It keeps the implementation bounded.
- It preserves revocation as the real immediate kill switch.

## Architecture

### Crate ownership
- `control-plane/crates/domain`
  - `AuthenticatedPrincipal`
  - `Role`
  - `Permission`
  - `UserId`
- `control-plane/crates/auth`
  - password hashing and verification
  - JWT sign and verify
  - auth config
  - login, refresh, revoke services
  - Axum middleware and extractors
  - narrow auth endpoint rate limiting
- `control-plane/crates/db`
  - auth tables, repos, migrations, rollback script
- `control-plane/crates/daemon`
  - route mounting, bind policy, auth mode, startup validation
- `control-plane/crates/mcp-server`
  - HTTP principal injection and tool-family authorization
- `control-plane/crates/graphql-server`
  - HTTP and WebSocket principal injection and resolver authorization

Locked decision:
- Principal and permission types live in `domain`, not `auth`, to preserve the crate DAG and avoid reverse dependency pressure later.

### Auth modes and bind policy
Config:
- `CONTROL_PLANE_AUTH_MODE=disabled|observe|enforce`
- `CONTROL_PLANE_BIND_ADDR`
- `CONTROL_PLANE_AUTH_ALLOW_INSECURE_NON_LOOPBACK=false` by default

Behavior:
- `disabled`: auth is off
- `observe`: daemon accepts requests but records missing-auth warnings and metrics
- `enforce`: all protected daemon HTTP surfaces require auth
- startup fails if `enforce` is combined with non-loopback bind and the insecure override is not explicitly set

### Data model
Add migration `004_auth.sql` with additive tables only.

Tables:
1. `users`
   - `id`
   - `username` unique
   - `password_hash`
   - `status` (`active`, `disabled`)
   - `created_at`
   - `updated_at`
2. `roles`
   - `id`
   - `name` unique
   - `description`
3. `permissions`
   - `id`
   - `name` unique
   - `description`
4. `user_roles`
   - `user_id`
   - `role_id`
   - unique composite key
5. `role_permissions`
   - `role_id`
   - `permission_id`
   - unique composite key
6. `refresh_tokens`
   - `id`
   - `user_id`
   - `token_hash`
   - `family_id`
   - `status` (`active`, `consumed`, `revoked`)
   - `replacement_token_id` nullable
   - `issued_at`
   - `expires_at`
   - `consumed_at` nullable
   - `revoked_at` nullable
   - `revocation_reason` nullable
   - `grace_until` nullable
   - `client_fingerprint` nullable
7. `auth_keys`
   - `kid`
   - `status` (`current`, `previous`, `retired`)
   - `created_at`
   - `not_after`

Locked decision:
- `auth_keys` stores metadata only.
- Secret material stays in environment variables or restricted files, not in SQLite.

### RBAC model
Locked decision:
- Keep RBAC in scope.
- Narrow day-one seeding to `admin` and `operator`.
- Model `viewer`, but do not seed it in MVP.

Seed permissions:
- `ideas:read`
- `ideas:create`
- `runs:read`
- `runs:start`
- `runs:cancel`
- `approvals:read`
- `approvals:resolve`
- `reports:read`
- `admin:manage_auth`

Permission matrix:

| Permission | admin | operator | viewer later |
|---|---|---|---|
| `ideas:read` | yes | yes | yes |
| `ideas:create` | yes | yes | no |
| `runs:read` | yes | yes | yes |
| `runs:start` | yes | yes | no |
| `runs:cancel` | yes | yes | no |
| `approvals:read` | yes | yes | yes |
| `approvals:resolve` | yes | yes | no |
| `reports:read` | yes | yes | yes |
| `admin:manage_auth` | yes | no | no |

Trade-off:
- Reviewer feedback correctly argued that broad RBAC would be premature.
- The proposal rejects full removal of RBAC because `admin` and `operator` already represent different trust boundaries.
- The compromise is a minimal, fixed permission matrix with no policy language and no day-one `viewer` rollout.

## Token and Session Model

### Access token
- JWT
- default TTL: 15 minutes
- algorithm: HS256 in MVP
- required header: `kid`
- required claims:
  - `iss`
  - `aud`
  - `sub`
  - `jti`
  - `iat`
  - `exp`
  - `roles`

Locked decision:
- Do not include `permissions_version` in MVP.

Reason:
- It creates invalidation complexity the current repo does not support.
- A 15-minute authorization staleness window is acceptable for the scoped threat model.
- Urgent lockout is handled by refresh-session revocation and WebSocket disconnect on revocation.

### Signing-key policy
Locked decision:
- Ship rotation support on day one.
- Use a two-key ring: current for signing, current plus previous for verification.

Config:
- `CONTROL_PLANE_AUTH_SIGNING_KEY_CURRENT`
- `CONTROL_PLANE_AUTH_SIGNING_KEY_PREVIOUS`
- file-based variants for both

Rules:
- minimum secret length: 32 bytes
- all new tokens use the current key
- current and previous keys verify
- retired keys do not verify

Compromise response for suspected compromise:
1. install a new current key
2. move the old current key to previous only if temporary verification grace is required
3. revoke all refresh-token families
4. restart daemon
5. force re-auth for all clients

Trade-off:
- Reviewer feedback raised valid concern about HS256.
- The proposal does not switch to RS256 in MVP because verification remains in-process and key distribution is not cross-service yet.
- The proposal does require `kid` and immediate rotation support so HS256 does not become a dead-end shortcut.

### Refresh token
- opaque random secret
- default TTL: 30 days
- stored hashed
- rotated on every refresh

Locked decision:
- Use strict family revocation for real replay.
- Add a bounded duplicate-submit grace path for normal desktop retry behavior.

Rotation rules:
1. execute refresh rotation inside one SQLite transaction using `BEGIN IMMEDIATE`
2. exactly one caller may consume an active token
3. on success, the old token becomes `consumed` and gets `grace_until = now + 30s`
4. if the exact just-consumed token is replayed during the grace window and the replacement token already exists, return the same replacement metadata without minting a second branch
5. if a consumed token is replayed outside grace, or a stale ancestor token is presented, revoke the entire family

This explicitly resolves the reviewer disagreement:
- accept the UX concern that duplicate desktop submits happen
- reject the idea of silently tolerating general replay
- allow bounded retry noise, but burn the family on real replay

### Password hashing
- use Rust `argon2` crate with PHC output
- algorithm: Argon2id
- memory cost, time cost, and parallelism are configurable
- hashing and verification run on `spawn_blocking`
- target login p95 below 500 ms on the baseline development machine

## Endpoint Contract

### `POST /auth/login`
Request:
- `username`
- `password`

Success:
- `access_token`
- `token_type`
- `expires_in`
- `refresh_token`
- `refresh_expires_in`
- `roles`

Failures:
- `401` invalid credentials
- `403` disabled user
- `428` bootstrap required
- `429` too many attempts

### `POST /auth/refresh`
Request:
- `refresh_token`

Success:
- same shape as login

Failures:
- `401` invalid, expired, revoked, or replayed token
- `429` too many attempts

### `POST /auth/revoke`
Request:
- current refresh token or session identifier

Locked decision:
- revoke the presented session only in MVP
- global sign-out remains future admin work

## Enforcement Contract

### MCP over HTTP
- authenticate before JSON-RPC dispatch
- map tool families to permissions
- preserve `Mcp-Session-Id`

### MCP over stdio
- treat as trusted local parent-process launch
- inject implicit admin principal
- reuse the same permission-check code paths for consistency

### GraphQL over HTTP
- authenticate both `GET /graphql` and `POST /graphql`
- build resolver context from `AuthenticatedPrincipal`
- authorize at resolver and mutation boundaries

### GraphQL over WebSocket
- require bearer token in `connection_init.payload.Authorization` or `connection_init.payload.authorization`
- reject unauthenticated sockets with a close frame
- inject principal into subscription context
- close the socket on session revocation

### Auth endpoint rate limiting
Add a narrow in-memory limiter in `auth`:
- login: 5 attempts per minute per username plus source-IP bucket
- refresh: 10 attempts per minute per token-family bucket

Locked decision:
- Keep this limiter intentionally narrow.
- Do not turn this proposal into a general perimeter-defense effort.

## Implementation Plan

### Workstream 1: schema and domain
- add `004_auth.sql`
- add rollback script for auth tables only
- add auth domain types in `domain`
- add DB repos for users, roles, permissions, and refresh-token families

### Workstream 2: auth core
- create `control-plane/crates/auth`
- implement Argon2 password hashing and verification
- implement JWT signer and verifier with `kid`
- implement auth config parsing and startup validation
- implement bootstrap-admin CLI flow as transactional and idempotent

### Workstream 3: daemon integration
- mount `/auth/login`, `/auth/refresh`, `/auth/revoke`
- add auth middleware and extractors
- enforce auth mode and bind-policy validation at startup
- add observe-mode logging and metrics

### Workstream 4: protocol surfaces
- inject principals into MCP HTTP request handling
- add permission mapping for tool families
- inject principals into GraphQL HTTP and WebSocket contexts
- add server-initiated socket close on revocation

### Workstream 5: app UX
- add credential sheet
- add banner-first `401` handling
- add `403` permission messaging
- add degraded read-only mode and one-shot retry-after-reauth queue
- add visible secure-storage degradation warning

### Workstream 6: proof and rollout
- add focused non-UI proof lanes
- add at least one authenticated end-to-end flow
- stage rollout through `observe` then `enforce`

## Acceptance Criteria
1. When `CONTROL_PLANE_AUTH_MODE=enforce`, unauthenticated requests to `/mcp`, `/graphql`, and `/graphql/ws` are rejected.
2. Bootstrap requires an explicit CLI admin-creation step when no users exist.
3. A valid login returns an access token, refresh token, and roles.
4. Refresh rotates the refresh token and prevents token branching under concurrent calls.
5. Replay outside the grace window revokes the refresh-token family.
6. GraphQL WebSocket connections authenticate at `connection_init` and are closed on revocation.
7. MCP stdio continues to function without a daemon HTTP login flow.
8. `operator` cannot perform `admin:manage_auth` actions.
9. Auth-enabled non-loopback bind fails at startup unless the insecure override is explicitly set.
10. Key rotation supports current-plus-previous verification without invalidating every live access token immediately.

## Rollout

### Phase 0: design and schema
- land `004_auth.sql`, rollback script, and repos
- land domain auth types and `auth` crate
- land bootstrap CLI
- land key-ring config and auth limiter

Rollback:
- rollback script drops auth tables only
- runtime remains `disabled`

### Phase 1: observe mode
- mount auth endpoints
- run with `CONTROL_PLANE_AUTH_MODE=observe`
- record missing-auth requests on `/mcp`, `/graphql`, `/graphql/ws`
- validate bootstrap and login UX

Rollback trigger:
- login-path instability
- daemon startup failures tied to auth config

Rollback procedure:
- set `CONTROL_PLANE_AUTH_MODE=disabled`
- restart daemon

### Phase 2: enforce in dev and CI
- enable `enforce` for CI and first-party daemon dev runs
- require authenticated HTTP integration tests
- enable WebSocket auth tests

Rollback trigger:
- re-auth failure rate above 2%
- unrecoverable CI breakage on authenticated flows

Rollback procedure:
- revert to `observe`
- keep schema intact

### Phase 3: default enforce for daemon HTTP
- default auth mode becomes `enforce`
- default bind under auth becomes loopback
- non-loopback requires explicit insecure override until TLS work lands

Rollback trigger:
- operator lockout
- auth-enabled startup or login regression blocks normal local use

Rollback procedure:
- use `CONTROL_PLANE_AUTH_MODE=disabled` as break-glass
- treat this as emergency-only, not normal deployment

## Metrics

### Outcome metrics
- zero unauthorized state mutations on enforced daemon HTTP surfaces
- zero auth-enabled deployments using plaintext non-loopback binds by default

### Adoption metrics
- first-login success rate above 95%
- forced re-auth rate below 2% per active operator day

### Operational metrics
- percentage of `/mcp`, `/graphql`, `/graphql/ws` requests denied for missing auth in observe mode
- refresh success rate
- refresh grace-path reuse rate
- replay-family revocation count
- `403` count by permission
- WebSocket auth failure count
- signing-material rotation drill time

## Risks and Trade-offs
1. HS256 remains the MVP signing algorithm. That is acceptable only because verification stays in-process and rotation support ships immediately.
2. The daemon remains unsuitable for internet exposure because TLS is out of scope.
3. Retry-after-reauth adds app complexity, but removing it would make long-running operator flows brittle around token expiry.
4. Even minimal RBAC adds mapping overhead. Limiting day-one seeding to `admin` and `operator` keeps that bounded.
5. Break-glass disablement is operationally necessary, but it must remain visibly exceptional or insecure behavior will become normalized.

## Open Questions
None of these block Phase 0, but each has an explicit default and owner.

| Question | Default | Owner | Decision deadline |
|---|---|---|---|
| Health endpoint | do not add now | daemon owner | deferred |
| Machine credentials | defer | auth implementer | after Phase 3 if needed |
| GraphQL WS token payload casing | accept `Authorization` and `authorization` | graphql owner | before Phase 1 |
| Initial TTL tuning | 15m access / 30d refresh | auth implementer | validate in Phase 2 |
| Viewer role seeding | defer until a real read-only client exists | product owner | before Phase 3 |

## Reviewer Feedback Resolution

### Product feedback
| Feedback | Resolution | Decision |
|---|---|---|
| `PO-001` threat model missing | Added explicit T1/T2/T3 threat model and loopback-by-default posture. | Accepted |
| `PO-002` user and trigger unclear | Added concrete trigger, user types, and 6-month identity horizon. | Accepted |
| `PO-003` RBAC may be premature | Kept RBAC, but reduced day-one seeding to `admin` and `operator`; deferred `viewer`. | Partially accepted |
| `PO-004` Swift integration mismatched real transport | Separated subprocess stdio from daemon HTTP clients and named exact protected surfaces. | Accepted |
| `PO-005` no user outcome metrics | Added outcome, adoption, and operational metrics. | Accepted |
| `PO-006` rollout lacked rollback | Added rollback triggers and rollback procedures per phase. | Accepted |
| `PO-007` key lifecycle underspecified | Added `kid`, two-key ring, minimum key length, and compromise response. | Accepted |
| `PO-008` open questions unmanaged | Added defaults, owners, and deadlines. | Accepted |
| `PO-009` WebSocket auth not designed | Specified `connection_init` auth and connection-lifetime behavior. | Accepted |
| `PO-010` stdio policy omitted | Documented stdio as trusted implicit admin and out of daemon HTTP scope. | Accepted |

### UX feedback
| Feedback | Resolution | Decision |
|---|---|---|
| `UX-001` no mid-run auth failure contract | Added degraded read-only mode and one-shot retry-after-reauth queue. | Accepted |
| `UX-002` replay revocation too aggressive | Kept family revocation for real replay, added 30-second duplicate-submit grace for retry noise. | Partially accepted |
| `UX-003` no 403 workflow UX | Added distinct `403` banner or inline error handling and no credential-sheet escalation. | Accepted |
| `UX-004` bootstrap experience unclear | Chose CLI bootstrap and specified `428 bootstrap_required` UI behavior. | Accepted |
| `UX-005` silent refresh interruption unspecified | Chose banner-first, non-modal interruption. | Accepted |
| `UX-006` WebSocket auth renewal unclear | Chose connect-time auth plus reconnect-on-new-token behavior. | Accepted |
| `UX-007` Keychain degraded mode missing | Added in-memory fallback with visible warning. | Accepted |
| `UX-008` accessibility missing | Added explicit accessibility identifiers for auth surfaces. | Accepted |

### UI feedback
| Feedback | Resolution | Decision |
|---|---|---|
| `UI-001` prompt visual spec absent | Specified sheet pattern, controls, and minimum size. | Accepted |
| `UI-002` 401 and 403 mapping unclear | Mapped `401` and `403` to different UI responses. | Accepted |
| `UI-003` interruption timing unclear | Set banner-first interruption and operator-initiated credential prompt. | Accepted |
| `UI-004` Keychain failure UI missing | Added visible secure-storage diagnostic warning. | Accepted |
| `UI-005` auth accessibility omitted | Added identifiers and required them in the UI contract. | Accepted |

### Architecture feedback
| Feedback | Resolution | Decision |
|---|---|---|
| `ARCH-001` refresh rotation atomicity unspecified | Requires one `BEGIN IMMEDIATE` transaction and a concurrent refresh proof test. | Accepted |
| `ARCH-002` key rotation missing | Added `kid`, current-plus-previous verification, and rotation procedure. | Accepted |
| `ARCH-003` principal types in wrong crate | Moved principal and permission types into `domain`. | Accepted |
| `ARCH-004` WebSocket auth unspecified | Added explicit WebSocket auth and revocation behavior. | Accepted |
| `ARCH-005` stdio bypass ignored | Documented implicit stdio principal while reusing shared authorization code paths. | Accepted |
| `ARCH-006` migration rollback missing | Constrained migration to additive tables and required rollback script. | Accepted |
| `ARCH-007` bootstrap atomicity missing | Bootstrap is transactional and idempotent CLI work. | Accepted |
| `ARCH-008` Argon2 parameters unspecified | Added Argon2id, configurability, and latency target. | Accepted |
| `ARCH-009` no rate limiting | Added narrow login and refresh rate limiting. | Accepted |
| `ARCH-010` test seam unspecified | Added `AuthVerifier` and `AuthContextProvider` test seams plus proof lanes. | Accepted |
| `ARCH-011` `permissions_version` unclear | Removed it from MVP and accepted bounded authorization staleness instead. | Accepted |

## Proof Plan
Required proof lanes before Phase 2:
- one end-to-end login, refresh, revoke flow
- one concurrent refresh race test proving exactly one winner under `BEGIN IMMEDIATE`
- one GraphQL WebSocket auth test
- one stdio implicit-principal test
- one current-plus-previous key-rotation verification test
- one startup-validation test for enforced non-loopback bind without override

Test seam:
- define `AuthVerifier` and `AuthContextProvider` so handlers and resolvers can be tested with injected principals instead of full crypto setup

## Recommendation
Proceed with this proposal.

It closes a live security gap on the daemon’s real control surfaces, preserves the current stdio transport model, keeps auth scope narrow enough for MVP delivery, and records the hard trade-offs explicitly rather than hiding them behind future follow-up work.
