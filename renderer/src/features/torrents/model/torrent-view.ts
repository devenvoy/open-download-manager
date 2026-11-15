import { formatBytes, formatSpeed } from "@/lib/format";
import { type TorrentFileItem, type TorrentListItem } from "@/types/ipc";

/**
 * Pure view-model helpers for the Torrents window.
 *
 * Ported 1:1 from `reference/legacy-renderer/torrents-view.js`, where the
 * same module ran in WebKit and under `node:test`. No DOM access lives here,
 * so every rule below is unit-testable.
 */

export type TorrentStateKey =
  | "downloading"
  | "waiting"
  | "seeding"
  | "idle"
  | "paused"
  | "metadata"
  | "error"
  | "unknown";

const ACTIVE_STATES: ReadonlySet<TorrentStateKey> = new Set([
  "downloading",
  "metadata",
  "waiting",
]);

const STATE_LABELS: Record<TorrentStateKey, string> = {
  downloading: "Downloading",
  waiting: "Waiting for peers",
  seeding: "Seeding",
  idle: "Nothing selected",
  paused: "Paused",
  metadata: "Fetching metadata",
  error: "Error",
  unknown: "Unknown",
};

export function stateKey(
  torrent: Partial<TorrentListItem> | null | undefined,
): TorrentStateKey {
  if (!torrent) return "unknown";
  const raw = String(torrent.state ?? "").toLowerCase();
  if (raw === "error") return "error";
  if (raw === "paused") return "paused";
  if (raw === "initializing") return "metadata";
  if (raw !== "live") return "unknown";

  // The engine reports a torrent without selected files as finished, but that
  // means "nothing is wanted", not "everything is downloaded".
  const done = Number(torrent.progressBytes) || 0;
  if (!torrent.finished) {
    // A running torrent that has not received a byte and cannot see a single
    // peer with the data is waiting for the swarm. It is not broken, and
    // saying "downloading" would be a lie.
    const seeds = Number(torrent.seeds) || 0;
    const speed = Number(torrent.downloadSpeed) || 0;
    if (done === 0 && seeds === 0 && speed === 0) return "waiting";
    return "downloading";
  }
  return done > 0 ? "seeding" : "idle";
}

export function stateLabel(
  torrent: Partial<TorrentListItem> | null | undefined,
): string {
  return STATE_LABELS[stateKey(torrent)];
}

export function stateShortLabel(
  torrent: Partial<TorrentListItem> | null | undefined,
): string {
  const key = stateKey(torrent);
  if (key === "metadata") return "Metadata";
  if (key === "waiting") return "Waiting";
  return stateLabel(torrent);
}

export function progressPercent(
  torrent: Partial<TorrentListItem> | null | undefined,
): number {
  if (!torrent) return 0;
  const total = Number(torrent.totalBytes) || 0;
  const done = Number(torrent.progressBytes) || 0;
  if (!total) return 0;
  if (done >= total) return 100;
  return Math.min(100, Math.max(0, (done / total) * 100));
}

export function formatRatio(value: number | null | undefined): string {
  const ratio = Number(value);
  return Number.isFinite(ratio) && ratio > 0 ? ratio.toFixed(2) : "0.00";
}

export function formatEta(seconds: number | null | undefined): string {
  const value = Number(seconds);
  if (!Number.isFinite(value) || value <= 0) return "—";
  if (value < 60) return `${Math.ceil(value)}s`;
  if (value < 3600) {
    return `${Math.floor(value / 60)}m ${Math.ceil(value % 60)}s`;
  }
  if (value < 86_400) {
    return `${Math.floor(value / 3600)}h ${Math.floor((value % 3600) / 60)}m`;
  }
  return `${Math.floor(value / 86_400)}d ${Math.floor((value % 86_400) / 3600)}h`;
}

export function rowSubtitle(torrent: TorrentListItem): string {
  const percent = progressPercent(torrent);
  const key = stateKey(torrent);
  if (key === "downloading") {
    const eta = formatEta(torrent.timeRemainingSecs);
    return `${percent.toFixed(1)}% · ↓ ${formatSpeed(torrent.downloadSpeed)}${
      eta === "—" ? "" : ` · ${eta} left`
    }`;
  }
  if (key === "seeding") {
    const up = torrent.uploadedBytes ? ` · Uploaded ${formatBytes(torrent.uploadedBytes)}` : "";
    const speed = torrent.uploadSpeed ? ` · ↑ ${formatSpeed(torrent.uploadSpeed)}` : "";
    const ratioPart = torrent.ratio && torrent.ratio > 0.01 ? ` · Ratio ${formatRatio(torrent.ratio)}` : "";
    return `Seeding${speed}${up}${ratioPart}`;
  }
  if (key === "waiting") {
    return "No peer with the files yet — still looking";
  }
  if (key === "paused") {
    return percent >= 100 ? "Completed" : `Paused at ${percent.toFixed(1)}%`;
  }
  if (key === "metadata") return "Fetching metadata…";
  if (key === "error") return torrent.error || "Transfer failed";
  return "—";
}

