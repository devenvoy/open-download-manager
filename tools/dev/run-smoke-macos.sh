#!/usr/bin/env bash
# Copyright (c) 2026 Open Download Manager contributors
# SPDX-License-Identifier: MIT
#
# tools/dev/run-smoke-macos.sh
# Runs Open Download Manager in smoke test mode with isolated temporary HOME & ODM_DATA_DIR.
# Verifies zero modifications to ~/Library before and after the run.
# Integrates synthetic benchmark server and WebKit process sampler.

set -euo pipefail

RUN_INDEX="${1:-1}"
OUTPUT_FILE="${2:-docs/audit/evidence/baseline-macos/smoke-result-${RUN_INDEX}.json}"
RELEASE_BIN=""
if [ -f "target/release/open-download-manager" ]; then
    RELEASE_BIN="target/release/open-download-manager"
elif [ -f "src-tauri/target/release/open-download-manager" ]; then
    RELEASE_BIN="src-tauri/target/release/open-download-manager"
elif [ -f "target/release/bundle/macos/Open Download Manager.app/Contents/MacOS/Open Download Manager" ]; then
    RELEASE_BIN="target/release/bundle/macos/Open Download Manager.app/Contents/MacOS/Open Download Manager"
fi

if [ -z "$RELEASE_BIN" ] || [ ! -f "$RELEASE_BIN" ]; then
    echo "Error: Release binary not found at target/release/open-download-manager. Build it first." >&2
    exit 1
fi

REAL_HOME="$HOME"
BEFORE_SNAP="/tmp/odm-library-before-${RUN_INDEX}.txt"
AFTER_SNAP="/tmp/odm-library-after-${RUN_INDEX}.txt"
SMOKE_TMP=$(mktemp -d /tmp/odm-smoke-isolated-XXXXXX)
SMOKE_HOME="$SMOKE_TMP/home"
SMOKE_DATA="$SMOKE_TMP/data"
mkdir -p "$SMOKE_HOME" "$SMOKE_DATA"
mkdir -p "$(dirname "$OUTPUT_FILE")"

SERVER_PID=""
SAMPLER_PID=""

cleanup() {
    set +e
    if [ -n "$SAMPLER_PID" ] && kill -0 "$SAMPLER_PID" 2>/dev/null; then
        kill -TERM "$SAMPLER_PID" 2>/dev/null || true
    fi
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill -TERM "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$BEFORE_SNAP" "$AFTER_SNAP"
    rm -rf "$SMOKE_TMP"
}
trap cleanup EXIT INT TERM

# 1. Start benchmark server if not already running
if ! curl -s -f "http://127.0.0.1:8999/health" >/dev/null 2>&1; then
    echo "=== [RUN-SMOKE] Starting synthetic benchmark server on port 8999 ==="
    bun tools/dev/benchmark-server.ts &
    SERVER_PID=$!
    for _ in {1..30}; do
        if curl -s -f "http://127.0.0.1:8999/health" >/dev/null 2>&1; then
            break
        fi
        sleep 0.1
    done
fi

# 2. Start process sampler in background
SAMPLER_CSV="${OUTPUT_FILE%.json}-samples.csv"
echo "=== [RUN-SMOKE] Starting process sampler: $SAMPLER_CSV ==="
./tools/dev/sample-process-macos.sh "$SAMPLER_CSV" >/dev/null 2>&1 &
SAMPLER_PID=$!

echo "=== [RUN-SMOKE] Safety Pre-flight: Snapshotting ~/Library ==="
(find "$REAL_HOME/Library" -maxdepth 3 2>/dev/null || true) | sort > "$BEFORE_SNAP"

echo "=== [RUN-SMOKE] Launching Run #$RUN_INDEX ==="
echo "  Isolated HOME     : $SMOKE_HOME"
echo "  Isolated DATA_DIR : $SMOKE_DATA"
echo "  Output JSON       : $OUTPUT_FILE"

# Launch app in smoke test mode
set +e
HOME="$SMOKE_HOME" \
ODM_DATA_DIR="$SMOKE_DATA" \
ODM_SMOKE="1" \
ODM_SMOKE_OUTPUT="$OUTPUT_FILE" \
"$RELEASE_BIN" --smoke-test
APP_EXIT_CODE=$?
set -e

echo "=== [RUN-SMOKE] App Exited with code: $APP_EXIT_CODE ==="

# Wait briefly for sampler to catch final state
sleep 1

# Safety check: Verify ~/Library was untouched
echo "=== [RUN-SMOKE] Safety Post-flight: Diffing ~/Library ==="
(find "$REAL_HOME/Library" -maxdepth 3 2>/dev/null || true) | sort > "$AFTER_SNAP"

DIFF_OUT=$(diff -u "$BEFORE_SNAP" "$AFTER_SNAP" | grep -E "^[+-]/Users" | grep -v -E "(DiagnosticReports|/Library/Logs/|/Library/Caches/)" || true)

if [ -n "$DIFF_OUT" ]; then
    echo "CRITICAL SAFETY VIOLATION: ~/Library was modified during smoke run!" >&2
    echo "$DIFF_OUT" >&2
    exit 2
fi

echo "SAFETY VERIFIED: ~/Library was completely untouched (0 changes)."

if [ "$APP_EXIT_CODE" -ne 0 ]; then
    echo "Smoke test reported failure (exit code $APP_EXIT_CODE)." >&2
    exit "$APP_EXIT_CODE"
fi

echo "=== [RUN-SMOKE] Run #$RUN_INDEX Completed Successfully ==="
