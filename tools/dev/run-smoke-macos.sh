#!/usr/bin/env bash
# Copyright (c) 2026 Open Download Manager contributors
# SPDX-License-Identifier: MIT
#
# tools/dev/run-smoke-macos.sh
# Runs Open Download Manager in smoke test mode with isolated temporary HOME & ODM_DATA_DIR.
# Verifies zero modifications to ~/Library before and after the run.

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

echo "=== [RUN-SMOKE] Safety Pre-flight: Snapshotting ~/Library ==="
(find "$REAL_HOME/Library" -maxdepth 3 2>/dev/null || true) | sort > "$BEFORE_SNAP"

SMOKE_TMP=$(mktemp -d /tmp/odm-smoke-isolated-XXXXXX)
SMOKE_HOME="$SMOKE_TMP/home"
SMOKE_DATA="$SMOKE_TMP/data"
mkdir -p "$SMOKE_HOME" "$SMOKE_DATA"

mkdir -p "$(dirname "$OUTPUT_FILE")"

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

# Safety check: Verify ~/Library was untouched
echo "=== [RUN-SMOKE] Safety Post-flight: Diffing ~/Library ==="
(find "$REAL_HOME/Library" -maxdepth 3 2>/dev/null || true) | sort > "$AFTER_SNAP"

DIFF_OUT=$(diff -u "$BEFORE_SNAP" "$AFTER_SNAP" | grep -E "^[+-]/Users" | grep -v -E "(DiagnosticReports|/Library/Logs/|/Library/Caches/)" || true)
rm -f "$BEFORE_SNAP" "$AFTER_SNAP"
rm -rf "$SMOKE_TMP"

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
