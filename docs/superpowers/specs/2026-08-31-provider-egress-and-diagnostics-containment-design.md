# Provider Egress and Diagnostics Containment

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Inherited finding: P1-04
Reserved focused gate: `provider-egress-containment`

## Purpose

Define provider network authority and diagnostic-data containment as one
runtime security boundary, independent of model labels and P086 semantics.

## Owned scope

- Default denial of direct provider DNS, TCP, UDP, QUIC, inbound listeners,
  undeclared Unix sockets, ambient proxies, and custom trust roots.
- A generation-bound loopback egress broker with exact provider/adapter digest,
  HTTPS CONNECT host and port allowlist, DNS/IP-class policy, system TLS trust,
  redirect policy, byte/time/connection budgets, expiry, and nonce.
- Revalidation on every CONNECT and redirect.
- Seatbelt and descendant-helper proof that no direct path bypasses the broker.
- Omission of Claude `debugFile` in ordinary, P079, and P086 launches.
- Bounded sanitized raw-message diagnostics, private retention, no-follow
  cleanup, purge limits, and failed-serve behavior for malformed legacy files.
- Metrics limited to bounded reason codes and counters, never endpoint secrets,
  session IDs, raw payloads, or paths.

## Required proof when scheduled

- DNS rebinding, forbidden IP class, redirect, custom CA, proxy override,
  direct socket, helper descendant, and local-socket negatives.
- Allowed pinned endpoint through the broker on every supported macOS release.
- Debug sentinel proving raw secrets never reach logs, artifacts, reports, or
  operator copy, plus bounded startup purge fixtures.

## Activation rule

A later security-reviewed proposal must select one provider adapter as the
first implementation cut. No broad provider egress change belongs to the
active model-variant proposal.
