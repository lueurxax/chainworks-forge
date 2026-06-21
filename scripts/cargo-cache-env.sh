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

chainworks_default_cargo_wrapper_dir() {
    if [[ -n "${HOME:-}" ]]; then
        printf '%s\n' "${HOME}/Library/Caches/Chainworks Forge/bin"
    else
        printf '%s\n' "${chainworks_cargo_cache_repo_root}/control-plane/target/bin"
    fi
}

chainworks_path_has_dir() {
    local needle="$1"
    local part
    IFS=':' read -r -a _chainworks_path_parts <<< "${PATH:-}"
    for part in "${_chainworks_path_parts[@]:-}"; do
        if [[ "$part" == "$needle" ]]; then
            unset _chainworks_path_parts
            return 0
        fi
    done
    unset _chainworks_path_parts
    return 1
}

chainworks_find_real_cargo() {
    local explicit="${CHAINWORKS_REAL_CARGO:-}"
    local wrapper_dir="${CHAINWORKS_CARGO_WRAPPER_DIR:-$(chainworks_default_cargo_wrapper_dir)}"
    local wrapper_path="${wrapper_dir%/}/cargo"
    if [[ -n "$explicit" && -x "$explicit" && "$explicit" != "$wrapper_path" ]]; then
        printf '%s\n' "$explicit"
        return 0
    fi

    local part candidate
    IFS=':' read -r -a _chainworks_path_parts <<< "${PATH:-}"
    for part in "${_chainworks_path_parts[@]:-}"; do
        [[ -n "$part" ]] || continue
        if [[ "$part" == "$wrapper_dir" ]]; then
            continue
        fi
        candidate="${part%/}/cargo"
        if [[ -x "$candidate" ]]; then
            printf '%s\n' "$candidate"
            unset _chainworks_path_parts
            return 0
        fi
    done
    unset _chainworks_path_parts

    command -v cargo 2>/dev/null || true
}

