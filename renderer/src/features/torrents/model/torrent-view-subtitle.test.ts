import { describe, expect, it } from "vitest";
import { makeTorrent } from "@/test/fixtures";
import { rowSubtitle } from "./torrent-view";

const torrent = makeTorrent;

describe("rowSubtitle", () => {
  it("formats seeding subtitle with ratio, speed, and uploaded bytes", () => {
    expect(
      rowSubtitle(
        torrent({
          state: "live",
          finished: true,
          progressBytes: 1024,
          totalBytes: 1024,
          ratio: 1.25,
          uploadSpeed: 2048,
          uploadedBytes: 4096,
        }),
      ),
    ).toBe("Seeding · ↑ 2.0 KB/s · Uploaded 4.0 KB · Ratio 1.25");

    expect(
      rowSubtitle(
        torrent({
          state: "live",
          finished: true,
          progressBytes: 1024,
          totalBytes: 1024,
          ratio: 0,
          uploadSpeed: 0,
          uploadedBytes: 0,
        }),
      ),
    ).toBe("Seeding");
  });

  it("formats downloading subtitle with progress, speed, and eta", () => {
    expect(
      rowSubtitle(
        torrent({
          state: "live",
          finished: false,
          progressBytes: 512,
          totalBytes: 1024,
          downloadSpeed: 1024,
          timeRemainingSecs: 60,
        }),
      ),
    ).toBe("50.0% · ↓ 1.0 KB/s · 1m 0s left");
  });

  it("formats paused subtitle differently for finished vs in-progress torrents", () => {
    expect(
      rowSubtitle(
        torrent({
          state: "paused",
          finished: true,
          progressBytes: 1024,
          totalBytes: 1024,
        }),
      ),
    ).toBe("Completed");

    expect(
      rowSubtitle(
        torrent({
          state: "paused",
          finished: false,
          progressBytes: 256,
          totalBytes: 1024,
        }),
      ),
    ).toBe("Paused at 25.0%");
  });
});
