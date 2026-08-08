#!/usr/bin/env bash
# Build-cache hygiene for Chainworks Forge.
#
# Reclaims disk used by stale build outputs while preserving active builds.
# The shared policy defaults Cargo incremental compilation off and keeps
# dependency/sccache reuse, so deleting stale incremental trees does not remove
# the canonical reusable compiler cache.
#
# Usage:
#   ./scripts/clean-build-caches.sh [--dry-run] [--aggressive] [--protect-worktree NAME]
#
# Default pass removes:
#   * `control-plane/target/proposal-*` / `target/p*-*` orphan
#     CARGO_TARGET_DIRs from proposal gates, including the same pattern
#     under `.chainworks/worktrees/*/control-plane/target`, unless cargo
#     is actively building/testing.
#   * Shared gate CARGO_TARGET_DIRs under
#     `~/Library/Caches/Chainworks Forge/cargo-target/gates/*`, also only
#     when cargo is not actively building/testing.
#   * Shared/worktree Cargo `debug/incremental` directories when their target
#     roots exceed the configured pressure budget. This keeps the common cache
#     useful while preventing long-lived agent targets from growing without
#     bound.
#   * Shared reusable `agents/debug` and `debug` target profiles when the shared
#     root is still above budget after lighter cleanup. This costs a rebuild but
#     keeps ENOSPC from recurring when deps/build outputs dwarf incremental data.
#   * `$TMPDIR/chainworks-test-gates` DerivedData, xcresult, and target
#     residuals not referenced by currently running xcodebuild commands.
#   * All but the most recently modified
#     `~/Library/Developer/Xcode/DerivedData/Chainworks_Forge-*` root.
#
# `--aggressive` additionally removes:
#   * `control-plane/target/debug/incremental` (regenerates on next
#     `cargo test`; costs ~2 min first compile).
#   * `control-plane/target/release` (only used by packaging lane /
#     Xcode Release config).

set -euo pipefail

DRY_RUN=0
AGGRESSIVE=0
PROTECTED_WORKTREES=()
while [[ "$#" -gt 0 ]]; do
    arg="$1"
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        --aggressive) AGGRESSIVE=1 ;;
        --protect-worktree=*) PROTECTED_WORKTREES+=("${arg#--protect-worktree=}") ;;
        --protect-worktree)
            shift
            if [[ "$#" -eq 0 || "${1:-}" == --* ]]; then
                echo "error: --protect-worktree requires NAME or --protect-worktree=NAME" >&2
                exit 64
            fi
            PROTECTED_WORKTREES+=("$1")
            ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "error: unknown argument: $arg" >&2
            exit 64
            ;;
    esac
    shift
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_BASE="${TMPDIR:-/tmp}/chainworks-test-gates"
WORKTREE_ROOT="$ROOT_DIR/.chainworks/worktrees"
if [[ -f "$ROOT_DIR/scripts/cargo-cache-env.sh" ]]; then
    CHAINWORKS_CARGO_CACHE_REPO_ROOT="$ROOT_DIR"
    # shellcheck source=scripts/cargo-cache-env.sh
    source "$ROOT_DIR/scripts/cargo-cache-env.sh"
fi
SHARED_CARGO_TARGET_DIR="${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-$(chainworks_default_cargo_target_dir)}"
SHARED_GATE_TARGET_ROOT="${CHAINWORKS_GATE_CARGO_TARGET_ROOT:-$SHARED_CARGO_TARGET_DIR/gates}"
SHARED_TARGET_MAX_GB="${CHAINWORKS_SHARED_CARGO_TARGET_MAX_GB:-32}"
LOCAL_TARGET_MAX_GB="${CHAINWORKS_LOCAL_CARGO_TARGET_MAX_GB:-8}"
WORKTREE_TARGET_MAX_GB="${CHAINWORKS_WORKTREE_CARGO_TARGET_MAX_GB:-8}"
CARGO_CLEANUP_DEFERRED=0

if [[ -n "${CHAINWORKS_CLEAN_PROTECTED_WORKTREES:-}" ]]; then
    IFS=' :' read -r -a _env_protected_worktrees <<< "${CHAINWORKS_CLEAN_PROTECTED_WORKTREES}"
    PROTECTED_WORKTREES+=("${_env_protected_worktrees[@]}")
