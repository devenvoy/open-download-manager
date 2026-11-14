import { describe, expect, it } from "vitest";
import { makeDownload } from "@/test/fixtures";
import {
  canCancel,
  canPause,
  canResume,
  detailFacts,
  errorKind,
  errorTitle,
  formatWhen,
  percentage,
  segmentCaption,
  segmentLabel,
  segmentNote,
  segmentStatus,
  segmentsOf,
  sizeLabel,
  statusLabel,
  statusOf,
  timeLeft,
} from "./progress-view";

const downloading = makeDownload({
  id: "1",
  total: 1024,
  downloaded: 512,
  speed: 2048,
  resumable: true,
});

describe("statusOf", () => {
  it("maps backend statuses onto the legacy display names", () => {
    expect(statusOf(null)).toBe("queued");
    expect(statusOf(makeDownload({ status: "probing" }))).toBe("connecting");
    expect(statusOf(makeDownload({ status: "complete" }))).toBe("completed");
    expect(statusOf(makeDownload({ status: "error" }))).toBe("failed");
    expect(statusOf(makeDownload({ status: "assembling" }))).toBe("assembling");
  });

  it("labels every display status", () => {
    expect(statusLabel("downloading")).toBe("Receiving data…");
    expect(statusLabel("paused")).toBe("Stopped");
    expect(statusLabel("assembling")).toBe("Finalizing…");
    expect(statusLabel("completed")).toBe("Complete");
    expect(statusLabel("failed")).toBe("Error");
  });
});

describe("percentage", () => {
  it("reports a completed download as 100 and clamps partial totals", () => {
    expect(percentage(null)).toBeNull();
    expect(percentage(makeDownload({ status: "complete", total: null }))).toBe(100);
    expect(percentage(downloading)).toBe(50);
    expect(percentage(makeDownload({ total: 0 }))).toBeNull();
    expect(percentage(makeDownload({ total: 100, downloaded: 150 }))).toBe(100);
  });
});

describe("sizeLabel", () => {
  it("explains a missing size instead of pretending it is zero", () => {
    expect(sizeLabel(makeDownload({ status: "queued", total: null }))).toBe(
      "Connecting…",
    );
    expect(sizeLabel(makeDownload({ total: 2_048, estimated: false }))).toBe(
      "2.0 KB",
    );
    expect(sizeLabel(makeDownload({ total: 2_048, estimated: true }))).toBe(
      "≈ 2.0 KB",
    );
    expect(sizeLabel(makeDownload({ total: null, kind: "hls" }))).toBe(
      "Calculating from segments…",
    );
    expect(sizeLabel(makeDownload({ total: null, kind: null }))).toBe(
      "Not supplied by server",
    );
  });
});

describe("timeLeft", () => {
  it("estimates the remaining time and marks it when the size is a guess", () => {
    expect(timeLeft(downloading)).toBe("1s");
    expect(timeLeft(makeDownload({ total: 1024, downloaded: 0, speed: 0 }))).toBe(
      "",
    );
    expect(
      timeLeft(
        makeDownload({ total: 1024, downloaded: 0, speed: 1024, estimated: true }),
      ),
    ).toBe("≈ 1s");
  });
});

describe("action guards", () => {
  it("allows resume, pause and cancel only in the right states", () => {
    expect(canResume(makeDownload({ status: "paused" }))).toBe(true);
    expect(canResume(makeDownload({ status: "error" }))).toBe(true);
    expect(canResume(makeDownload({ status: "downloading" }))).toBe(false);
    expect(canResume(makeDownload({ status: "paused", needsReview: true }))).toBe(
      false,
    );
    expect(canPause(downloading)).toBe(true);
    expect(canPause(makeDownload({ status: "paused" }))).toBe(false);
    expect(canCancel(downloading)).toBe(true);
    expect(canCancel(makeDownload({ status: "complete" }))).toBe(false);
    expect(canCancel(makeDownload({ status: "cancelled" }))).toBe(false);
  });
});

