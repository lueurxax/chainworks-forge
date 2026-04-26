# Phase 0 Settlement Service Boundary

Durable mediation settlement occurs only through the engine-owned `MediationSettlementService`.

## Allowed Entrypoints
- ResolveLeadMediationConfirmation command path
- Engine or orchestrator auto-settle path for valid no-confirmation outcomes
- Engine recovery or repair path for stale, canceled, duplicate, and ignored-late-output outcomes

## Forbidden Entrypoints
- GraphQL mutation or resolver direct write path
- MCP server direct repository write path
- DB repository method that calls TransitionAuthorityResolver
- ApproveStage or RejectStage reused as a shortcut for mediated settlement