chainworks_effective_sccache_dir() {
    local candidate="${SCCACHE_DIR:-$(chainworks_default_sccache_dir)}"
    if [[ -n "${SCCACHE_DIR:-}" ]]; then
        printf '%s\n' "${candidate}"
        return 0
    fi

    # sccache uses a Unix socket under SCCACHE_DIR. Provider runtimes can set a
    # long isolated HOME path, and macOS rejects socket paths near SUN_LEN.
    local socket_path="${candidate%/}/sccache.sock"
    if (( ${#socket_path} < 100 )); then
        printf '%s\n' "${candidate}"
        return 0
    fi

    local uid
    uid="$(id -u 2>/dev/null || printf 'unknown')"
    printf '/tmp/chainworks-sccache-%s\n' "${uid}"
}

chainworks_sccache_compiler_path() {
    local sysroot
    sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    if [[ -n "${sysroot}" && -x "${sysroot}/bin/rustc" ]]; then
        printf '%s\n' "${sysroot}/bin/rustc"
        return 0
    fi
    command -v rustc 2>/dev/null || true
}

chainworks_sccache_can_wrap_rustc() {
    local compiler_path
    compiler_path="$(chainworks_sccache_compiler_path)"
    if [[ -z "${compiler_path}" ]]; then
        return 1
    fi

    # sccache on macOS also rejects very long compiler paths before Cargo can
    # run tests. Provider-local rustup toolchains live under long temp roots, so
    # auto mode must fail open to plain rustc instead of failing the gate.
    (( ${#compiler_path} < 100 ))
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

chainworks_install_cargo_wrapper() {
    if [[ "${CHAINWORKS_CARGO_WRAPPER:-1}" =~ ^(0|false|FALSE|off|OFF|no|NO)$ ]]; then
        return 0
    fi

    local real_cargo
    real_cargo="$(chainworks_find_real_cargo)"
    if [[ -z "$real_cargo" || ! -x "$real_cargo" ]]; then
        return 0
    fi

    export CHAINWORKS_REAL_CARGO="$real_cargo"
    export CHAINWORKS_CARGO_WRAPPER_DIR="${CHAINWORKS_CARGO_WRAPPER_DIR:-$(chainworks_default_cargo_wrapper_dir)}"
    mkdir -p "$CHAINWORKS_CARGO_WRAPPER_DIR" 2>/dev/null || return 0

    local wrapper_path="${CHAINWORKS_CARGO_WRAPPER_DIR%/}/cargo"
    cat > "$wrapper_path" <<'CHAINWORKS_CARGO_WRAPPER'
#!/usr/bin/env bash
set -euo pipefail

real_cargo="${CHAINWORKS_REAL_CARGO:-}"
if [[ -z "$real_cargo" || ! -x "$real_cargo" ]]; then
  echo "chainworks cargo wrapper: CHAINWORKS_REAL_CARGO is not executable" >&2
  exit 127
fi

if [[ ! "${CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR:-0}" =~ ^(1|true|TRUE|yes|YES)$ ]]; then
  requested="${CARGO_TARGET_DIR:-}"
  suffix=""
  case "$requested" in
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
  esac

  if [[ -n "$suffix" ]]; then
    if [[ -z "$suffix" || "$suffix" == "." ]]; then
      suffix="default"
    fi
    safe_suffix="$(printf '%s' "$suffix" | sed -E 's#[/[:space:]]+#-#g; s#[^A-Za-z0-9._-]#_#g; s#^[-.]+##; s#[-.]+$##')"
    if [[ -z "$safe_suffix" ]]; then
      safe_suffix="default"
    fi
    gate_root="${CHAINWORKS_GATE_CARGO_TARGET_ROOT:-${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-}/gates}"
    if [[ "$gate_root" == "/gates" || -z "$gate_root" ]]; then
      gate_root="${HOME:-/tmp}/Library/Caches/Chainworks Forge/cargo-target/gates"
    fi
    export CARGO_TARGET_DIR="${gate_root}/${safe_suffix}"
    mkdir -p "$CARGO_TARGET_DIR"
  fi
fi

exec "$real_cargo" "$@"
CHAINWORKS_CARGO_WRAPPER
    chmod +x "$wrapper_path" 2>/dev/null || return 0

    if ! chainworks_path_has_dir "$CHAINWORKS_CARGO_WRAPPER_DIR"; then
        export PATH="${CHAINWORKS_CARGO_WRAPPER_DIR}:${PATH:-}"
    fi
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

export CHAINWORKS_SHARED_CARGO_TARGET_DIR="${CHAINWORKS_SHARED_CARGO_TARGET_DIR:-$(chainworks_default_cargo_target_dir)}"
export CHAINWORKS_GATE_CARGO_TARGET_ROOT="${CHAINWORKS_GATE_CARGO_TARGET_ROOT:-${CHAINWORKS_SHARED_CARGO_TARGET_DIR}/gates}"
mkdir -p "${CARGO_TARGET_DIR}"

_chainworks_sccache_mode="${CHAINWORKS_CARGO_SCCACHE:-auto}"
case "${_chainworks_sccache_mode}" in
    0|false|FALSE|off|OFF|no|NO)
        ;;
    *)
        if [[ -z "${RUSTC_WRAPPER:-}" ]] && command -v sccache >/dev/null 2>&1; then
            if [[ ! "${_chainworks_sccache_mode}" =~ ^(auto|AUTO)?$ ]] || chainworks_sccache_can_wrap_rustc; then
                export SCCACHE_DIR="$(chainworks_effective_sccache_dir)"
                export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-20G}"
                mkdir -p "${SCCACHE_DIR}"
                export RUSTC_WRAPPER="$(command -v sccache)"
            fi
        fi
        ;;
esac
unset _chainworks_sccache_mode

chainworks_install_cargo_wrapper