describe("detailFacts", () => {
  it("builds the legacy fact list for a running download", () => {
    const facts = detailFacts(downloading);
    expect(facts).toEqual([
      { label: "Status", value: "Receiving data…" },
      { label: "File size", value: "1.0 KB" },
      { label: "Downloaded", value: "512 B (50.00%)" },
      { label: "Transfer rate", value: "2.0 KB/sec" },
      { label: "Time left", value: "1s" },
      { label: "Resume capability", value: "Yes" },
    ]);
  });

  it("adds duration and schedule rows, and explains an unknown resume flag", () => {
    const facts = detailFacts(
      makeDownload({
        status: "scheduled",
        duration: 125,
        scheduledAt: "2026-11-14T10:00:00Z",
        resumable: null,
        notice: "Waiting for a queue slot.",
      }),
    );
    expect(facts.map((fact) => fact.label)).toEqual([
      "Status",
      "Video duration",
      "File size",
      "Downloaded",
      "Transfer rate",
      "Time left",
      "Resume capability",
      "Start at",
      "Note",
    ]);
    expect(facts[1]?.value).toBe("2 min 5 sec");
    expect(facts[6]?.value).toBe("Checking…");
    expect(facts[7]?.value).toBeTruthy();
  });
});

describe("segments", () => {
  it("prefers the engine report and falls back to one bar", () => {
    const engine = makeDownload({
      connectionsProgress: [
        { start: 0, end: 99, downloaded: 50, total: 100, status: "downloading" },
      ],
    });
    expect(segmentsOf(engine)).toHaveLength(1);
    expect(
      segmentsOf(makeDownload({ downloaded: 0, total: 0, status: "queued" })),
    ).toEqual([]);
    expect(segmentsOf(downloading)[0]).toMatchObject({
      downloaded: 512,
      total: 1024,
    });
  });

  it("describes paired tracks, HLS streams and plain connections", () => {
    expect(segmentCaption(makeDownload({ transferKind: "paired" }))).toBe(
      "Media tracks",
    );
    expect(segmentCaption(makeDownload({ kind: "hls" }))).toBe(
      "HLS download streams",
    );
    expect(segmentCaption(downloading)).toBe(
      "Start positions and download progress by connections",
    );
    expect(segmentLabel(makeDownload({ transferKind: "paired" }), 0)).toBe("Video");
    expect(segmentLabel(makeDownload({ transferKind: "paired" }), 1)).toBe("Audio");
    expect(segmentLabel(downloading, 2)).toBe("3");
    expect(segmentNote(makeDownload({ transferKind: "paired" }), 2)).toBe(
      "2 tracks (Video + Audio)",
    );
    expect(segmentNote(downloading, 4)).toBe("4 connection(s)");
    expect(segmentNote(downloading, 1)).toBe("1 connection");
  });

  it("marks finished segments and falls back to the download status", () => {
    expect(
      segmentStatus({ start: 0, end: 1, downloaded: 1, total: 1, status: "completed" }, "downloading"),
    ).toBe("Completed.");
    expect(
      segmentStatus({ start: 0, end: 1, downloaded: 0, total: 1, status: "" }, "downloading"),
    ).toBe("Receiving data…");
    expect(
      segmentStatus({ start: 0, end: 1, downloaded: 0, total: 1, status: "" }, "paused"),
    ).toBe("Stopped");
  });
});

describe("errors", () => {
  it("classifies the three actionable failures", () => {
    expect(errorKind("FFmpeg is required")).toBe("ffmpeg");
    expect(errorKind("yt-dlp exited with code 1")).toBe("yt-dlp");
    expect(errorKind("HTTP 403: link expired")).toBe("expired");
    expect(errorKind("socket closed")).toBe("other");
    expect(errorKind(null)).toBe("other");
  });

  it("titles the card", () => {
    expect(errorTitle("ffmpeg")).toBe("FFmpeg Assembly Required");
    expect(errorTitle("yt-dlp")).toBe("YouTube Media Tool Required");
    expect(errorTitle("expired")).toBe("Download Link Expired");
    expect(errorTitle("other")).toBe("Download Interrupted");
  });
});

describe("formatWhen", () => {
  it("formats a valid timestamp and drops an invalid one", () => {
    expect(formatWhen(null)).toBe("");
    expect(formatWhen("not a date")).toBe("");
    expect(formatWhen("2026-11-14T10:00:00Z")).not.toBe("");
  });
});
