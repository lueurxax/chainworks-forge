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

if [[ -n "${CHAINWORKS_XCODE_CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${CHAINWORKS_XCODE_CARGO_TARGET_DIR}"
elif [[ -n "${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${CHAINWORKS_SHARED_CARGO_TARGET_DIR}"
elif [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="$(chainworks_default_cargo_target_dir)"
fi

mkdir -p "${CARGO_TARGET_DIR}"

case "${CHAINWORKS_CARGO_SCCACHE:-auto}" in
    0|false|FALSE|off|OFF|no|NO)
        ;;
    *)
        if [[ -z "${RUSTC_WRAPPER:-}" ]] && command -v sccache >/dev/null 2>&1; then
            export RUSTC_WRAPPER="$(command -v sccache)"
        fi
        ;;
esac
