#!/usr/bin/env bash
# Throttled, lock-safe entry point for the bounded build-cache policy.

set -euo pipefail

if [[ "${CHAINWORKS_AUTO_CACHE_CLEANUP:-1}" =~ ^(0|false|FALSE|off|OFF|no|NO)$ ]]; then
    exit 0
fi

ROOT_DIR="${CHAINWORKS_CARGO_CACHE_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MAINTENANCE_DIR="${CHAINWORKS_CACHE_MAINTENANCE_DIR:-${HOME:-/tmp}/Library/Caches/Chainworks Forge/cache-maintenance}"
CLEANUP_COMMAND="${CHAINWORKS_CACHE_CLEANUP_COMMAND:-$ROOT_DIR/scripts/clean-build-caches.sh}"
INTERVAL_SECONDS="${CHAINWORKS_AUTO_CACHE_CLEANUP_INTERVAL_SECONDS:-900}"
LOCK_DIR="$MAINTENANCE_DIR/cleanup.lock"
LOCK_PID="$LOCK_DIR/pid"
SUCCESS_STAMP="$MAINTENANCE_DIR/last-success"
LOG_PATH="$MAINTENANCE_DIR/last-cleanup.log"

if [[ ! "$INTERVAL_SECONDS" =~ ^[0-9]+$ ]]; then
    printf 'warning: invalid CHAINWORKS_AUTO_CACHE_CLEANUP_INTERVAL_SECONDS=%q; using 900\n' \
        "$INTERVAL_SECONDS" >&2
    INTERVAL_SECONDS=900
fi

if ! mkdir -p "$MAINTENANCE_DIR" 2>/dev/null; then
    printf 'warning: Chainworks cache maintenance state is unavailable: %s\n' \
        "$MAINTENANCE_DIR" >&2
    exit 0
fi

now="$(date +%s)"
last_success=0
if [[ -f "$SUCCESS_STAMP" ]]; then
    read -r last_success < "$SUCCESS_STAMP" || last_success=0
fi
if [[ ! "$last_success" =~ ^[0-9]+$ ]]; then
    last_success=0
fi
if (( INTERVAL_SECONDS > 0 && now - last_success < INTERVAL_SECONDS )); then
    exit 0
fi

acquire_lock() {
    if mkdir "$LOCK_DIR" 2>/dev/null; then
        if ! printf '%s\n' "$$" > "$LOCK_PID"; then
            rmdir "$LOCK_DIR" 2>/dev/null || true
            return 1
        fi
        return 0
    fi

    local holder=""
    if [[ -f "$LOCK_PID" ]]; then
        read -r holder < "$LOCK_PID" || holder=""
    fi
    if [[ "$holder" =~ ^[0-9]+$ ]] && ! kill -0 "$holder" 2>/dev/null; then
        rm -f "$LOCK_PID"
        if rmdir "$LOCK_DIR" 2>/dev/null && mkdir "$LOCK_DIR" 2>/dev/null; then
            if ! printf '%s\n' "$$" > "$LOCK_PID"; then
                rmdir "$LOCK_DIR" 2>/dev/null || true
                return 1
            fi
            return 0
        fi
    fi
    return 1
}

if ! acquire_lock; then
    exit 0
fi

release_lock() {
    rm -f "$LOCK_PID"
    rmdir "$LOCK_DIR" 2>/dev/null || true
}
trap release_lock EXIT

# Another process may have completed maintenance while this process waited.
last_success=0
if [[ -f "$SUCCESS_STAMP" ]]; then
    read -r last_success < "$SUCCESS_STAMP" || last_success=0
fi
if [[ "$last_success" =~ ^[0-9]+$ ]] &&
    (( INTERVAL_SECONDS > 0 && now - last_success < INTERVAL_SECONDS )); then
    exit 0
fi

if [[ ! -x "$CLEANUP_COMMAND" ]]; then
    printf 'warning: Chainworks cache cleanup command is not executable: %s\n' "$CLEANUP_COMMAND" >&2
    exit 0
fi

tmp_log="$LOG_PATH.tmp.$$"
cleanup_status=0
CHAINWORKS_CARGO_WRAPPER=0 \
    CHAINWORKS_CACHE_REQUIRE_CARGO_MAINTENANCE=1 \
    "$CLEANUP_COMMAND" >"$tmp_log" 2>&1 || cleanup_status=$?
if [[ "$cleanup_status" -eq 0 ]]; then
    mv "$tmp_log" "$LOG_PATH"
    if printf '%s\n' "$(date +%s)" > "$SUCCESS_STAMP.tmp.$$"; then
        mv "$SUCCESS_STAMP.tmp.$$" "$SUCCESS_STAMP" 2>/dev/null ||
            rm -f "$SUCCESS_STAMP.tmp.$$"
    fi
    if [[ "${CHAINWORKS_AUTO_CACHE_CLEANUP_VERBOSE:-0}" =~ ^(1|true|TRUE|yes|YES)$ ]]; then
        cat "$LOG_PATH" >&2
    fi
elif [[ "$cleanup_status" -eq 75 ]]; then
    mv "$tmp_log" "$LOG_PATH" 2>/dev/null || rm -f "$tmp_log"
else
    mv "$tmp_log" "$LOG_PATH" 2>/dev/null || true
    printf 'warning: Chainworks automatic cache cleanup failed with exit %s; see %s\n' \
        "$cleanup_status" "$LOG_PATH" >&2
fi

exit 0
