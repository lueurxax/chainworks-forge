#!/usr/bin/env bash
# P042 §7.2 / §7.5 embed-daemon build phase.
#
# Invoked by the "Embed Control-Plane Daemon" Xcode Run Script phase on
# the `Chainworks Forge` target. Runs after the Swift compilation step
# and before the code-sign step, so the embedded binary participates in
# the same Developer-ID + notarization pass as the app itself.
#
# Behavior:
#   1. Build `${CARGO_TARGET_DIR}/${profile}/control-plane` via cargo.
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

# Xcode Cloud does not provide Rust/Cargo in the Xcode build phase. Its
# repository custom scripts run before xcodebuild and can leave a compiled
# daemon in a stable workspace path. Only Xcode Cloud may auto-discover that
# cache; local developer/proposal builds must compile the current checkout so
# a stale `.xcode-cloud` artifact cannot silently change the GraphQL schema.
PREBUILT_CANDIDATES=()
append_prebuilt_candidate() {
    local candidate="$1"
    local existing
    if [ "${#PREBUILT_CANDIDATES[@]}" -gt 0 ]; then
        for existing in "${PREBUILT_CANDIDATES[@]}"; do
            if [ "${existing}" = "${candidate}" ]; then
                return
            fi
        done
    fi
    PREBUILT_CANDIDATES+=("${candidate}")
}

if [ -n "${CHAINWORKS_PREBUILT_CONTROL_PLANE_DAEMON:-}" ]; then
    append_prebuilt_candidate "${CHAINWORKS_PREBUILT_CONTROL_PLANE_DAEMON}"
fi
if [ -n "${CI_XCODE_CLOUD:-}" ]; then
    append_prebuilt_candidate "${SRCROOT}/.xcode-cloud/control-plane/${PROFILE_DIR}/control-plane"
    append_prebuilt_candidate "${SRCROOT}/.xcode-cloud/control-plane/release/control-plane"
    append_prebuilt_candidate "${SRCROOT}/.xcode-cloud/control-plane/control-plane"
fi

SOURCE_BIN=""
if [ "${#PREBUILT_CANDIDATES[@]}" -gt 0 ]; then
    for candidate in "${PREBUILT_CANDIDATES[@]}"; do
        if [ -x "${candidate}" ]; then
            SOURCE_BIN="${candidate}"
            break
        fi
    done
fi

# Xcode build phases do NOT inherit the user's shell profile, so Cargo
# and optional wrappers installed by rustup/homebrew may be absent from
# PATH. Add standard locations before applying the cache helper so it can
# discover `sccache` when present.
for candidate in "$HOME/.cargo/bin" "/opt/homebrew/bin" "/usr/local/bin"; do
    if [ -d "$candidate" ]; then
        PATH="$candidate:$PATH"
    fi
done
export PATH

# Keep Xcode-triggered Rust builds in a stable cache instead of
# DerivedData/TARGET_TEMP_DIR so Swift test runs do not rebuild Rust
# dependencies from scratch. The helper also enables sccache when it is
# installed. CHAINWORKS_XCODE_CARGO_TARGET_DIR remains the explicit
# override for CI/proposal gates that want a different pre-warmed target.
# shellcheck source=scripts/cargo-cache-env.sh
source "${SRCROOT}/scripts/cargo-cache-env.sh"

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

if [ -n "${SOURCE_BIN}" ]; then
    echo "embed-control-plane-daemon: using prebuilt daemon ${SOURCE_BIN}"
else
    if ! command -v cargo >/dev/null 2>&1; then
        echo "error: cargo not found on PATH ($PATH)" >&2
        echo "       no prebuilt daemon was found in:" >&2
        if [ "${#PREBUILT_CANDIDATES[@]}" -gt 0 ]; then
            for candidate in "${PREBUILT_CANDIDATES[@]}"; do
                echo "         - ${candidate}" >&2
            done
        else
            echo "         - none configured for this local build" >&2
        fi
        echo "       Xcode Cloud should run ci_scripts/ci_post_clone.sh before xcodebuild" >&2
        echo "       local builds need Rust installed via https://rustup.rs" >&2
        exit 127
    fi

    (
        cd "${CONTROL_PLANE_DIR}"
        # shellcheck disable=SC2086
        cargo build ${CARGO_PROFILE_FLAG} --bin control-plane
    )

    SOURCE_BIN="${CARGO_TARGET_DIR}/${PROFILE_DIR}/control-plane"
fi
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

# App-side lifecycle/read surfaces compare this stamp with the live
# daemon's `/ready` build_sha so operators see an explicit update-required
# state even when the GraphQL schema version did not change.
BUNDLED_SHA_RESOURCE_DIR="${TARGET_BUILD_DIR}/${UNLOCALIZED_RESOURCES_FOLDER_PATH:-${CONTENTS_FOLDER_PATH:-Contents}/Resources}"
mkdir -p "${BUNDLED_SHA_RESOURCE_DIR}"
printf '%s\n' "${GIT_SHA}" > "${BUNDLED_SHA_RESOURCE_DIR}/bundled-daemon-build-sha.txt"

# The packaged daemon resolves its agent catalog from the app bundle. Keep the
# catalog beside the build stamp so debug and archive products have the same
# launch-time inputs as the source-tree daemon.
BUNDLED_AGENT_CATALOG="${SRCROOT}/examples/agents/agents.yaml"
if [ ! -f "${BUNDLED_AGENT_CATALOG}" ]; then
    echo "error: packaged daemon agent catalog missing at ${BUNDLED_AGENT_CATALOG}" >&2
    exit 65
fi
cp "${BUNDLED_AGENT_CATALOG}" "${BUNDLED_SHA_RESOURCE_DIR}/agents.yaml"

# `SMAppService.agent` starts from the embedded LaunchAgent plist, not
# from the current shell. Stamp the same SHA into that plist so a
# daemon launched from the app reports the bundled build identity even
# when the Rust binary was supplied through the local prebuilt path.
BUNDLED_LAUNCH_AGENT_PLIST="${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH:-Contents}/Library/LaunchAgents/com.chainworks.forge.daemon.plist"
if [ -f "${BUNDLED_LAUNCH_AGENT_PLIST}" ]; then
    /usr/libexec/PlistBuddy -c 'Print :EnvironmentVariables' "${BUNDLED_LAUNCH_AGENT_PLIST}" >/dev/null 2>&1 \
        || /usr/libexec/PlistBuddy -c 'Add :EnvironmentVariables dict' "${BUNDLED_LAUNCH_AGENT_PLIST}"
    if /usr/libexec/PlistBuddy -c 'Print :EnvironmentVariables:GIT_SHA' "${BUNDLED_LAUNCH_AGENT_PLIST}" >/dev/null 2>&1; then
        /usr/libexec/PlistBuddy -c "Set :EnvironmentVariables:GIT_SHA ${GIT_SHA}" "${BUNDLED_LAUNCH_AGENT_PLIST}"
    else
        /usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:GIT_SHA string ${GIT_SHA}" "${BUNDLED_LAUNCH_AGENT_PLIST}"
    fi
else
    echo "warning: embedded LaunchAgent plist not found at ${BUNDLED_LAUNCH_AGENT_PLIST}; GIT_SHA will not be stamped into launchd environment" >&2
fi

echo "embed-control-plane-daemon: ok → ${DEST_BIN}"
