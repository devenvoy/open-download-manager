import { describe, expect, it } from "vitest";
import { makeTorrent } from "@/test/fixtures";
import {
  availableActions,
  formatEta,
  formatRatio,
  progressPercent,
  staleTorrentIds,
  stateKey,
  stateLabel,
  stateShortLabel,
  summarize,
  torrentKey,
} from "./torrent-view";

/**
 * Ported from `reference/legacy-renderer/torrents.test.cjs`: the rules the
 * window relied on before the React migration must behave identically.
 */

const torrent = makeTorrent;

describe("stateKey", () => {
  it("derives a state from the engine payload", () => {
    expect(stateKey(null)).toBe("unknown");
    expect(
      stateKey({ state: "live", finished: false, seeds: 2, downloadSpeed: 1024 }),
    ).toBe("downloading");
    expect(stateKey({ state: "live", finished: true, progressBytes: 512 })).toBe(
      "seeding",
    );
    expect(stateKey({ state: "paused", finished: false })).toBe("paused");
    expect(stateKey({ state: "initializing" })).toBe("metadata");
    expect(stateKey({ state: "error" })).toBe("error");
    expect(stateKey({ state: "other" })).toBe("unknown");
  });

  it("reports a running torrent with no peer that has the data", () => {
    expect(
      stateKey({ state: "live", finished: false, seeds: 0, downloadSpeed: 0 }),
    ).toBe("waiting");
    expect(
      stateKey({
        state: "live",
        finished: false,
        seeds: 0,
        leechers: 4,
        downloadSpeed: 0,
      }),
    ).toBe("waiting");
    expect(
      stateKey({ state: "live", finished: false, seeds: 1, downloadSpeed: 0 }),
    ).toBe("downloading");
    expect(
      stateKey({
        state: "live",
        finished: false,
        seeds: 0,
        downloadSpeed: 0,
        progressBytes: 4096,
      }),
    ).toBe("downloading");
  });

  it("does not claim seeding before anything was downloaded", () => {
    expect(stateKey({ state: "live", finished: true, progressBytes: 0 })).toBe(
      "idle",
    );
    expect(
      stateKey({ state: "live", finished: true, progressBytes: 1024 }),
    ).toBe("seeding");
    expect(stateKey({ state: "paused", finished: true })).toBe("paused");
  });
});

describe("labels", () => {
  it("uses readable state labels", () => {
    expect(
      stateLabel({ state: "live", finished: false, seeds: 1, downloadSpeed: 1024 }),
    ).toBe("Downloading");
    expect(
      stateLabel({ state: "live", finished: false, seeds: 0, downloadSpeed: 0 }),
    ).toBe("Waiting for peers");
    expect(stateLabel({ state: "live", finished: true, progressBytes: 512 })).toBe(
      "Seeding",
    );
    expect(stateLabel({ state: "live", finished: true, progressBytes: 0 })).toBe(
      "Nothing selected",
    );
    expect(stateLabel({ state: "paused" })).toBe("Paused");
    expect(stateLabel({ state: "initializing" })).toBe("Fetching metadata");
    expect(stateLabel({ state: "error" })).toBe("Error");
    expect(stateLabel({ state: "other" })).toBe("Unknown");
  });

  it("shortens metadata and waiting labels", () => {
    expect(stateShortLabel({ state: "initializing" })).toBe("Metadata");
    expect(stateShortLabel({ state: "live", finished: true })).toBe(
      "Nothing selected",
    );
    expect(stateShortLabel({ state: "live", finished: true, progressBytes: 512 })).toBe(
      "Seeding",
    );
    expect(
      stateShortLabel({ state: "live", finished: false, seeds: 0, downloadSpeed: 0 }),
    ).toBe("Waiting");
  });
});

describe("progressPercent", () => {
  it("clamps and handles missing totals", () => {
    expect(progressPercent(null)).toBe(0);
    expect(progressPercent(torrent({ progressBytes: 512, totalBytes: 1024 }))).toBe(
      50,
    );
    expect(progressPercent(torrent({ progressBytes: 0, totalBytes: 0 }))).toBe(0);
    expect(progressPercent(torrent({ progressBytes: 2048, totalBytes: 1024 }))).toBe(
      100,
    );
  });

  it("does not claim a finished torrent that downloaded nothing", () => {
    expect(
      progressPercent(torrent({ finished: true, progressBytes: 0, totalBytes: 1024 })),
    ).toBe(0);
    expect(
      progressPercent(torrent({ finished: true, progressBytes: 1024, totalBytes: 1024 })),
    ).toBe(100);
  });
});

