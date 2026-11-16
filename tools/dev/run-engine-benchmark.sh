#!/usr/bin/env bash
# Copyright (c) 2026 Open Download Manager contributors
# SPDX-License-Identifier: MIT
#
# tools/dev/run-engine-benchmark.sh
# Runs local synthetic HTTP benchmark server and executes the engine finish-phase benchmark.

set -euo pipefail

PORT=8999
SERVER_URL="http://127.0.0.1:$PORT"
OUTPUT_FILE="${1:-docs/audit/evidence/baseline-macos/engine-finish-phase.json}"

echo "=== [ENGINE-BENCH] Starting Synthetic Benchmark Server on $SERVER_URL ==="
BENCHMARK_PORT="$PORT" bun tools/dev/benchmark-server.ts > /tmp/odm-benchmark-server.log 2>&1 &
SERVER_PID=$!

cleanup() {
    echo "=== [ENGINE-BENCH] Stopping Synthetic Server (PID: $SERVER_PID) ==="
    kill -9 "$SERVER_PID" 2>/dev/null || true
    rm -f /tmp/odm-benchmark-server.log
}
trap cleanup EXIT INT TERM

# Wait for server to become responsive
for i in {1..30}; do
    if curl -s -f "$SERVER_URL/health" > /dev/null 2>&1; then
        echo "=== [ENGINE-BENCH] Synthetic Server is healthy and listening. ==="
        break
    fi
    sleep 0.2
done

mkdir -p "$(dirname "$OUTPUT_FILE")"

echo "=== [ENGINE-BENCH] Executing Engine Benchmark Binary ==="
ENGINE_BENCH_OUTPUT="$OUTPUT_FILE" target/release/engine_benchmark "$SERVER_URL"

echo "=== [ENGINE-BENCH] Engine Benchmark Finished Successfully ==="
