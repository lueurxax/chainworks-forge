# P031 Report Payload Priority Decision

Status: READY
Owner: P031 release owner
Blocking Phase: Phase 0d
Decision Date: 2026-04-24
Last Updated: 2026-04-25T04:27:00Z

## Default Decision

Full GraphQL report payload rendering remains a P0 follow-up unless Phase 0d evidence justifies a downgrade.

## Evidence Required To Downgrade

- Report inspection frequency from representative dogfood or operator usage.
- Operator workflow notes showing metadata-only report inspection is sufficient.
- Release-owner decision that names the follow-up priority and review date.

## Decision

Decision: keep full GraphQL report payload rendering as `P031-FOLLOWUP-REPORT-PAYLOAD`, priority P0.

Reason: no local dogfood or representative operator usage evidence is available in this tree to justify a downgrade from the proposal default. Live GraphQL evidence confirms that current report rows expose metadata only and defer payload rendering with `PAYLOAD_DEFERRED_BY_P031`; that validates the P031 metadata-only contract but does not justify lowering the payload follow-up priority.

Live evidence:

- `docs/evidence/p031-runtime/report-payload-live-evidence-2026-04-25.json`
- Run `6ad4e80a-8341-42a5-9809-849f98d79779` returned 876 artifacts and 34 report metadata rows.
- Report metadata rows had `payloadAvailabilityState=metadata_only` and `payloadUnavailableReasonCode=PAYLOAD_DEFERRED_BY_P031`.

Owner: P031 release owner.
Next review: Phase 0d exit or Phase 3 sign-off, whichever first has representative report-inspection evidence.
