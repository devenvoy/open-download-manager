import { describe, expect, it } from "vitest";
import { makeTorrentFile } from "@/test/fixtures";
import {
  fileProgressPercent,
  friendlyError,
  includedIndices,
  numberValue,
  reviewPlan,
  validateSettingsInput,
} from "./torrent-view";

/** File selection and settings rules, split from the state-formatting suite. */

describe("includedIndices", () => {
  it("returns sorted included file indexes", () => {
    expect(includedIndices(null)).toEqual([]);
    expect(
      includedIndices([
        makeTorrentFile({ index: 3, included: true }),
        makeTorrentFile({ index: 1, included: false }),
        makeTorrentFile({ index: 2, included: true }),
        makeTorrentFile({ index: -1, included: true }),
      ]),
    ).toEqual([2, 3]);
  });
});

describe("reviewPlan", () => {
  const files = [
    makeTorrentFile({ index: 0, included: true }),
    makeTorrentFile({ index: 1, included: false }),
    makeTorrentFile({ index: 2, included: true }),
  ];

  it("uses the files marked as included when nothing was picked", () => {
    const plan = reviewPlan(files, null, false);
    expect(plan.files).toEqual([0, 2]);
    expect(plan.action).toBe("resume");
    expect(plan.error).toBeNull();
  });

  it("honours the explicit file selection", () => {
    const plan = reviewPlan(files, new Set([1]), false);
    expect(plan.files).toEqual([1]);
    expect(plan.action).toBe("resume");
    expect(plan.error).toBeNull();
  });

  it("sorts and drops invalid indices", () => {
    const plan = reviewPlan(files, new Set([3, 1, -1]), false);
    expect(plan.files).toEqual([1, 3]);
  });

  it("reports when the file list is not ready", () => {
    const plan = reviewPlan([], null, false);
    expect(plan.files).toEqual([]);
    expect(plan.action).toBe("resume");
    expect(plan.error).toMatch(/file list is not ready/i);
  });

  it("reports an empty selection", () => {
    const plan = reviewPlan(
      [makeTorrentFile({ index: 0, included: false })],
      new Set(),
      false,
    );
    expect(plan.files).toEqual([]);
    expect(plan.error).toMatch(/at least one file/i);
  });

  it("keeps the torrent paused when the setting asks for it", () => {
    const plan = reviewPlan(files, null, true);
    expect(plan.files).toEqual([0, 2]);
    expect(plan.action).toBe("pause");
    expect(plan.error).toBeNull();
  });
});

describe("friendlyError", () => {
  it("turns engine wording into readable text", () => {
    expect(friendlyError(new Error("torrent is already live"))).toBe(
      "This torrent is already running.",
    );
    expect(friendlyError(new Error("torrent is not resolved"))).toBe(
      "The torrent metadata is not ready yet. Wait a moment and try again.",
    );
    expect(friendlyError(new Error("No space left on device"))).toBe(
      "There is not enough space left on the disk.",
    );
    expect(friendlyError(new Error(""))).toBe(
      "The action could not be completed.",
    );
    expect(friendlyError(new Error("something else went wrong"))).toBe(
      "something else went wrong",
    );
  });
});

describe("fileProgressPercent", () => {
  it("handles partial and missing sizes", () => {
    expect(fileProgressPercent(null)).toBe(0);
    expect(fileProgressPercent(makeTorrentFile({ length: 0, progressBytes: 0 }))).toBe(
      0,
    );
    expect(fileProgressPercent(makeTorrentFile({ length: 200, progressBytes: 50 }))).toBe(
      25,
    );
    expect(
      fileProgressPercent(makeTorrentFile({ length: 200, progressBytes: 400 })),
    ).toBe(100);
  });
});

describe("numberValue", () => {
  it("falls back for empty or invalid input", () => {
    expect(numberValue("", 7)).toBe(7);
    expect(numberValue(null, 7)).toBe(7);
    expect(numberValue(undefined, 7)).toBe(7);
    expect(numberValue("12.5", 7)).toBe(12.5);
    expect(numberValue("nope", 7)).toBe(7);
  });
});

describe("validateSettingsInput", () => {
  const base = {
    listenPort: 0,
    maxPeersPerTorrent: 50,
    downloadLimitKbps: 0,
    uploadLimitKbps: 0,
    seedRatioLimit: 0,
    seedTimeLimitMinutes: 0,
    downloadDirectory: "",
  };

  it("accepts safe values", () => {
    expect(validateSettingsInput(base)).toBeNull();
    expect(
      validateSettingsInput({ ...base, downloadDirectory: "/Users/me/Torrents" }),
    ).toBeNull();
    expect(validateSettingsInput({ ...base, downloadDirectory: "C:\\Torrents" })).toBeNull();
  });

  it("rejects unsafe values", () => {
    expect(validateSettingsInput({ ...base, listenPort: 70_000 })).toMatch(
      /Listen port/,
    );
    expect(validateSettingsInput({ ...base, maxPeersPerTorrent: 0 })).toMatch(
      /Max peers/,
    );
    expect(validateSettingsInput({ ...base, downloadLimitKbps: -1 })).toMatch(
      /download limit/,
    );
    expect(validateSettingsInput({ ...base, seedRatioLimit: 2_000 })).toMatch(
      /Seed ratio/,
    );
    expect(validateSettingsInput({ ...base, seedTimeLimitMinutes: 600_000 })).toMatch(
      /Seed time/,
    );
    expect(validateSettingsInput({ ...base, downloadDirectory: "relative/path" })).toMatch(
      /absolute/,
    );
  });
});
