import { formatBytes, formatDuration } from "@/lib/format";
import {
  type DownloadSegmentProgress,
  type DownloadStatus,
  type DownloadSummary,
} from "@/types/ipc";

/** Pure view rules for the progress window, ported from formatters.js. */

export type DisplayStatus =
  | "queued"
  | "scheduled"
  | "paused"
  | "downloading"
  | "connecting"
  | "assembling"
  | "completed"
  | "failed"
  | "cancelled"
  | "skipped";

const STATUS_MAP: Partial<Record<DownloadStatus, DisplayStatus>> = {
  queued: "queued",
  scheduled: "scheduled",
  paused: "paused",
  downloading: "downloading",
  probing: "connecting",
  assembling: "assembling",
  complete: "completed",
  error: "failed",
  cancelled: "cancelled",
  skipped: "skipped",
};

export function statusOf(download: DownloadSummary | null | undefined): DisplayStatus {
  if (!download) return "queued";
  return STATUS_MAP[download.status] ?? "queued";
}

const STATUS_LABELS: Record<DisplayStatus, string> = {
  queued: "Queued",
  scheduled: "Scheduled",
  paused: "Stopped",
  downloading: "Receiving data…",
  connecting: "Connecting…",
  assembling: "Finalizing…",
  completed: "Complete",
  failed: "Error",
  cancelled: "Cancelled",
  skipped: "Skipped",
};

export function statusLabel(status: DisplayStatus): string {
  return STATUS_LABELS[status];
}

export function percentage(download: DownloadSummary | null | undefined): number | null {
  if (!download) return null;
  if (statusOf(download) === "completed") return 100;
  const total = Number(download.total) || 0;
  if (!total) return null;
  const done = Number(download.downloaded) || 0;
  return Math.min(100, (done / total) * 100);
}

export function sizeLabel(download: DownloadSummary): string {
  const status = statusOf(download);
  if (status === "connecting" || status === "queued") return "Connecting…";
  if (download.total) {
    return `${download.estimated ? "≈ " : ""}${formatBytes(download.total)}`;
  }
  return download.kind === "hls"
    ? "Calculating from segments…"
    : "Not supplied by server";
}

export function timeLeft(download: DownloadSummary): string {
  const total = Number(download.total) || 0;
  const speed = Number(download.speed) || 0;
  if (!total || !speed) return "";
  const seconds = Math.max(0, Math.ceil((total - (Number(download.downloaded) || 0)) / speed));
  const prefix = !download.total || download.estimated ? "≈ " : "";
  return `${prefix}${formatDuration(seconds)}`;
}

export function canResume(download: DownloadSummary | null | undefined): boolean {
  if (!download || download.needsReview) return false;
  return ["queued", "paused", "failed", "cancelled"].includes(statusOf(download));
}

export function canPause(download: DownloadSummary | null | undefined): boolean {
  if (!download) return false;
  return ["connecting", "downloading"].includes(statusOf(download));
}

export function canCancel(download: DownloadSummary | null | undefined): boolean {
  if (!download) return false;
  return !["completed", "cancelled"].includes(statusOf(download));
}

export function isErrorStatus(status: DisplayStatus): boolean {
  return status === "failed";
}

export interface DetailFact {
  label: string;
  value: string;
}

export function detailFacts(download: DownloadSummary): DetailFact[] {
  const status = statusOf(download);
  const percent = percentage(download);
  const facts: DetailFact[] = [
    { label: "Status", value: statusLabel(status) },
    { label: "File size", value: sizeLabel(download) },
    {
      label: "Downloaded",
      value: `${formatBytes(download.downloaded)}${
        percent !== null ? ` (${percent.toFixed(2)}%)` : ""
      }`,
    },
    {
      label: "Transfer rate",
      value: status === "downloading" ? `${formatBytes(download.speed)}/sec` : "0 B/sec",
    },
    {
      label: "Time left",
      value: status === "downloading" ? timeLeft(download) || "Estimating…" : "—",
    },
    {
      label: "Resume capability",
      value:
        download.resumable === true
          ? "Yes"
          : download.resumable === false
            ? "No"
            : "Checking…",
    },
  ];
  if (download.duration) {
    facts.splice(1, 0, {
      label: "Video duration",
      value: `${Math.floor(download.duration / 60)} min ${Math.floor(download.duration % 60)} sec`,
    });
  }
  if (download.scheduledAt && status === "scheduled") {
    facts.push({ label: "Start at", value: formatWhen(download.scheduledAt) });
  }
  if (download.notice) facts.push({ label: "Note", value: download.notice });
  return facts;
}

export function formatWhen(value: string | null | undefined): string {
  if (!value) return "";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime())
    ? ""
    : parsed.toLocaleString([], { dateStyle: "short", timeStyle: "short" });
}

/**
 * The connection bars. The engine sends one entry per worker; an older record
 * without them still gets a single bar covering the whole transfer.
 */
export function segmentsOf(download: DownloadSummary): DownloadSegmentProgress[] {
  if (Array.isArray(download.connectionsProgress) && download.connectionsProgress.length > 0) {
    return download.connectionsProgress;
  }
  const status = statusOf(download);
  if (!(download.downloaded > 0 || status === "downloading")) return [];
  const total = Number(download.total) || download.downloaded;
  return [
    {
      start: 0,
      end: total > 0 ? total - 1 : download.downloaded,
      downloaded: download.downloaded,
      total,
      status: status === "downloading" ? "downloading" : status,
    },
  ];
}

export function segmentCaption(download: DownloadSummary): string {
  if (download.transferKind === "paired" || download.kind === "paired") {
    return "Media tracks";
  }
  if (download.transferKind === "hls" || download.kind === "hls") {
    return "HLS download streams";
  }
  return "Start positions and download progress by connections";
}

export function segmentNote(download: DownloadSummary, count: number): string {
  if (download.transferKind === "paired" || download.kind === "paired") {
    return count === 2 ? "2 tracks (Video + Audio)" : `${count} track(s)`;
  }
  return count > 1 ? `${count} connection(s)` : count === 1 ? "1 connection" : "";
}

export function segmentLabel(download: DownloadSummary, index: number): string {
  if (download.transferKind === "paired" || download.kind === "paired") {
    return index === 0 ? "Video" : index === 1 ? "Audio" : String(index + 1);
  }
  return String(index + 1);
}

/** The per-segment status cell, falling back to the download's own status. */
export function segmentStatus(
  segment: DownloadSegmentProgress,
  status: DisplayStatus,
): string {
  if (segment.status === "complete" || segment.status === "completed") {
    return "Completed.";
  }
  if (segment.status) return segment.status;
  return status === "downloading" ? "Receiving data…" : statusLabel(status);
}

export type ErrorKind = "ffmpeg" | "yt-dlp" | "expired" | "other";

export function errorKind(error: string | null | undefined): ErrorKind {
  const text = error ?? "";
  if (/ffmpeg/i.test(text)) return "ffmpeg";
  if (/yt-dlp/i.test(text)) return "yt-dlp";
  if (/expired|HTTP 401|HTTP 403|HTTP 410/i.test(text)) return "expired";
  return "other";
}

export function errorTitle(kind: ErrorKind): string {
  if (kind === "ffmpeg") return "FFmpeg Assembly Required";
  if (kind === "yt-dlp") return "YouTube Media Tool Required";
  if (kind === "expired") return "Download Link Expired";
  return "Download Interrupted";
}
