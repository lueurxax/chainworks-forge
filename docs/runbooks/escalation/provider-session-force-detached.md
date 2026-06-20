# Provider Session Force Detached

**Code:** `provider_session_force_detached`

**Phase:** `runtime`

**Operator Action Hint:** Inspect provider state and resume manually; tier does not auto-advance. If `identity_ambiguous` appears, treat it as a manual identity hold and do not retry until provider identity is resolved.
