# Proposal Revision Summary

Reworked the initial JWT auth proposal into a more implementation-ready draft aligned to current repo reality.

Main improvements:
- anchored the scope to the actual daemon surfaces: `/mcp`, `/graphql`, and `/graphql/ws`
- removed the implied `/health` dependency and documented that it does not exist today
- made crate ownership, migration naming, and config requirements explicit
- expanded the RBAC section into a concrete seeded permission matrix
- clarified GraphQL websocket auth, MCP tool authorization, and bootstrap-user provisioning
- tightened rollout into dark launch, enforced dev/CI, and enforced baseline phases
- added explicit reviewer-feedback resolution entries even though no review artifact exists yet

Key trade-off decisions recorded for review:
- JWT access tokens with opaque refresh tokens
- HS256 for MVP with an RS256-ready abstraction
- full protection of current control-plane transport surfaces
- strict refresh-token replay handling by revoking the full token family
- temporary rollout flag, not permanent optional auth
- `operator` retains `approvals:resolve` in MVP
- logout revokes only the presented session in MVP
