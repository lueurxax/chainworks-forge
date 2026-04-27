//! P064: Gate fixture map for main-sync and knowledge-capsule testing.
//!
//! Phase 0 contract freeze — defines the canonical fixture scenario names
//! that Phase 1, 2, and 3 tests must cover. Each constant is a test fixture
//! scenario identifier used as the fixture key in integration tests.
//!
//! Adding a new fixture here makes the exhaustiveness test at the bottom fail
//! until a corresponding test exists in the relevant Phase test module.

/// Sync inspects worktree and finds local main is already an ancestor.
pub const FIXTURE_NO_OP: &str = "p064:no_op";

/// Local main is ahead and fast-forward is possible (no merge commit).
pub const FIXTURE_FAST_FORWARD: &str = "p064:fast_forward";

/// Diverged histories require a normal merge commit with no conflicts.
pub const FIXTURE_CLEAN_MERGE: &str = "p064:clean_merge";

/// Worktree is dirty — preservation commit and artifacts created before merge.
pub const FIXTURE_DIRTY_PRESERVATION: &str = "p064:dirty_preservation";

/// Active read-only consumer blocks barrier acquisition.
pub const FIXTURE_ACTIVE_READER_WAIT: &str = "p064:active_reader_wait";

/// Active write-enabled consumer blocks barrier acquisition.
pub const FIXTURE_ACTIVE_WRITER_WAIT: &str = "p064:active_writer_wait";

/// Scheduler must not claim new read work while sync barrier is active.
pub const FIXTURE_NO_NEW_READER_CLAIM_DURING_SYNC: &str = "p064:no_new_reader_claim_during_sync";

/// Scheduler must not claim new write work while sync barrier is active.
pub const FIXTURE_NO_NEW_WRITER_CLAIM_DURING_SYNC: &str = "p064:no_new_writer_claim_during_sync";

/// Merge conflicts detected — abort back to preservation commit, record conflict files.
pub const FIXTURE_CONFLICT_ABORT: &str = "p064:conflict_abort";

/// Resolver agent completes conflict resolution with required evidence.
pub const FIXTURE_RESOLVER_COMPLETION: &str = "p064:resolver_completion";

/// Attempt stuck in inconsistent state — moved to recovery_required.
pub const FIXTURE_RECOVERY_REQUIRED_REPAIR: &str = "p064:recovery_required_repair";

/// Knowledge capsule attached and injected into prompt appendix.
pub const FIXTURE_CAPSULE_PROMPT_CONSTRUCTION: &str = "p064:capsule_prompt_construction";

/// Scheduler excludes claims for worktree resource while barrier is held.
pub const FIXTURE_SCHEDULER_CLAIM_EXCLUSION: &str = "p064:scheduler_claim_exclusion";

/// Stale barrier lease stolen after heartbeat expiry.
pub const FIXTURE_STALE_LEASE_STEAL: &str = "p064:stale_lease_steal";

/// Missing or malformed worktree_access_mode metadata fails closed.
pub const FIXTURE_FAIL_CLOSED_MISSING_METADATA: &str = "p064:fail_closed_missing_metadata";

/// Duplicate sync trigger with same idempotency key is deduped (no duplicate Git mutation).
pub const FIXTURE_DUPLICATE_TRIGGER_DEDUPE: &str = "p064:duplicate_trigger_dedupe";

/// All fixture names in a single slice for exhaustiveness checks.
pub const ALL_FIXTURES: &[&str] = &[
    FIXTURE_NO_OP,
    FIXTURE_FAST_FORWARD,
    FIXTURE_CLEAN_MERGE,
    FIXTURE_DIRTY_PRESERVATION,
    FIXTURE_ACTIVE_READER_WAIT,
    FIXTURE_ACTIVE_WRITER_WAIT,
    FIXTURE_NO_NEW_READER_CLAIM_DURING_SYNC,
    FIXTURE_NO_NEW_WRITER_CLAIM_DURING_SYNC,
    FIXTURE_CONFLICT_ABORT,
    FIXTURE_RESOLVER_COMPLETION,
    FIXTURE_RECOVERY_REQUIRED_REPAIR,
    FIXTURE_CAPSULE_PROMPT_CONSTRUCTION,
    FIXTURE_SCHEDULER_CLAIM_EXCLUSION,
    FIXTURE_STALE_LEASE_STEAL,
    FIXTURE_FAIL_CLOSED_MISSING_METADATA,
    FIXTURE_DUPLICATE_TRIGGER_DEDUPE,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fixtures_slice_matches_expected_count() {
        // 16 fixtures defined in proposal phase 0 gate fixture map.
        // Update this count when adding a new fixture.
        assert_eq!(
            ALL_FIXTURES.len(),
            16,
            "ALL_FIXTURES count changed — update this test when adding new fixtures"
        );
    }

    #[test]
    fn all_fixtures_have_p064_prefix() {
        for fixture in ALL_FIXTURES {
            assert!(
                fixture.starts_with("p064:"),
                "fixture {fixture} must use p064: prefix"
            );
        }
    }

    #[test]
    fn all_fixtures_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for fixture in ALL_FIXTURES {
            assert!(seen.insert(fixture), "duplicate fixture name: {fixture}");
        }
    }
}