fi

say() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        echo "[dry-run] $*"
    else
        echo "$*"
    fi
}

delete() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        echo "[dry-run] rm -rf $1"
    else
        rm -rf "$1"
    fi
}

du_kib() {
    local path="$1"
    [[ -e "$path" ]] || {
        echo 0
        return 0
    }
    du -sk "$path" 2>/dev/null | awk '{print $1}'
}

gb_to_kib() {
    local gb="$1"
    awk -v gb="$gb" 'BEGIN { printf "%.0f\n", gb * 1024 * 1024 }'
}

load_process_list() {
    if [[ -n "${CHAINWORKS_CACHE_PROCESS_LIST_FILE:-}" ]]; then
        cat "$CHAINWORKS_CACHE_PROCESS_LIST_FILE"
        return $?
    fi
    ps -axo pid=,command= 2>/dev/null
}

is_cargo_running() {
    local processes
    local ignored_pid="${CHAINWORKS_CACHE_IGNORE_PID:-}"
    if ! processes="$(load_process_list)"; then
        # Process visibility is a safety boundary. Unknown means active.
        return 0
    fi
    printf '%s\n' "$processes" |
        awk -v self="$$" -v ignored="$ignored_pid" '
            $1 == self || (ignored != "" && $1 == ignored) { next }
            /(^|[\/[:space:]])cargo[[:space:]]+(test|build|check|clippy|run|bench|doc|rustc)([[:space:]]|$)/ { found = 1; exit }
            /(^|[\/[:space:]])rustc([[:space:]]|$)/ { found = 1; exit }
            END { exit(found ? 0 : 1) }
        '
}

load_active_tmp_paths() {
    local processes
    ACTIVE_TMP_PATHS=()
    if ! processes="$(load_process_list)"; then
        return 1
    fi
    while IFS= read -r path; do
        [[ -n "$path" ]] || continue
        ACTIVE_TMP_PATHS+=("$path")
    done < <(
        printf '%s\n' "$processes" |
            awk -v base="$TMP_BASE/" '
                /xcodebuild/ {
                    for (i = 1; i <= NF; i++) {
                        if (index($i, base) == 1) print $i
                    }
                }
            ' |
            sort -u
    )
}

is_active_tmp_path() {
    local candidate="$1"
    local active
    for active in "${ACTIVE_TMP_PATHS[@]:-}"; do
        if [[ "$candidate" == "$active" ]]; then
            return 0
        fi
    done
    return 1
}

is_protected_worktree() {
    local worktree_name="$1"
    local protected
    for protected in "${PROTECTED_WORKTREES[@]:-}"; do
        [[ -n "$protected" ]] || continue
        if [[ "$worktree_name" == "$protected" ]]; then
            return 0
        fi
    done
    return 1
}

clean_incremental_under() {
    local target_root="$1"
    [[ -d "$target_root" ]] || return 0

    local incremental
    while IFS= read -r incremental; do
        [[ -d "$incremental" ]] || continue
        say "removing Cargo incremental cache $incremental"
        delete "$incremental"
    done < <(find "$target_root" -path '*/debug/incremental' -type d -prune 2>/dev/null)
}

clean_target_pressure() {
    local target_root="$1"
    local max_gb="$2"
    local label="$3"
    [[ -d "$target_root" ]] || return 0

    local size_kib max_kib
    size_kib="$(du_kib "$target_root")"
    max_kib="$(gb_to_kib "$max_gb")"
    if [[ "$size_kib" -le "$max_kib" ]]; then
        return 0
    fi

    say "$label Cargo target is above pressure budget (${size_kib} KiB > ${max_kib} KiB); cleaning incremental caches"
    clean_incremental_under "$target_root"
}

target_root_over_budget() {
    local target_root="$1"
    local max_gb="$2"
    [[ -d "$target_root" ]] || return 1

    local size_kib max_kib
    size_kib="$(du_kib "$target_root")"
    max_kib="$(gb_to_kib "$max_gb")"
    [[ "$size_kib" -gt "$max_kib" ]]
}

