import { describe, expect, it } from "vitest";
import { makeSettings } from "@/test/fixtures";
import {
  buildSettingsPatch,
  cleanHostPattern,
  draftFromSettings,
  formatWindowLimitUsage,
  parseFileTypes,
  parseList,
  validateSettingsDraft,
} from "./settings-draft";

describe("settings-draft parsers", () => {
  it("parses file types stripping dots and lowercasing", () => {
    expect(parseFileTypes(".ZIP, .tar.gz   mp4\niso")).toEqual([
      "zip",
      "tar.gz",
      "mp4",
      "iso",
    ]);
    expect(parseFileTypes("")).toEqual([]);
  });

  it("parses comma-separated lists", () => {
    expect(parseList("example.com,  *.domain.com , internal")).toEqual([
      "example.com",
      "*.domain.com",
      "internal",
    ]);
  });

  it("cleans host patterns removing protocol, path, and port", () => {
    expect(cleanHostPattern("https://example.com:8080/path")).toBe("example.com");
    expect(cleanHostPattern("HTTP://*.Sub.Domain.com/")).toBe("*.sub.domain.com");
    expect(cleanHostPattern("127.0.0.1:3000")).toBe("127.0.0.1");
  });

  it("formats window limit usage string", () => {
    expect(formatWindowLimitUsage(null)).toBe("");
    expect(formatWindowLimitUsage({ enabled: false } as never)).toBe("");
    expect(
      formatWindowLimitUsage({
        enabled: true,
        currentBytes: 1024 * 1024 * 512,
        maxBytes: 1024 * 1024 * 2048,
        stoppedUntil: null,
      } as never),
    ).toBe("Current usage: 512.0 / 2048 MB in rolling window.");

    expect(
      formatWindowLimitUsage({
        enabled: true,
        currentBytes: 1024 * 1024 * 2048,
        maxBytes: 1024 * 1024 * 2048,
        stoppedUntil: "14:00",
      } as never),
    ).toContain("(Quota exceeded; auto-resumes at 14:00)");
  });
});

describe("draftFromSettings and validation", () => {
  it("converts settings into mutable draft", () => {
    const settings = makeSettings({
      speedLimitKbps: 100,
      completionActions: [
        { type: "playSound", sound_file: "ding.wav" },
        { type: "powerAction", kind: "Shutdown", confirmed: true },
      ],
      windowLimit: {
        enabled: true,
        windowHours: 8,
        maxBytes: 1024 * 1024 * 1024,
        windowAnchor: "2026-11-14T10:00:00Z",
        currentBytes: 100,
        stoppedUntil: null,
      },
    });

    const draft = draftFromSettings(settings);
    expect(draft.speedLimitKbps).toBe(100);
    expect(draft.playSound).toBe(true);
    expect(draft.soundFile).toBe("ding.wav");
    expect(draft.powerActionKind).toBe("Shutdown");
    expect(draft.powerConfirm).toBe(true);
    expect(draft.windowLimitHours).toBe(8);
    expect(draft.windowLimitMaxMb).toBe(1024);
  });

  it("validates draft constraints", () => {
    const draft = draftFromSettings(makeSettings());
    expect(validateSettingsDraft(draft)).toBeNull();

    expect(
      validateSettingsDraft({ ...draft, defaultDirectory: "   " }),
    ).toBe("Default download folder is required.");

    expect(validateSettingsDraft({ ...draft, maxConcurrent: 0 })).toBe(
      "Maximum simultaneous downloads must be between 1 and 16.",
    );
    expect(validateSettingsDraft({ ...draft, connections: 33 })).toBe(
      "Maximum connections per file must be between 1 and 32.",
    );
    expect(validateSettingsDraft({ ...draft, speedLimitKbps: -1 })).toBe(
      "Speed limit must be between 0 and 1,048,576 KB/s.",
    );

    expect(
      validateSettingsDraft({
        ...draft,
        windowLimitEnabled: true,
        windowLimitMaxMb: 0,
      }),
    ).toBe("Max data volume must be at least 1 MB.");

    expect(
      validateSettingsDraft({
        ...draft,
        proxyEnabled: true,
        proxyProtocol: "http",
        proxyHost: "",
      }),
    ).toBe("Proxy host is required when proxy is enabled.");
  });

  it("builds settings patch matching engine requirements", () => {
    const settings = makeSettings();
    const draft = draftFromSettings(settings);
    draft.temporaryDirectory = "";
    draft.playSound = true;
    draft.soundFile = "chime.wav";
    draft.scanAntivirus = true;
    draft.scannerExecutable = "/usr/bin/scan";
    draft.scannerArgsText = "--arg1 {file}";
    draft.powerActionKind = "Sleep";
    draft.powerConfirm = false;

    const patch = buildSettingsPatch(draft, settings);
    expect(patch.temporaryDirectory).toBe("");
    expect(patch.completionActions).toHaveLength(3);
    expect(patch.completionActions).toEqual([
      { type: "playSound", sound_file: "chime.wav" },
      {
        type: "antivirusScan",
        executable: "/usr/bin/scan",
        arguments: ["--arg1", "{file}"],
      },
      { type: "powerAction", kind: "Sleep", confirmed: false },
    ]);
  });
});
