#!/usr/bin/env bash
# Shared Cargo cache policy for Chainworks Forge build scripts.
#
# Source this file from scripts that invoke Cargo from Xcode or other
# short-lived harnesses. It keeps Rust build products in a stable cache
# instead of per-invocation temp directories, and enables sccache when it
# is installed without making it a hard dependency.

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    echo "error: scripts/cargo-cache-env.sh must be sourced, not executed" >&2
    exit 64
fi

chainworks_cargo_cache_repo_root="${CHAINWORKS_CARGO_CACHE_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

chainworks_default_cargo_target_dir() {
    if [[ -n "${HOME:-}" ]]; then
        printf '%s\n' "${HOME}/Library/Caches/Chainworks Forge/cargo-target"
    else
        printf '%s\n' "${chainworks_cargo_cache_repo_root}/control-plane/target/xcode-shared"
    fi
}

chainworks_default_sccache_dir() {
    if [[ -n "${HOME:-}" ]]; then
        printf '%s\n' "${HOME}/Library/Caches/Chainworks Forge/sccache"
    else
        printf '%s\n' "${chainworks_cargo_cache_repo_root}/control-plane/target/sccache"
    fi
}

# Detect git worktrees and use a local target dir to prevent cache conflicts with the main
# workspace. Worktrees differ in source (e.g. P058 adds escalation modules absent in main),
# so a shared CARGO_TARGET_DIR built from main produces stale .rlib files for worktree builds,
# causing "unresolved import" errors at Xcode embed time. Worktrees always live under
# .chainworks/worktrees/; the main workspace path never contains that component.
_chainworks_effective_root="${SRCROOT:-${chainworks_cargo_cache_repo_root}}"
_chainworks_is_worktree=0
if [[ "${_chainworks_effective_root}" == *"/.chainworks/worktrees/"* ]]; then
    _chainworks_is_worktree=1
fi

if [[ -n "${CHAINWORKS_XCODE_CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${CHAINWORKS_XCODE_CARGO_TARGET_DIR}"
elif [[ -n "${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${CHAINWORKS_SHARED_CARGO_TARGET_DIR}"
elif [[ "${_chainworks_is_worktree}" == "1" ]]; then
    # Use a worktree-local target dir inside control-plane/target/ (already gitignored)
    # to avoid .rlib cache collisions with the main workspace build.
    export CARGO_TARGET_DIR="${_chainworks_effective_root}/control-plane/target/xcode-local"
elif [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="$(chainworks_default_cargo_target_dir)"
fi

mkdir -p "${CARGO_TARGET_DIR}"

case "${CHAINWORKS_CARGO_SCCACHE:-auto}" in
    0|false|FALSE|off|OFF|no|NO)
        ;;
    *)
        if [[ -z "${RUSTC_WRAPPER:-}" ]] && command -v sccache >/dev/null 2>&1; then
            export SCCACHE_DIR="${SCCACHE_DIR:-$(chainworks_default_sccache_dir)}"
            export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-20G}"
            mkdir -p "${SCCACHE_DIR}"
            export RUSTC_WRAPPER="$(command -v sccache)"
        fi
        ;;
esac