prune_child_dirs_until_budget() {
    local budget_root="$1"
    local child_root="$2"
    local max_gb="$3"
    local label="$4"
    [[ -d "$budget_root" && -d "$child_root" ]] || return 0

    local child
    if [[ "$DRY_RUN" -eq 1 ]]; then
        while IFS= read -r child; do
            [[ -n "$child" && -d "$child" ]] || continue
            say "$label is above pressure budget; removing oldest reusable Cargo target $child"
            delete "$child"
        done < <(find "$child_root" -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null |
            xargs -0 ls -dtr 2>/dev/null)
        return 0
    fi

    while target_root_over_budget "$budget_root" "$max_gb"; do
        child="$(find "$child_root" -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null |
            xargs -0 ls -dtr 2>/dev/null |
            head -n 1 || true)"
        [[ -n "$child" && -d "$child" ]] || break
        say "$label is above pressure budget; removing oldest reusable Cargo target $child"
        delete "$child"
    done
}

prune_specific_dirs_until_budget() {
    local budget_root="$1"
    local max_gb="$2"
    local label="$3"
    shift 3
    [[ -d "$budget_root" ]] || return 0

    local candidate
    for candidate in "$@"; do
        if ! target_root_over_budget "$budget_root" "$max_gb"; then
            return 0
        fi
        [[ -d "$candidate" ]] || continue
        say "$label is above pressure budget; removing reusable Cargo target $candidate"
        delete "$candidate"
    done
}

