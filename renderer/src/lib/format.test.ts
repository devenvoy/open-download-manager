import { describe, expect, it } from "vitest";
import { formatBytes, formatDuration, formatPercent, formatSpeed } from "./format";

describe("format helpers", () => {
  it("formats bytes across units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 ** 2)).toBe("5.0 MB");
  });

  it("formats a transfer speed", () => {
    expect(formatSpeed(1024)).toBe("1.0 KB/s");
  });

  it("formats a percentage and handles an unknown total", () => {
    expect(formatPercent(50, 100)).toBe("50%");
    expect(formatPercent(50, null)).toBe("—");
    expect(formatPercent(200, 100)).toBe("100%");
  });

  it("formats a duration", () => {
    expect(formatDuration(45)).toBe("45s");
    expect(formatDuration(125)).toBe("2m 5s");
    expect(formatDuration(3700)).toBe("1h 1m");
    expect(formatDuration(0)).toBe("—");
  });
});
