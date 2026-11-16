#!/usr/bin/env bash
# Copyright (c) 2026 Open Download Manager contributors
# SPDX-License-Identifier: MIT
#
# tools/dev/sample-process-macos.sh
# macOS Process Sampler for Open Download Manager & WebKit Helper Processes
#
# ATTRIBUTION METHODOLOGY & LIMITATIONS:
# -------------------------------------
# 1. On macOS, WebKit helper processes (com.apple.WebKit.WebContent, com.apple.WebKit.Networking,
#    com.apple.WebKit.GPU) are managed as sandboxed XPC services by launchd (PPID 1). They are NOT
#    direct children of the Tauri/Wry host process, so parent-child process tree queries (e.g. pgrep -P)
#    cannot discover them.
# 2. This script attributes WebKit helpers by:
#    a. Taking a baseline snapshot of all pre-existing WebKit helper PIDs prior to application launch.
#    b. Periodically scanning for newly appeared WebKit helper PIDs that did not exist in the baseline snapshot.
# 3. Limitations:
#    - If other WebKit-based applications (e.g. Safari, Mail, Slack, Discord) spawn or terminate WebKit
#      helpers during the measurement window, those helpers may be erroneously attributed to Open Download Manager.
#    - For strict isolation, close Safari and all other WebKit-based applications before running the benchmark.
#    - XPC helper lifetimes are managed by the OS and may linger briefly after window closures.

set -euo pipefail

TARGET_NAME="open-download-manager"
OUTPUT_CSV="${1:-macos-process-samples.csv}"
INTERVAL=1

echo "========================================================================="
echo " macOS Process Resource Sampler (Host + WebKit Helpers)"
echo "========================================================================="
echo "Target Application : $TARGET_NAME"
echo "Sampling Interval  : ${INTERVAL}s"
echo "Output CSV         : $OUTPUT_CSV"
echo "========================================================================="

# Snapshot baseline WebKit PIDs
BASELINE_WEBKIT_PIDS=$(pgrep -f "com.apple.WebKit" | sort || true)
if [ -n "$BASELINE_WEBKIT_PIDS" ]; then
    echo "Notice: Detected pre-existing WebKit helper PIDs (will be excluded from attribution):"
    echo "  $BASELINE_WEBKIT_PIDS"
fi

echo "Waiting for $TARGET_NAME process to appear..."
HOST_PID=""
while [ -z "$HOST_PID" ]; do
    HOST_PID=$(pgrep -x "$TARGET_NAME" | head -n 1 || true)
    if [ -z "$HOST_PID" ]; then
        HOST_PID=$(pgrep -f "open-download-manager" | grep -v "sample-process" | head -n 1 || true)
    fi
    sleep 0.5
done

echo "Found $TARGET_NAME PID: $HOST_PID"
echo "Starting continuous 1-second sampling (Press Ctrl+C to stop)..."

# CSV Header
echo "timestamp,host_pid,host_cpu_pct,host_rss_mb,webkit_pids,webkit_cpu_pct,webkit_rss_mb,total_cpu_pct,total_rss_mb" > "$OUTPUT_CSV"
printf "%-20s | %-12s | %-14s | %-12s | %-14s | %-12s | %-14s\n" \
    "Timestamp" "Host CPU%" "Host RSS (MB)" "WebKit CPU%" "WebKit RSS (MB)" "Total CPU%" "Total RSS (MB)"
echo "------------------------------------------------------------------------------------------------------------------"

sample_count=0

cleanup() {
    echo ""
    echo "Sampler stopped. Captured $sample_count samples."
    echo "Results written to: $OUTPUT_CSV"
    exit 0
}
trap cleanup SIGINT SIGTERM

while kill -0 "$HOST_PID" 2>/dev/null; do
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    # Sample Host Process
    HOST_STATS=$(ps -p "$HOST_PID" -o %cpu=,rss= 2>/dev/null || echo "0.0 0")
    HOST_CPU=$(echo "$HOST_STATS" | awk '{print $1}')
    HOST_RSS_KB=$(echo "$HOST_STATS" | awk '{print $2}')
    HOST_RSS_MB=$(awk "BEGIN {printf \"%.2f\", $HOST_RSS_KB / 1024}")

    # Discover and sample attributed WebKit helper processes
    ALL_WEBKIT=$(pgrep -f "com.apple.WebKit" || true)
    ATTRIBUTED_WEBKIT_PIDS=""
    WEBKIT_CPU=0.0
    WEBKIT_RSS_KB=0

    for wpid in $ALL_WEBKIT; do
        # Exclude baseline PIDs
        if ! echo "$BASELINE_WEBKIT_PIDS" | grep -q "^$wpid$"; then
            ATTRIBUTED_WEBKIT_PIDS="${ATTRIBUTED_WEBKIT_PIDS:+$ATTRIBUTED_WEBKIT_PIDS;}$wpid"
            WSTATS=$(ps -p "$wpid" -o %cpu=,rss= 2>/dev/null || echo "0.0 0")
            WCPU=$(echo "$WSTATS" | awk '{print $1}')
            WRSS=$(echo "$WSTATS" | awk '{print $2}')
            WEBKIT_CPU=$(awk "BEGIN {printf \"%.2f\", $WEBKIT_CPU + $WCPU}")
            WEBKIT_RSS_KB=$((WEBKIT_RSS_KB + WRSS))
        fi
    done

    WEBKIT_RSS_MB=$(awk "BEGIN {printf \"%.2f\", $WEBKIT_RSS_KB / 1024}")
    TOTAL_CPU=$(awk "BEGIN {printf \"%.2f\", $HOST_CPU + $WEBKIT_CPU}")
    TOTAL_RSS_MB=$(awk "BEGIN {printf \"%.2f\", $HOST_RSS_MB + $WEBKIT_RSS_MB}")

    # Log to CSV
    echo "$TIMESTAMP,$HOST_PID,$HOST_CPU,$HOST_RSS_MB,\"$ATTRIBUTED_WEBKIT_PIDS\",$WEBKIT_CPU,$WEBKIT_RSS_MB,$TOTAL_CPU,$TOTAL_RSS_MB" >> "$OUTPUT_CSV"

    # Print to console
    printf "%-20s | %10s %% | %11s MB | %10s %% | %11s MB | %10s %% | %11s MB\n" \
        "$TIMESTAMP" "$HOST_CPU" "$HOST_RSS_MB" "$WEBKIT_CPU" "$WEBKIT_RSS_MB" "$TOTAL_CPU" "$TOTAL_RSS_MB"

    sample_count=$((sample_count + 1))
    sleep "$INTERVAL"
done

echo "Target process $HOST_PID terminated."
cleanup