describe("formatRatio", () => {
  it("keeps two decimals and never shows negatives", () => {
    expect(formatRatio(0.1234)).toBe("0.12");
    expect(formatRatio(1)).toBe("1.00");
    expect(formatRatio(0)).toBe("0.00");
    expect(formatRatio(-2)).toBe("0.00");
    expect(formatRatio("not a number" as unknown as number)).toBe("0.00");
  });
});

describe("formatEta", () => {
  it("renders compact durations", () => {
    expect(formatEta(0)).toBe("—");
    expect(formatEta(null)).toBe("—");
    expect(formatEta(-10)).toBe("—");
    expect(formatEta(30)).toBe("30s");
    expect(formatEta(90)).toBe("1m 30s");
    expect(formatEta(3700)).toBe("1h 1m");
    expect(formatEta(90_000)).toBe("1d 1h");
  });
});

describe("availableActions", () => {
  it("guards actions by torrent state", () => {
    expect(availableActions(null)).toEqual({
      pause: false,
      resume: false,
      recheck: false,
      forget: false,
      delete: false,
    });
    expect(availableActions(torrent({ state: "live" }))).toEqual({
      pause: true,
      resume: false,
      recheck: true,
      forget: true,
      delete: true,
    });
    expect(availableActions(torrent({ state: "paused" }))).toEqual({
      pause: false,
      resume: true,
      recheck: true,
      forget: true,
      delete: true,
    });
    expect(availableActions(torrent({ state: "initializing" }))).toEqual({
      pause: true,
      resume: false,
      recheck: false,
      forget: true,
      delete: true,
    });
    expect(availableActions(torrent({ state: "error" }))).toEqual({
      pause: false,
      resume: true,
      recheck: true,
      forget: true,
      delete: true,
    });
  });
});

describe("torrentKey", () => {
  it("identifies a download by its info hash, not by its id", () => {
    const first = { id: 0, infoHash: "aaaa" };
    const second = { id: 0, infoHash: "bbbb" };
    expect(torrentKey(first)).toBe("aaaa");
    expect(torrentKey(second)).toBe("bbbb");
    expect(torrentKey(first)).not.toBe(torrentKey(second));
    expect(torrentKey({ id: 7 })).toBe("id:7");
    expect(torrentKey(null)).toBe("");
  });
});

describe("staleTorrentIds", () => {
  it("finds cache entries that belong to another download", () => {
    const known = new Map([
      [0, "aaaa"],
      [1, "bbbb"],
    ]);
    const torrents = [
      makeTorrent({ id: 1, infoHash: "cccc" }),
    ];
    expect([...staleTorrentIds(known, torrents)].sort()).toEqual([0, 1]);
    expect(
      staleTorrentIds(known, [
        makeTorrent({ id: 0, infoHash: "aaaa" }),
        makeTorrent({ id: 1, infoHash: "bbbb" }),
      ]).size,
    ).toBe(0);
    expect(staleTorrentIds(new Map(), torrents).size).toBe(0);
  });
});

describe("summarize", () => {
  it("aggregates counts and speeds", () => {
    const summary = summarize([
      torrent({ id: 1, state: "live", finished: false, downloadSpeed: 100, uploadSpeed: 10 }),
      torrent({ id: 2, state: "live", finished: true, downloadSpeed: 0, uploadSpeed: 20 }),
      torrent({ id: 3, state: "paused", downloadSpeed: 0, uploadSpeed: 0 }),
      torrent({ id: 4, state: "initializing", downloadSpeed: 50, uploadSpeed: 0 }),
    ]);
    expect(summary).toEqual({
      count: 4,
      downloading: 2,
      waiting: 0,
      seeding: 1,
      paused: 1,
      downloadSpeed: 150,
      uploadSpeed: 30,
    });
    expect(summarize(null)).toEqual({
      count: 0,
      downloading: 0,
      waiting: 0,
      seeding: 0,
      paused: 0,
      downloadSpeed: 0,
      uploadSpeed: 0,
    });
  });

  it("counts a torrent that waits for peers separately", () => {
    const summary = summarize([
      torrent({
        id: 1,
        state: "live",
        finished: false,
        seeds: 0,
        progressBytes: 0,
        downloadSpeed: 0,
      }),
    ]);
    expect(summary.waiting).toBe(1);
    expect(summary.downloading).toBe(0);
  });
});
