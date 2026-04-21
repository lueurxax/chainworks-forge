#!/usr/bin/env bash
# Build-cache hygiene for Chainworks Forge.
#
# Reclaims disk used by stale build outputs WITHOUT hurting the hot
# path for active work. Skips directories that cargo/xcodebuild
# actively rely on (current DerivedData, `target/debug/deps`) so the
# next build still re-uses the hot incremental cache.
#
# Usage:
#   ./scripts/clean-build-caches.sh [--dry-run] [--aggressive]
#
# Default pass removes:
#   * `control-plane/target/proposal-*` / `target/p*-*` orphan
#     CARGO_TARGET_DIRs from other proposals' gates.
#   * `$TMPDIR/chainworks-test-gates/p042-swift-*-DerivedData` stamps
#     older than the two most recent.
#   * `$TMPDIR/chainworks-test-gates/proposal-0*-DerivedData` and
#     `/*-target` dirs from other proposals' gates.
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
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        --aggressive) AGGRESSIVE=1 ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
    esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_BASE="${TMPDIR:-/tmp}/chainworks-test-gates"

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

bail_if_cargo_running() {
    if pgrep -lf 'cargo test|cargo build' >/dev/null 2>&1; then
        echo "error: cargo build/test is running; refusing to touch target/"
        echo "       rerun this script after the build finishes, or kill cargo first."
        exit 2
    fi
}

# ── 1. Orphan per-proposal target dirs ───────────────────────────────
TARGET_ROOT="$ROOT_DIR/control-plane/target"
if [[ -d "$TARGET_ROOT" ]]; then
    bail_if_cargo_running
    # Collect every direct child of target/ that is neither debug,
    # release, nor a cargo metadata file. Those are per-proposal
    # CARGO_TARGET_DIRs used by other gates.
    for entry in "$TARGET_ROOT"/*; do
        [[ -d "$entry" ]] || continue
        name="$(basename "$entry")"
        case "$name" in
            debug|release|doc|tmp) continue ;;
        esac
        say "removing orphan target dir $entry"
        delete "$entry"
    done
fi

# ── 2. Swift gate DerivedData prune (keep 2 newest) ──────────────────
if [[ -d "$TMP_BASE" ]]; then
    while IFS= read -r stale; do
        [[ -n "$stale" ]] || continue
        say "removing stale Swift DerivedData $stale"
        delete "$stale"
    done < <(ls -dt "$TMP_BASE"/p042-swift-*-DerivedData 2>/dev/null | tail -n +3)

    # Other proposals' residuals are never consumed by the P042 gate.
    for pat in "proposal-0*-DerivedData" "p*-readloop-targeted-DerivedData" "proposal-0*-target" "p*-regression-target"; do
        # shellcheck disable=SC2086
        for stale in "$TMP_BASE"/${pat}; do
            [[ -e "$stale" ]] || continue
            say "removing other-proposal residual $stale"
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
    bail_if_cargo_running
    if [[ -d "$TARGET_ROOT/debug/incremental" ]]; then
        say "removing target/debug/incremental (regenerates on next cargo run)"
        delete "$TARGET_ROOT/debug/incremental"
    fi
    if [[ -d "$TARGET_ROOT/release" ]]; then
        say "removing target/release (only used by packaging lane / Xcode Release)"
        delete "$TARGET_ROOT/release"
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
