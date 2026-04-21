#!/usr/bin/env bash
# P042 §7.2 / §7.5 embed-daemon build phase.
#
# Invoked by the "Embed Control-Plane Daemon" Xcode Run Script phase on
# the `Chainworks Forge` target. Runs after the Swift compilation step
# and before the code-sign step, so the embedded binary participates in
# the same Developer-ID + notarization pass as the app itself.
#
# Behavior:
#   1. Build `control-plane/target/${profile}/control-plane` via cargo.
#      Profile follows Xcode's CONFIGURATION: Debug → dev profile,
#      everything else → release.
#   2. Copy the resulting binary to
#      `${TARGET_BUILD_DIR}/${EXECUTABLE_FOLDER_PATH}/chainworks-forge-daemon`.
#   3. Stamp chmod +x so `posix_spawn` / `SMAppService` can invoke it.
#
# The "Embed LaunchAgent Plist" Copy Files phase runs in parallel and
# copies `Chainworks Forge/Resources/LaunchAgents/com.chainworks.forge.daemon.plist`
# into `Contents/Library/LaunchAgents/`. Both phases together make
# `SMAppService.agent(plistName:).register()` succeed at runtime.
#
# NOTE on LaunchAgents vs LaunchDaemons: `SMAppService.agent(...)`
# always reads from `Contents/Library/LaunchAgents/` (per-user, session
# scope). LaunchDaemons is system-wide and requires privileged install
# via `SMAppService.daemon(...)` + admin auth — not what P042 ships.
# Earlier drafts of this comment said LaunchDaemons and caused
# operator/reviewer drift; R11 REQ-019 flags that as documentation
# drift and this header is the canonical fix.

set -euo pipefail

# Xcode build phases do NOT inherit the user's shell profile, so `cargo`
# (installed by rustup into `~/.cargo/bin/`) is not on PATH by default.
# Add the standard rustup paths if they exist before anything else.
for candidate in "$HOME/.cargo/bin" "/opt/homebrew/bin" "/usr/local/bin"; do
    if [ -d "$candidate" ]; then
        PATH="$candidate:$PATH"
    fi
done
export PATH

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found on PATH ($PATH)" >&2
    echo "       install Rust via https://rustup.rs and rerun the build" >&2
    exit 127
fi

if [ -z "${TARGET_BUILD_DIR:-}" ] || [ -z "${EXECUTABLE_FOLDER_PATH:-}" ]; then
    echo "error: this script must run inside an Xcode build phase" >&2
    exit 64
fi

if [ -z "${SRCROOT:-}" ]; then
    echo "error: SRCROOT is unset; refusing to guess repo root" >&2
    exit 64
fi

PROFILE="${CONFIGURATION:-Debug}"
case "${PROFILE}" in
    Debug) CARGO_PROFILE_FLAG=""; PROFILE_DIR="debug" ;;
    *)     CARGO_PROFILE_FLAG="--release"; PROFILE_DIR="release" ;;
esac

# SRCROOT in Xcode is the directory containing `.xcodeproj`, i.e. the
# repo root. `control-plane/` is a sibling of `Chainworks Forge/`, so
# it sits directly under SRCROOT — no `..` indirection.
CONTROL_PLANE_DIR="${SRCROOT}/control-plane"
if [ ! -d "${CONTROL_PLANE_DIR}" ]; then
    echo "error: control-plane directory not found at ${CONTROL_PLANE_DIR}" >&2
    exit 65
fi

# Cargo locks its `target/` directory, so a concurrent `cargo test`
# from the terminal or test-gate.sh blocks this build phase. Redirect
# Xcode's cargo output into DerivedData so the two never contend.
# Trade-off: a Clean Build re-downloads/compiles from scratch (cache
# lives outside the repo). That's the right choice for an Xcode-owned
# build step — terminal cargo stays fast with its own cache.
#
# Proposal gates that intentionally compose an Xcode build with Rust
# control-plane checks may opt into a pre-warmed target directory to
# avoid recompiling the daemon from scratch inside a prerequisite gate.
export CARGO_TARGET_DIR="${CHAINWORKS_XCODE_CARGO_TARGET_DIR:-${TARGET_TEMP_DIR}/cargo-target}"
mkdir -p "${CARGO_TARGET_DIR}"