clean_orphan_target_dirs() {
    local target_root="$1"
    [[ -d "$target_root" ]] || return 0

    # Collect every direct child of target/ that is neither debug,
    # release, nor a cargo metadata file. Those are per-proposal
    # CARGO_TARGET_DIRs used by gates and ad-hoc retry/refine passes.
    for entry in "$target_root"/*; do
        [[ -d "$entry" ]] || continue
        name="$(basename "$entry")"
        case "$name" in
            debug|release|doc|tmp|xcode-local|xcode-shared|sccache) continue ;;
        esac
        say "removing orphan target dir $entry"
        delete "$entry"
    done
}

clean_worktree_adhoc_target_dirs() {
    local worktree="$1"
    [[ -d "$worktree" ]] || return 0

    local entry name
    for entry in "$worktree"/target-* "$worktree"/control-plane/target-*; do
        [[ -d "$entry" ]] || continue
        name="$(basename "$entry")"
        case "$name" in
            target-debug|target-release) continue ;;
        esac
        say "removing worktree ad-hoc target dir $entry"
        delete "$entry"
    done
}

# ── 1. Orphan per-proposal target dirs ───────────────────────────────
TARGET_ROOT="$ROOT_DIR/control-plane/target"
if is_cargo_running; then
    CARGO_CLEANUP_DEFERRED=1
    say "cargo build/test is running; skipping Cargo target cleanup"
else
    clean_orphan_target_dirs "$TARGET_ROOT"
    clean_target_pressure "$TARGET_ROOT" "$LOCAL_TARGET_MAX_GB" "local"
    prune_specific_dirs_until_budget \
        "$TARGET_ROOT" \
        "$LOCAL_TARGET_MAX_GB" \
        "local Cargo target" \
        "$TARGET_ROOT/debug" \
        "$TARGET_ROOT/release"

    clean_target_pressure "$SHARED_CARGO_TARGET_DIR" "$SHARED_TARGET_MAX_GB" "shared"
    prune_child_dirs_until_budget "$SHARED_CARGO_TARGET_DIR" "$SHARED_GATE_TARGET_ROOT" "$SHARED_TARGET_MAX_GB" "shared Cargo target"
    prune_specific_dirs_until_budget \
        "$SHARED_CARGO_TARGET_DIR" \
        "$SHARED_TARGET_MAX_GB" \
        "shared Cargo target" \
        "$SHARED_CARGO_TARGET_DIR/agents/debug" \
        "$SHARED_CARGO_TARGET_DIR/agents" \
        "$SHARED_CARGO_TARGET_DIR/debug"

    if [[ -d "$WORKTREE_ROOT" ]]; then
        for worktree in "$WORKTREE_ROOT"/*; do
            [[ -d "$worktree" ]] || continue
            worktree_name="$(basename "$worktree")"
            if is_protected_worktree "$worktree_name"; then
                say "keeping protected worktree target caches $worktree_name"
                continue
            fi
            clean_worktree_adhoc_target_dirs "$worktree"
            clean_orphan_target_dirs "$worktree/control-plane/target"
            clean_target_pressure "$worktree/control-plane/target" "$WORKTREE_TARGET_MAX_GB" "worktree $worktree_name"
        done
    fi
fi

# ── 2. Swift gate residuals ──────────────────────────────────────────
if [[ -d "$TMP_BASE" ]]; then
    if load_active_tmp_paths; then
        for pat in "*-DerivedData" "*.xcresult" "proposal-0*-target" "p*-regression-target" "p*-readloop-targeted-DerivedData"; do
            # shellcheck disable=SC2086
            for stale in "$TMP_BASE"/${pat}; do
                [[ -e "$stale" ]] || continue
                if is_active_tmp_path "$stale"; then
                    say "keeping active xcodebuild path $stale"
                    continue
                fi
                say "removing stale Swift gate residual $stale"
                delete "$stale"
            done
        done
    else
        say "process visibility is unavailable; skipping Swift gate residual cleanup"
    fi
fi

# ── 3. Xcode DerivedData (keep newest) ───────────────────────────────
DD_ROOT="$HOME/Library/Developer/Xcode/DerivedData"
if [[ -d "$DD_ROOT" ]]; then
    NEWEST_DD="$(ls -dt "$DD_ROOT"/Chainworks_Forge-* 2>/dev/null | head -n 1 || true)"
    for entry in "$DD_ROOT"/Chainworks_Forge-*; do
        [[ -d "$entry" ]] || continue
        if [[ "$entry" != "$NEWEST_DD" ]]; then
            say "removing stale Xcode DerivedData $entry"
            delete "$entry"
        fi
    done
fi

# ── 4. --aggressive: incremental + release ───────────────────────────
if [[ "$AGGRESSIVE" -eq 1 ]]; then
    if is_cargo_running; then
        CARGO_CLEANUP_DEFERRED=1
        say "cargo build/test is running; skipping aggressive target cleanup"
    else
        if [[ -d "$TARGET_ROOT/debug/incremental" ]]; then
            say "removing target/debug/incremental (regenerates on next cargo run)"
            delete "$TARGET_ROOT/debug/incremental"
        fi
        if [[ -d "$TARGET_ROOT/release" ]]; then
            say "removing target/release (only used by packaging lane / Xcode Release)"
            delete "$TARGET_ROOT/release"
        fi
    fi
fi

# ── 5. Summary ────────────────────────────────────────────────────────
if [[ -d "$TARGET_ROOT" ]]; then
    echo ""
    echo "control-plane/target final size:"
    du -sh "$TARGET_ROOT" 2>/dev/null || true
fi
if [[ -d "$SHARED_CARGO_TARGET_DIR" ]]; then
    echo "shared Cargo target final size:"
    du -sh "$SHARED_CARGO_TARGET_DIR" 2>/dev/null || true
fi
if [[ -d "$SHARED_GATE_TARGET_ROOT" ]]; then
    echo "shared gate Cargo targets final size:"
    du -sh "$SHARED_GATE_TARGET_ROOT" 2>/dev/null || true
fi
if [[ -d "$TMP_BASE" ]]; then
    echo "chainworks-test-gates final size:"
    du -sh "$TMP_BASE" 2>/dev/null || true
fi
if [[ -d "$DD_ROOT" ]]; then
    echo "Xcode DerivedData Chainworks_Forge final size:"
    du -sh "$DD_ROOT"/Chainworks_Forge-* 2>/dev/null | tail -n 1 || true
fi

if [[ "$CARGO_CLEANUP_DEFERRED" -eq 1 &&
    "${CHAINWORKS_CACHE_REQUIRE_CARGO_MAINTENANCE:-0}" =~ ^(1|true|TRUE|yes|YES)$ ]]; then
    exit 75
fi
