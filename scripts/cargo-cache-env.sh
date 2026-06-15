#!/usr/bin/env bash
# Shared Cargo cache policy for Chainworks Forge build scripts.
#
# Source this file from scripts that invoke Cargo from Xcode, test gates,
# or agent worktrees. It keeps Rust build products in one stable bounded
# cache instead of multiplying `control-plane/target` per worktree, and
# enables sccache when it is installed without making it a hard dependency.

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

chainworks_gate_cargo_target_root() {
    if [[ -n "${CHAINWORKS_GATE_CARGO_TARGET_ROOT:-}" ]]; then
        printf '%s\n' "${CHAINWORKS_GATE_CARGO_TARGET_ROOT}"
    elif [[ -n "${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-}" ]]; then
        printf '%s\n' "${CHAINWORKS_SHARED_CARGO_TARGET_DIR}/gates"
    else
        printf '%s\n' "$(chainworks_default_cargo_target_dir)/gates"
    fi
}

chainworks_gate_cargo_target_dir() {
    local requested="${1:-}"
    if [[ -z "${requested}" ]]; then
        chainworks_default_cargo_target_dir
        return 0
    fi
    if [[ "${CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR:-0}" =~ ^(1|true|TRUE|yes|YES)$ ]]; then
        printf '%s\n' "${requested}"
        return 0
    fi

    local suffix=""
    case "${requested}" in
        target)
            suffix="default"
            ;;
        target/*)
            suffix="${requested#target/}"
            ;;
        control-plane/target)
            suffix="default"
            ;;
        control-plane/target/*)
            suffix="${requested#control-plane/target/}"
            ;;
        */control-plane/target)
            suffix="default"
            ;;
        */control-plane/target/*)
            suffix="${requested#*/control-plane/target/}"
            ;;
        *)
            printf '%s\n' "${requested}"
            return 0
            ;;
    esac

    if [[ -z "${suffix}" || "${suffix}" == "." ]]; then
        suffix="default"
    fi

    local safe_suffix
    safe_suffix="$(printf '%s' "${suffix}" | sed -E 's#[/[:space:]]+#-#g; s#[^A-Za-z0-9._-]#_#g; s#^[-.]+##; s#[-.]+$##')"
    if [[ -z "${safe_suffix}" ]]; then
        safe_suffix="default"
    fi
    printf '%s/%s\n' "$(chainworks_gate_cargo_target_root)" "${safe_suffix}"
}

chainworks_find_sccache() {
    if [[ -n "${CHAINWORKS_SCCACHE_BINARY:-}" && -x "${CHAINWORKS_SCCACHE_BINARY}" ]]; then
        printf '%s\n' "${CHAINWORKS_SCCACHE_BINARY}"
        return 0
    fi
    if [[ -n "${RUSTC_WRAPPER:-}" && -x "${RUSTC_WRAPPER}" ]]; then
        printf '%s\n' "${RUSTC_WRAPPER}"
        return 0
    fi
    if command -v sccache >/dev/null 2>&1; then
        command -v sccache
        return 0
    fi
    local candidate
    for candidate in \
        "/opt/homebrew/bin/sccache" \
        "/usr/local/bin/sccache" \
        "${HOME:-}/.cargo/bin/sccache"; do
        if [[ -n "${candidate}" && -x "${candidate}" ]]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    done
    return 1
}

# Worktrees used to default to a local `control-plane/target`, which made
# every active run allocate tens of GiB. The default is now shared. A local
# target remains available as an explicit escape hatch for one-off diagnosis.
_chainworks_effective_root="${SRCROOT:-${chainworks_cargo_cache_repo_root}}"
_chainworks_is_worktree=0
if [[ "${_chainworks_effective_root}" == *"/.chainworks/worktrees/"* ]]; then
    _chainworks_is_worktree=1
fi

if [[ -n "${CHAINWORKS_XCODE_CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${CHAINWORKS_XCODE_CARGO_TARGET_DIR}"
elif [[ -n "${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${CHAINWORKS_SHARED_CARGO_TARGET_DIR}"
elif [[ "${_chainworks_is_worktree}" == "1" && "${CHAINWORKS_WORKTREE_LOCAL_CARGO_TARGET:-0}" =~ ^(1|true|TRUE|yes|YES)$ ]]; then
    # Escape hatch only. This is intentionally not the default because it
    # recreates the per-worktree disk blow-up this policy is meant to prevent.
    export CARGO_TARGET_DIR="${_chainworks_effective_root}/control-plane/target/xcode-local"
elif [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="$(chainworks_default_cargo_target_dir)"
fi

mkdir -p "${CARGO_TARGET_DIR}"

case "${CHAINWORKS_CARGO_SCCACHE:-auto}" in
    0|false|FALSE|off|OFF|no|NO)
        ;;
    *)
        if [[ -z "${RUSTC_WRAPPER:-}" ]] && sccache_binary="$(chainworks_find_sccache)"; then
            export SCCACHE_DIR="${SCCACHE_DIR:-$(chainworks_default_sccache_dir)}"
            export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-20G}"
            mkdir -p "${SCCACHE_DIR}"
            export RUSTC_WRAPPER="${sccache_binary}"
        fi
        ;;
esac