# R12 OPS-001 / REQ-008: the daemon reads its build SHA via
# `option_env!("GIT_SHA")`, which is resolved at *Rust compile time*.
# Earlier versions of this script computed `GIT_SHA` AFTER `cargo
# build`, so the compiled daemon always got `None` → `build-sha.txt`
# reported `dev` in packaged releases. We now export `GIT_SHA` before
# invoking cargo so the literal makes it into the binary.
if command -v git >/dev/null 2>&1; then
    GIT_SHA="$(cd "${CONTROL_PLANE_DIR}" && git rev-parse --short HEAD 2>/dev/null || true)"
fi
if [ -z "${GIT_SHA:-}" ]; then
    GIT_SHA="dev"
fi
export GIT_SHA

echo "embed-control-plane-daemon: profile=${PROFILE}"
echo "                           source=${CONTROL_PLANE_DIR}"
echo "                           target=${CARGO_TARGET_DIR}"
echo "                           GIT_SHA=${GIT_SHA}"

(
    cd "${CONTROL_PLANE_DIR}"
    # shellcheck disable=SC2086
    cargo build ${CARGO_PROFILE_FLAG} --bin control-plane
)

SOURCE_BIN="${CARGO_TARGET_DIR}/${PROFILE_DIR}/control-plane"
if [ ! -x "${SOURCE_BIN}" ]; then
    echo "error: expected daemon binary not found at ${SOURCE_BIN}" >&2
    exit 65
fi

DEST_DIR="${TARGET_BUILD_DIR}/${EXECUTABLE_FOLDER_PATH}"
DEST_BIN="${DEST_DIR}/chainworks-forge-daemon"
mkdir -p "${DEST_DIR}"
cp "${SOURCE_BIN}" "${DEST_BIN}"
chmod +x "${DEST_BIN}"

# P042 §7.5 + LWCR: SMAppService's Launchd Constraint Rule requires
# that the supervising app and the launched agent share a team ID.
# Xcode's final `codesign` pass only signs the app's main executable
# and Frameworks/ — it does NOT recurse into `Contents/MacOS/`
# auxiliary binaries. A rust-produced binary arrives ad-hoc
# (linker-signed); launchd rejects that combination with EX_CONFIG
# and spawns fail silently.
#
# Re-sign the embedded daemon with the same identity Xcode is about
# to use on the app. `EXPANDED_CODE_SIGN_IDENTITY` is the keychain
# SHA1 of the chosen identity; `--force` replaces the ad-hoc
# signature in place.
if [ -n "${EXPANDED_CODE_SIGN_IDENTITY:-}" ] \
       && [ "${EXPANDED_CODE_SIGN_IDENTITY}" != "-" ]; then
    CODESIGN_ARGS=(
        --force
        --sign "${EXPANDED_CODE_SIGN_IDENTITY}"
        --timestamp=none
        --generate-entitlement-der
    )
    if [ "${ENABLE_HARDENED_RUNTIME:-NO}" = "YES" ]; then
        CODESIGN_ARGS+=(--options=runtime)
    fi
    echo "embed-control-plane-daemon: signing with ${EXPANDED_CODE_SIGN_IDENTITY_NAME:-identity}"
    /usr/bin/codesign "${CODESIGN_ARGS[@]}" "${DEST_BIN}"
else
    echo "embed-control-plane-daemon: no code-sign identity (ad-hoc signature kept)"
fi

# GIT_SHA was exported above, before `cargo build`, so the Rust
# `option_env!("GIT_SHA")` call in `daemon::packaging::write_build_sha`
# now has a compile-time literal. The post-build echo below stays
# informational only — the value in the binary is the exported one.
echo "embed-control-plane-daemon: bundled daemon sha ${GIT_SHA}"

echo "embed-control-plane-daemon: ok → ${DEST_BIN}"
