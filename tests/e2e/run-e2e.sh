#!/bin/bash
# E2E smoke runner for Cull.
#
# Starts Vite in browser-test mode unless a server is already listening on the
# chosen port, then runs the Playwright smoke suite.

set -euo pipefail

PORT="${CULL_E2E_PORT:-1420}"
URL="${CULL_E2E_URL:-http://127.0.0.1:${PORT}}"
SHOTS="${CULL_E2E_SHOTS:-/tmp/cull-e2e}"
LOG="${CULL_E2E_LOG:-/tmp/cull-e2e-vite.log}"
SERVER_PID=""

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

wait_for_server() {
    local attempts=0
    until curl -fsS "$URL" >/dev/null 2>&1; do
        attempts=$((attempts + 1))
        if [ "$attempts" -gt 80 ]; then
            echo "Vite did not become ready at $URL"
            echo "Log: $LOG"
            exit 1
        fi
        sleep 0.25
    done
}

mkdir -p "$SHOTS"

if curl -fsS "$URL" >/dev/null 2>&1; then
    echo "[e2e] Reusing server at $URL"
else
    echo "[e2e] Starting Vite at $URL"
    CULL_E2E_MOCK=1 npx vite dev --host 127.0.0.1 --port "$PORT" >"$LOG" 2>&1 &
    SERVER_PID="$!"
    wait_for_server
fi

CULL_E2E_URL="$URL" CULL_E2E_SHOTS="$SHOTS" python3 tests/e2e/smoke.py
