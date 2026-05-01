-- P077 BLK-011: Persist soft-convergence blocker_digest across state-9 evaluations.
--
-- The synthesizer compares the current blocker digest against the previously
-- persisted value to detect soft convergence (repeated identical blockers
-- without diff or gate progress). When equal, routing switches from
-- ReturnToCodeRefine to AwaitOperatorDecision without claiming P052 hard
-- budget exhaustion. Outside of unit tests, this requires the prior digest
-- to be available on the next evaluation — hence persistence on the readiness
-- generation row.

ALTER TABLE closeout_gate_generations ADD COLUMN blocker_digest TEXT;
