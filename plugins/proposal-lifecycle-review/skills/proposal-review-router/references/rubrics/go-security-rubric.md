# Go Security Review Rubric

Use for Go proposals touching auth, public endpoints, secrets, validation, outbound calls, permissions, or sensitive data.

## Focus areas

- Authn/authz: middleware order, principal propagation, RBAC, tenant scope, and denial behavior are explicit.
- Secrets/PII: logs, errors, traces, metrics, and persistence avoid leakage.
- Validation/deserialization: body size, schema validation, unknown fields, and type coercion are controlled.
- Outbound calls: SSRF-like risks, redirects, DNS, egress allowlists, and timeouts are considered.
- Supply chain: new modules, code generation, and native dependencies are reviewed.

## Sharp heuristics

- Treat webhook/public endpoint proposals as security-sensitive by default only when the endpoint is actually public.
- Treat URL fetchers and proxy-like behavior as SSRF candidates.
- Treat auth middleware changes as API contract and rollout changes when clients observe errors.

## Finding requirements

Each finding must cite evidence IDs, trust boundary, abuse case, required fix, acceptance criteria, and confidence.
