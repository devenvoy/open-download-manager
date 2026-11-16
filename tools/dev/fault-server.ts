// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(test-infra): clean-room HTTP fault injection server for chaos network testing (W8)

/**
 * Clean-room fault injection server for testing download engine resilience.
 * Supports configurable connection drops, rate limiting (429), ETag flipping,
 * bandwidth throttling, and non-range server responses.
 *
 * Endpoints:
 * - /health
 * - /bytes?size=<N> : Normal synthetic payload with Range & ETag
 * - /fault/drop?size=<N>&drop_after=<M> : Forcefully closes socket after M bytes
 * - /fault/ratelimit?max_concurrency=<K> : Returns 429 when concurrency > K
 * - /fault/etag-flip?size=<N> : Changes ETag on subsequent requests
 * - /fault/slow?size=<N>&rate_kbps=<R> : Throttles to R KB/s
 * - /fault/no-range?size=<N> : Ignores Range header and serves full 200 OK
 */

const PORT = parseInt(process.env.FAULT_SERVER_PORT || "8998", 10);
const CHUNK_SIZE = 64 * 1024; // 64 KiB buffer

const PATTERN = new Uint8Array(CHUNK_SIZE);
for (let i = 0; i < CHUNK_SIZE; i++) {
  PATTERN[i] = (i * 33 + 7) & 0xff;
}

let activeConnections = 0;
let etagFlipCounter = 0;

function resolveTotalSize(url: URL, defaultSize: number = 10 * 1024 * 1024): number {
  if (url.searchParams.has("size")) {
    return parseInt(url.searchParams.get("size")!, 10);
  }
  return defaultSize;
}

const server = Bun.serve({
  port: PORT,
  fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/health") {
      return new Response("OK", { status: 200 });
    }

    // 1. Rate limiting fault (HTTP 429)
    const maxConnsParam = url.searchParams.get("max_conns") || url.searchParams.get("max_concurrency");
    if (maxConnsParam !== null || url.pathname === "/fault/ratelimit") {
      const maxConcurrency = parseInt(maxConnsParam || "1", 10);
      if (activeConnections >= maxConcurrency) {
        const retryAfter = url.searchParams.get("retry_after") || "1";
        return new Response("Too Many Requests - Concurrency limit exceeded", {
          status: 429,
          headers: {
            "Retry-After": retryAfter,
            "Content-Type": "text/plain",
          },
        });
      }
    }

    // 2. No Range support fault (ignores Range header, serves 200 OK)
    if (url.pathname === "/fault/no-range") {
      const totalSize = resolveTotalSize(url);
      if (req.method === "HEAD") {
        return new Response(null, {
          status: 200,
          headers: {
            "Content-Length": totalSize.toString(),
            "Content-Type": "application/octet-stream",
            "Accept-Ranges": "none",
          },
        });
      }

      let remaining = totalSize;
      const stream = new ReadableStream({
        pull(controller) {
          if (remaining <= 0) {
            controller.close();
            return;
          }
          const toSend = Math.min(remaining, CHUNK_SIZE);
          controller.enqueue(PATTERN.subarray(0, toSend));
          remaining -= toSend;
        },
      });

      return new Response(stream, {
        status: 200,
        headers: {
          "Content-Length": totalSize.toString(),
          "Content-Type": "application/octet-stream",
          "Accept-Ranges": "none",
        },
      });
    }

    // 3. ETag Flip fault (ETag changes between requests)
    let etag = `"normal-etag"`;
    if (url.pathname === "/fault/etag-flip") {
      etagFlipCounter++;
      const version = etagFlipCounter <= 1 ? "v1" : "v2";
      etag = `"etag-flip-${version}"`;

      const ifMatch = req.headers.get("if-match");
      if (ifMatch && ifMatch !== etag && ifMatch !== "*") {
        return new Response("Precondition Failed", { status: 412 });
      }

      const ifRange = req.headers.get("if-range");
      if (ifRange && ifRange !== etag) {
        // If-Range mismatch sends full 200 OK per RFC 7233
        const totalSize = resolveTotalSize(url);
        return new Response(PATTERN.subarray(0, Math.min(totalSize, CHUNK_SIZE)), {
          status: 200,
          headers: {
            "Content-Length": totalSize.toString(),
            "ETag": etag,
            "Accept-Ranges": "bytes",
          },
        });
      }
    }

    const totalSize = resolveTotalSize(url);

    if (req.method === "HEAD") {
      return new Response(null, {
        status: 200,
        headers: {
          "Content-Length": totalSize.toString(),
          "Accept-Ranges": "bytes",
          "ETag": etag,
          "Content-Type": "application/octet-stream",
        },
      });
    }

    // 4. Dropped connection fault (closes socket after drop_after bytes)
    const isDropFault = url.pathname === "/fault/drop";
    const dropAfter = isDropFault
      ? parseInt(url.searchParams.get("drop_after") || "1048576", 10) // 1 MB default
      : Infinity;

    // 5. Throttled / slow stream fault
    const isSlowFault = url.pathname === "/fault/slow";
    const rateKbps = isSlowFault
      ? parseInt(url.searchParams.get("rate_kbps") || "128", 10)
      : 0;

    const rangeHeader = req.headers.get("range");
    let start = 0;
    let end = totalSize - 1;
    let isPartial = false;

    if (rangeHeader) {
      const match = rangeHeader.match(/bytes=(\d+)-(\d*)/);
      if (match) {
        start = parseInt(match[1], 10);
        if (match[2]) {
          end = parseInt(match[2], 10);
        }
        isPartial = true;
      }
    }

    const contentLength = end - start + 1;
    let remaining = contentLength;
    let bytesSent = 0;

    activeConnections++;

    const stream = new ReadableStream({
      async pull(controller) {
        if (isDropFault && bytesSent >= dropAfter) {
          activeConnections = Math.max(0, activeConnections - 1);
          // Forcefully abort stream
          controller.error(new Error("Simulated network connection drop"));
          return;
        }

        if (remaining <= 0) {
          activeConnections = Math.max(0, activeConnections - 1);
          controller.close();
          return;
        }

        const toSend = Math.min(
          remaining,
          isDropFault ? Math.min(CHUNK_SIZE, dropAfter - bytesSent) : CHUNK_SIZE
        );

        if (toSend <= 0 && isDropFault) {
          activeConnections = Math.max(0, activeConnections - 1);
          controller.error(new Error("Simulated network connection drop"));
          return;
        }

        controller.enqueue(PATTERN.subarray(0, toSend));
        remaining -= toSend;
        bytesSent += toSend;

        if (rateKbps > 0) {
          const delayMs = Math.round((toSend / (rateKbps * 1024)) * 1000);
          if (delayMs > 0) {
            await new Promise((resolve) => setTimeout(resolve, delayMs));
          }
        }
      },
      cancel() {
        activeConnections = Math.max(0, activeConnections - 1);
      },
    });

    const headers: Record<string, string> = {
      "Content-Length": contentLength.toString(),
      "Accept-Ranges": "bytes",
      "ETag": etag,
      "Content-Type": "application/octet-stream",
    };

    if (isPartial) {
      headers["Content-Range"] = `bytes ${start}-${end}/${totalSize}`;
    }

    return new Response(stream, {
      status: isPartial ? 206 : 200,
      headers,
    });
  },
});

console.log(`[FAULT_SERVER] Listening on http://127.0.0.1:${PORT}`);