export interface TorrentActions {
  pause: boolean;
  resume: boolean;
  recheck: boolean;
  forget: boolean;
  delete: boolean;
}

export function availableActions(
  torrent: Partial<TorrentListItem> | null | undefined,
): TorrentActions {
  const key = stateKey(torrent);
  const exists = Boolean(torrent);
  return {
    pause: exists && ACTIVE_STATES.has(key),
    resume: exists && (key === "paused" || key === "error"),
    recheck: exists && key !== "metadata",
    forget: exists,
    delete: exists,
  };
}

export interface TorrentSummary {
  count: number;
  downloading: number;
  waiting: number;
  seeding: number;
  paused: number;
  downloadSpeed: number;
  uploadSpeed: number;
}

export function summarize(
  torrents: TorrentListItem[] | null | undefined,
): TorrentSummary {  const list = Array.isArray(torrents) ? torrents : [];
  let downloading = 0;
  let waiting = 0;
  let seeding = 0;
  let paused = 0;
  let downloadSpeed = 0;
  let uploadSpeed = 0;

  for (const torrent of list) {
    const key = stateKey(torrent);
    if (key === "downloading" || key === "metadata") downloading += 1;
    else if (key === "waiting") waiting += 1;
    else if (key === "seeding") seeding += 1;
    else if (key === "paused") paused += 1;
    downloadSpeed += Number(torrent.downloadSpeed) || 0;
    uploadSpeed += Number(torrent.uploadSpeed) || 0;
  }

  return {
    count: list.length,
    downloading,
    waiting,
    seeding,
    paused,
    downloadSpeed,
    uploadSpeed,
  };
}

/** One line for the toolbar: what the whole list is doing. */
export function summaryText(summary: TorrentSummary): string {
  if (summary.count === 0) return "No torrents";
  const parts: string[] = [];
  if (summary.downloading) parts.push(`${summary.downloading} downloading`);
  if (summary.waiting) parts.push(`${summary.waiting} waiting`);
  if (summary.seeding) parts.push(`${summary.seeding} seeding`);
  if (summary.paused) parts.push(`${summary.paused} paused`);
  if (parts.length === 0) parts.push(`${summary.count} total`);
  return parts.join(", ");
}

export function includedIndices(files: TorrentFileItem[] | null | undefined): number[] {
  if (!Array.isArray(files)) return [];
  return files
    .filter((file) => file && file.included)
    .map((file) => Number(file.index))
    .filter((index) => Number.isInteger(index) && index >= 0)
    .sort((left, right) => left - right);
}

export function defaultFileSelection(
  files: TorrentFileItem[] | null | undefined,
  isReview = false,
): number[] {
  const reported = includedIndices(files);
  if (reported.length) return reported;
  if (isReview && Array.isArray(files)) {
    return files
      .map((file) => Number(file.index))
      .filter((index) => Number.isInteger(index) && index >= 0);
  }
  return [];
}

export interface ReviewPlan {
  files: number[];
  action: "pause" | "resume";
  error: string | null;
}

/**
 * Builds the plan for confirming a torrent review.
 *
 * The torrent was added with an empty file selection, so nothing is
 * transferred until the user confirms. This returns the file indices to
 * enable and the engine action to run afterwards.
 */
export function reviewPlan(
  files: TorrentFileItem[] | null | undefined,
  selection: ReadonlySet<number> | number[] | null | undefined,
  startPaused: boolean,
): ReviewPlan {
  const action = startPaused ? "pause" : "resume";

  if (!Array.isArray(files) || files.length === 0) {
    return {
      files: [],
      action,
      error: "The file list is not ready yet. Wait for metadata, then try again.",
    };
  }

  const picked =
    selection === null || selection === undefined
      ? defaultFileSelection(files, true)
      : [...selection];

  const selected = picked
    .map((index) => Number(index))
    .filter((index) => Number.isInteger(index) && index >= 0)
    .sort((left, right) => left - right);

  if (selected.length === 0) {
    return { files: [], action, error: "Select at least one file to download." };
  }

  return { files: selected, action, error: null };
}

export function fileProgressPercent(
  file: Partial<TorrentFileItem> | null | undefined,
): number {
  if (!file) return 0;
  const total = Number(file.length) || 0;
  const done = Number(file.progressBytes) || 0;
  if (!total) return 0;
  return Math.min(100, Math.max(0, (done / total) * 100));
}

export {
  friendlyError,
  numberValue,
  staleTorrentIds,
  torrentKey,
  validateSettingsInput,
} from "./torrent-helpers";
