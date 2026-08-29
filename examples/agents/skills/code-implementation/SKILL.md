---
name: code-implementation
description: Implements an approved proposal incrementally with focused tests and explicit remaining-work evidence.
compatibility: Chainworks Forge implementation stages with a dedicated writable worktree and frozen output contracts.
---
# Code Implementation Procedure

1. Treat the frozen operator request and approved proposal as the implementation boundary. Use review artifacts to resolve defects inside that boundary, not to add adjacent scope.
2. Inspect the existing subsystem before editing. Follow established ownership, persistence, API, and test patterns.
3. Implement the smallest coherent change that closes the approved acceptance criteria. Keep unrelated dirty work intact.
4. Add focused regression coverage proportional to the changed behavior and run the narrowest authoritative local gates available.
5. Report completed work, verification, remaining code-owned tasks, and known risk through the declared provider outputs. Control-plane-owned Git evidence is not a provider output.

This procedure does not grant Git metadata access, release authority, extra tools, or permission beyond the frozen agent binding.
