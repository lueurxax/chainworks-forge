#!/usr/bin/env bash

set -euo pipefail

MODE="${1:-run}"
APP_NAME="Chainworks Forge"
BUNDLE_ID="xax.Chainworks-Forge"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE_OUTPUT_ROOT="${TMPDIR:-/tmp}/chainworks-test-gates"

case "$MODE" in
    run|--debug|debug|--logs|logs|--telemetry|telemetry|--verify|verify) ;;
    *)
        printf 'usage: %s [run|--debug|--logs|--telemetry|--verify]\n' "$0" >&2
        exit 2
        ;;
esac

/usr/bin/pkill -x "$APP_NAME" >/dev/null 2>&1 || true

"$ROOT_DIR/scripts/test-gate.sh" build

APP_BUNDLE="$(
    find "$GATE_OUTPUT_ROOT" \
        -path "*/Build/Products/Debug/$APP_NAME.app" \
        -type d \
        -print0 2>/dev/null |
        xargs -0 stat -f '%m %N' 2>/dev/null |
        sort -nr |
        head -n 1 |
        cut -d' ' -f2-
)"
if [[ -z "$APP_BUNDLE" || ! -d "$APP_BUNDLE" ]]; then
    printf 'error: built %s.app was not found under %s\n' "$APP_NAME" "$GATE_OUTPUT_ROOT" >&2
    exit 1
fi

APP_BINARY="$APP_BUNDLE/Contents/MacOS/$APP_NAME"
if [[ ! -x "$APP_BINARY" ]]; then
    printf 'error: built app binary is not executable: %s\n' "$APP_BINARY" >&2
    exit 1
fi

open_app() {
    /usr/bin/open -n "$APP_BUNDLE"
}

case "$MODE" in
    run)
        open_app
        ;;
    --debug|debug)
        /usr/bin/lldb -- "$APP_BINARY"
        ;;
    --logs|logs)
        open_app
        /usr/bin/log stream --info --style compact --predicate "process == \"$APP_NAME\""
        ;;
    --telemetry|telemetry)
        open_app
        /usr/bin/log stream --info --style compact --predicate "subsystem == \"$BUNDLE_ID\""
        ;;
    --verify|verify)
        open_app
        for _ in {1..20}; do
            if /usr/bin/pgrep -x "$APP_NAME" >/dev/null; then
                printf '%s launched from %s\n' "$APP_NAME" "$APP_BUNDLE"
                exit 0
            fi
            sleep 0.25
        done
        printf 'error: %s did not remain running after launch\n' "$APP_NAME" >&2
        exit 1
        ;;
esac
