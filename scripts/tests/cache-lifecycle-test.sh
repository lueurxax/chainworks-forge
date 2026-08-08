#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/chainworks-cache-lifecycle.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

COUNT_FILE="$SCRATCH/cleanup-count"
FAKE_CLEANER="$SCRATCH/fake-cleaner.sh"
mkdir -p "$SCRATCH/home"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf x >> "$CHAINWORKS_TEST_COUNT_FILE"' \
    'exit "${CHAINWORKS_TEST_CLEAN_EXIT:-0}"' \
    > "$FAKE_CLEANER"
chmod +x "$FAKE_CLEANER"

run_auto_cleanup() {
    env \
        HOME="$SCRATCH/home" \
        CHAINWORKS_AUTO_CACHE_CLEANUP=1 \
        CHAINWORKS_CACHE_MAINTENANCE_DIR="$SCRATCH/maintenance" \
        CHAINWORKS_CACHE_CLEANUP_COMMAND="$FAKE_CLEANER" \
        CHAINWORKS_TEST_COUNT_FILE="$COUNT_FILE" \
        "$ROOT_DIR/scripts/maybe-clean-build-caches.sh"
}

assert_cleanup_count() {
    local expected="$1"
    local actual=0
    if [[ -f "$COUNT_FILE" ]]; then
        actual="$(wc -c < "$COUNT_FILE" | tr -d '[:space:]')"
    fi
    if [[ "$actual" != "$expected" ]]; then
        printf 'expected cleanup count %s, got %s\n' "$expected" "$actual" >&2
        exit 1
    fi
}

# Automatic cleanup runs once and is then throttled.
run_auto_cleanup
run_auto_cleanup
assert_cleanup_count 1

# A zero interval forces another maintenance pass.
CHAINWORKS_AUTO_CACHE_CLEANUP_INTERVAL_SECONDS=0 run_auto_cleanup
assert_cleanup_count 2

# A held lock prevents concurrent cleanup.
mkdir -p "$SCRATCH/maintenance/cleanup.lock"
CHAINWORKS_AUTO_CACHE_CLEANUP_INTERVAL_SECONDS=0 run_auto_cleanup
assert_cleanup_count 2
rmdir "$SCRATCH/maintenance/cleanup.lock"

# Cleanup failures fail open and are retried because no success stamp is written.
rm -f "$SCRATCH/maintenance/last-success"
CHAINWORKS_TEST_CLEAN_EXIT=1 run_auto_cleanup 2>/dev/null
assert_cleanup_count 3
run_auto_cleanup
assert_cleanup_count 4

# A concurrency deferral is quiet and is retried without writing a success stamp.
rm -f "$SCRATCH/maintenance/last-success"
deferred_output="$(
    CHAINWORKS_TEST_CLEAN_EXIT=75 run_auto_cleanup 2>&1
)"
assert_cleanup_count 5
if [[ -n "$deferred_output" ]]; then
    printf 'deferred cleanup emitted an unexpected warning: %s\n' "$deferred_output" >&2
    exit 1
fi
run_auto_cleanup
assert_cleanup_count 6

# Cache maintenance must fail open when its state directory is unavailable.
printf 'not-a-directory\n' > "$SCRATCH/blocked-maintenance"
if ! env \
    HOME="$SCRATCH/home" \
    CHAINWORKS_AUTO_CACHE_CLEANUP=1 \
    CHAINWORKS_CACHE_MAINTENANCE_DIR="$SCRATCH/blocked-maintenance/state" \
    CHAINWORKS_CACHE_CLEANUP_COMMAND="$FAKE_CLEANER" \
    CHAINWORKS_TEST_COUNT_FILE="$COUNT_FILE" \
    "$ROOT_DIR/scripts/maybe-clean-build-caches.sh" >/dev/null 2>&1; then
    printf 'automatic cache cleanup did not fail open for unavailable state storage\n' >&2
    exit 1
fi

# Managed Cargo invokes cleanup and disables duplicate incremental artifacts.
rm -f "$COUNT_FILE"
managed_output="$(
    env \
        HOME="$SCRATCH/home" \
        CHAINWORKS_AUTO_CACHE_CLEANUP=1 \
        CHAINWORKS_CACHE_MAINTENANCE_DIR="$SCRATCH/managed-maintenance" \
        CHAINWORKS_CACHE_CLEANUP_COMMAND="$FAKE_CLEANER" \
        CHAINWORKS_TEST_COUNT_FILE="$COUNT_FILE" \
        CHAINWORKS_CARGO_WRAPPER=0 \
        "$ROOT_DIR/scripts/cargo-managed" --print-env
)"
assert_cleanup_count 1
if ! grep -q '^CARGO_INCREMENTAL=0$' <<< "$managed_output"; then
    printf 'cargo-managed did not disable incremental artifacts:\n%s\n' "$managed_output" >&2
    exit 1
