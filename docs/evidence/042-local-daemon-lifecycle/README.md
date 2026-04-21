# Local Daemon Lifecycle Evidence

This directory collects the evidence logs produced by
the local daemon lifecycle gates.

The historical gate aliases are retained as stable operational names:

- `./scripts/test-gate.sh proposal-042` proves implementation readiness.
- `./scripts/test-gate.sh proposal-042-packaging` proves release-host packaging readiness.

The implemented contract is documented in
`docs/reference/local-daemon-lifecycle-supervision-and-packaging.md`.

## Implementation evidence

The implementation-ready proof log is:

```text
proposal-042-gate-20260420T063230Z.log
```

It records a green `proposal-042` run with focused Rust tests, Swift focused
tests, and full Rust workspace regression.

## Release-host packaging evidence

Each run writes a timestamped file of the form

```
release-gate-YYYYMMDDTHHMMSSZ.log
```

containing:

* Build / archive / export invocation and exit codes.
* `codesign -dvvv` output for the app bundle and the embedded
  `chainworks-forge-daemon`.
* `stapler validate` + `spctl --assess` outcomes.
* Team ID of the signing identity and whether it matched the
  `P042_EXPECTED_TEAM_ID` allow list.
* Notarization submission ID and (if applicable) the full
  `xcrun notarytool log` output.
* Launch-to-Ready proof: the packaged app is started, the lane waits
  for `~/Library/Application Support/Chainworks Forge/daemon.port`
  to appear, curls `/health`, and asserts the lifecycle state is
  `ready`; the daemon is then sent SIGTERM and the clean exit
  (status 0, within 5 s) is recorded.

A release is only considered ready for ship once the most recent log
in this directory records all checks as `PASS`. CI / release
automation should store these files as build artifacts alongside the
signed and notarized `.app` bundle.
