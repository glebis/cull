#!/bin/bash
# E2E smoke runner for Cull.
#
# Starts Vite in browser-test mode unless a server is already listening on the
# chosen port, then runs the Playwright smoke suite.

set -euo pipefail

PORT="${CULL_E2E_PORT:-1420}"
EXPLICIT_URL="${CULL_E2E_URL:-}"
REUSE_SERVER="${CULL_E2E_REUSE_SERVER:-0}"
URL="${EXPLICIT_URL:-http://127.0.0.1:${PORT}}"
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

is_port_open() {
    local port="$1"
    (echo >"/dev/tcp/127.0.0.1/${port}") >/dev/null 2>&1
}

select_port() {
    local port="$1"
    local limit=$((port + 50))
    while is_port_open "$port"; do
        if [ "$REUSE_SERVER" = "1" ]; then
            echo "$port"
            return
        fi
        echo "[e2e] Port $port is already in use; trying $((port + 1))" >&2
        port=$((port + 1))
        if [ "$port" -gt "$limit" ]; then
            echo "No free E2E port found between $1 and $limit" >&2
            exit 1
        fi
    done
    echo "$port"
}

mkdir -p "$SHOTS"

if [ -z "$EXPLICIT_URL" ]; then
    PORT="$(select_port "$PORT")"
    URL="http://127.0.0.1:${PORT}"
fi

if curl -fsS "$URL" >/dev/null 2>&1; then
    echo "[e2e] Reusing server at $URL"
else
    echo "[e2e] Starting Vite at $URL"
    CULL_E2E_MOCK=1 npx vite dev --host 127.0.0.1 --port "$PORT" >"$LOG" 2>&1 &
    SERVER_PID="$!"
    wait_for_server
fi

CULL_E2E_URL="$URL" CULL_E2E_SHOTS="$SHOTS" python3 tests/e2e/smoke.py