fi

# Cleanup must preserve targets while any artifact-writing Cargo command runs.
ACTIVE_TARGET="$SCRATCH/active-target"
ACTIVE_MARKER="$ACTIVE_TARGET/debug/incremental/keep/marker"
mkdir -p "$(dirname "$ACTIVE_MARKER")"
printf 'keep\n' > "$ACTIVE_MARKER"
printf '123 /usr/bin/cargo check --workspace\n' > "$SCRATCH/process-list"
env \
    HOME="$SCRATCH/home" \
    CHAINWORKS_AUTO_CACHE_CLEANUP=1 \
    CHAINWORKS_CARGO_WRAPPER=0 \
    CHAINWORKS_SHARED_CARGO_TARGET_DIR="$ACTIVE_TARGET" \
    CHAINWORKS_GATE_CARGO_TARGET_ROOT="$ACTIVE_TARGET/gates" \
    CHAINWORKS_SHARED_CARGO_TARGET_MAX_GB=0 \
    CHAINWORKS_CACHE_PROCESS_LIST_FILE="$SCRATCH/process-list" \
    "$ROOT_DIR/scripts/clean-build-caches.sh" >/dev/null
if [[ ! -f "$ACTIVE_MARKER" ]]; then
    printf 'cleanup removed an active cargo check target\n' >&2
    exit 1
fi

set +e
env \
    HOME="$SCRATCH/home" \
    CHAINWORKS_AUTO_CACHE_CLEANUP=1 \
    CHAINWORKS_CARGO_WRAPPER=0 \
    CHAINWORKS_SHARED_CARGO_TARGET_DIR="$ACTIVE_TARGET" \
    CHAINWORKS_GATE_CARGO_TARGET_ROOT="$ACTIVE_TARGET/gates" \
    CHAINWORKS_SHARED_CARGO_TARGET_MAX_GB=0 \
    CHAINWORKS_CACHE_PROCESS_LIST_FILE="$SCRATCH/process-list" \
    CHAINWORKS_CACHE_REQUIRE_CARGO_MAINTENANCE=1 \
    "$ROOT_DIR/scripts/clean-build-caches.sh" >/dev/null
deferred_status=$?
set -e
if [[ "$deferred_status" -ne 75 ]]; then
    printf 'expected active Cargo cleanup deferral status 75, got %s\n' "$deferred_status" >&2
    exit 1
fi

# A wrapper may ignore only its own PID so it can service cleanup before exec.
SELF_TARGET="$SCRATCH/self-target"
SELF_MARKER="$SELF_TARGET/debug/incremental/remove/marker"
mkdir -p "$(dirname "$SELF_MARKER")"
printf 'remove\n' > "$SELF_MARKER"
env \
    HOME="$SCRATCH/home" \
    CHAINWORKS_CARGO_WRAPPER=0 \
    CHAINWORKS_SHARED_CARGO_TARGET_DIR="$SELF_TARGET" \
    CHAINWORKS_GATE_CARGO_TARGET_ROOT="$SELF_TARGET/gates" \
    CHAINWORKS_SHARED_CARGO_TARGET_MAX_GB=0 \
    CHAINWORKS_CACHE_PROCESS_LIST_FILE="$SCRATCH/process-list" \
    CHAINWORKS_CACHE_IGNORE_PID=123 \
    "$ROOT_DIR/scripts/clean-build-caches.sh" >/dev/null
if [[ -f "$SELF_MARKER" ]]; then
    printf 'cleanup treated its initiating cargo wrapper as a competing build\n' >&2
    exit 1
fi

# Every test-gate entry point also services the bounded cache lifecycle.
rm -f "$COUNT_FILE"
env \
    HOME="$SCRATCH/home" \
    CHAINWORKS_AUTO_CACHE_CLEANUP=1 \
    CHAINWORKS_CACHE_MAINTENANCE_DIR="$SCRATCH/gate-maintenance" \
    CHAINWORKS_CACHE_CLEANUP_COMMAND="$FAKE_CLEANER" \
    CHAINWORKS_TEST_COUNT_FILE="$COUNT_FILE" \
    "$ROOT_DIR/scripts/test-gate.sh" list >/dev/null
assert_cleanup_count 1

if ! grep -Fq '"$ROOT_DIR/scripts/tests/cache-lifecycle-test.sh"' "$ROOT_DIR/scripts/test-gate.sh"; then
    printf 'guardrails do not execute the cache lifecycle regression test\n' >&2
    exit 1
fi

printf 'cache lifecycle tests passed\n'
