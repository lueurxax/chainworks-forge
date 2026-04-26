#!/usr/bin/env bash
# Xcode Cloud bootstrap for the Rust control-plane daemon.
#
# The Xcode target's "Embed Control-Plane Daemon" build phase can consume a
# prebuilt daemon from `.xcode-cloud/control-plane/...`. Building it here keeps
# the Xcode build phase deterministic and avoids depending on Cargo being
# available inside SwiftBuild's restricted PATH.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTROL_PLANE_DIR="${ROOT_DIR}/control-plane"
PREBUILT_ROOT="${ROOT_DIR}/.xcode-cloud/control-plane"

if [ ! -d "${CONTROL_PLANE_DIR}" ]; then
    echo "ci_post_clone: control-plane directory not found at ${CONTROL_PLANE_DIR}" >&2
    exit 65
fi

PATH="${HOME}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"
export PATH

if ! command -v cargo >/dev/null 2>&1; then
    echo "ci_post_clone: installing minimal Rust toolchain for Xcode Cloud"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
    PATH="${HOME}/.cargo/bin:${PATH}"
    export PATH
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "ci_post_clone: cargo is still unavailable after Rust bootstrap" >&2
    exit 127
fi

PROFILE="${CHAINWORKS_XCODE_CLOUD_DAEMON_PROFILE:-release}"
case "${PROFILE}" in
    debug)
        CARGO_PROFILE_FLAG=""
        PROFILE_DIR="debug"
        ;;
    release)
        CARGO_PROFILE_FLAG="--release"
        PROFILE_DIR="release"
        ;;
    *)
        echo "ci_post_clone: unsupported CHAINWORKS_XCODE_CLOUD_DAEMON_PROFILE=${PROFILE}" >&2
        echo "               expected 'release' or 'debug'" >&2
        exit 64
        ;;
esac

if command -v git >/dev/null 2>&1; then
    GIT_SHA="$(cd "${ROOT_DIR}" && git rev-parse --short HEAD 2>/dev/null || true)"
fi
if [ -z "${GIT_SHA:-}" ]; then
    GIT_SHA="dev"
fi
export GIT_SHA

export CARGO_TARGET_DIR="${CHAINWORKS_XCODE_CLOUD_CARGO_TARGET_DIR:-${ROOT_DIR}/.xcode-cloud/cargo-target}"
mkdir -p "${CARGO_TARGET_DIR}" "${PREBUILT_ROOT}/${PROFILE_DIR}"

echo "ci_post_clone: building control-plane daemon (${PROFILE_DIR})"
echo "               source=${CONTROL_PLANE_DIR}"
echo "               target=${CARGO_TARGET_DIR}"
echo "               GIT_SHA=${GIT_SHA}"

(
    cd "${CONTROL_PLANE_DIR}"
    # shellcheck disable=SC2086
    cargo build ${CARGO_PROFILE_FLAG} --bin control-plane
)

SOURCE_BIN="${CARGO_TARGET_DIR}/${PROFILE_DIR}/control-plane"
DEST_BIN="${PREBUILT_ROOT}/${PROFILE_DIR}/control-plane"

if [ ! -x "${SOURCE_BIN}" ]; then
    echo "ci_post_clone: expected daemon binary not found at ${SOURCE_BIN}" >&2
    exit 65
fi

cp "${SOURCE_BIN}" "${DEST_BIN}"
chmod +x "${DEST_BIN}"

echo "ci_post_clone: prebuilt daemon ready at ${DEST_BIN}"
