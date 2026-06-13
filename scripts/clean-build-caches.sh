#!/usr/bin/env bash
# Build-cache hygiene for Chainworks Forge.
#
# Reclaims disk used by stale build outputs WITHOUT hurting the hot
# path for active work. Skips directories that cargo/xcodebuild
# actively rely on (current DerivedData, `target/debug/deps`) so the
# next build still re-uses the hot incremental cache.
#
# Usage:
#   ./scripts/clean-build-caches.sh [--dry-run] [--aggressive] [--protect-worktree NAME]
#
# Default pass removes:
#   * `control-plane/target/proposal-*` / `target/p*-*` orphan
#     CARGO_TARGET_DIRs from proposal gates, including the same pattern
#     under `.chainworks/worktrees/*/control-plane/target`, unless cargo
#     is actively building/testing.
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

is_cargo_running() {
    if pgrep -lf 'cargo test|cargo build' >/dev/null 2>&1; then
        return 0
    fi
    return 1
}

load_active_tmp_paths() {
    ACTIVE_TMP_PATHS=()
    while IFS= read -r path; do
        [[ -n "$path" ]] || continue
        ACTIVE_TMP_PATHS+=("$path")
    done < <(
        ps -axo command |
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

# ── 1. Orphan per-proposal target dirs ───────────────────────────────
TARGET_ROOT="$ROOT_DIR/control-plane/target"
if is_cargo_running; then
    say "cargo build/test is running; skipping Cargo target cleanup"
else
    clean_orphan_target_dirs "$TARGET_ROOT"

    if [[ -d "$WORKTREE_ROOT" ]]; then
        for worktree in "$WORKTREE_ROOT"/*; do
            [[ -d "$worktree" ]] || continue
            worktree_name="$(basename "$worktree")"
            if is_protected_worktree "$worktree_name"; then
                say "keeping protected worktree target caches $worktree_name"
                continue
            fi
            clean_orphan_target_dirs "$worktree/control-plane/target"
        done
    fi
fi

# ── 2. Swift gate residuals ──────────────────────────────────────────
if [[ -d "$TMP_BASE" ]]; then
    load_active_tmp_paths
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
if [[ -d "$TMP_BASE" ]]; then
    echo "chainworks-test-gates final size:"
    du -sh "$TMP_BASE" 2>/dev/null || true
fi
if [[ -d "$DD_ROOT" ]]; then
    echo "Xcode DerivedData Chainworks_Forge final size:"
    du -sh "$DD_ROOT"/Chainworks_Forge-* 2>/dev/null | tail -n 1 || true
fi
