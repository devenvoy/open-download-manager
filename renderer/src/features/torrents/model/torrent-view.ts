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
      ? includedIndices(files)
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

export function numberValue(value: unknown, fallback = 0): number {
  if (value === "" || value === null || value === undefined) return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function validateSettingsInput(settings: {
  listenPort?: unknown;
  maxPeersPerTorrent?: unknown;
  downloadLimitKbps?: unknown;
  uploadLimitKbps?: unknown;
  seedRatioLimit?: unknown;
  seedTimeLimitMinutes?: unknown;
  downloadDirectory?: string | null;
}): string | null {
  const port = numberValue(settings.listenPort, 0);
  if (!Number.isInteger(port) || port < 0 || port > 65_535) {
    return "Listen port must be a whole number between 0 and 65535.";
  }
  const peers = numberValue(settings.maxPeersPerTorrent, 0);
  if (!Number.isInteger(peers) || peers < 1 || peers > 2_000) {
    return "Max peers per torrent must be between 1 and 2000.";
  }
  const downloadLimit = numberValue(settings.downloadLimitKbps, 0);
  const uploadLimit = numberValue(settings.uploadLimitKbps, 0);
  if (!Number.isInteger(downloadLimit) || downloadLimit < 0) {
    return "The download limit must be 0 or a positive whole number.";
  }
  if (!Number.isInteger(uploadLimit) || uploadLimit < 0) {
    return "The upload limit must be 0 or a positive whole number.";
  }
  const ratio = numberValue(settings.seedRatioLimit, 0);
  if (!Number.isFinite(ratio) || ratio < 0 || ratio > 1_000) {
    return "Seed ratio limit must be between 0 and 1000.";
  }
  const seedTime = numberValue(settings.seedTimeLimitMinutes, 0);
  if (!Number.isInteger(seedTime) || seedTime < 0 || seedTime > 525_600) {
    return "Seed time limit must be between 0 and 525600 minutes.";
  }
  const directory = String(settings.downloadDirectory ?? "").trim();
  if (directory && !directory.startsWith("/") && !/^[a-zA-Z]:[\\/]/.test(directory)) {
    return "Choose an absolute torrent download folder.";
  }
  return null;
}

/**
 * The identity of a torrent.
 *
 * The engine hands the same id to the next torrent once the list is empty, so
 * an id alone cannot tell two downloads apart. The info hash can.
 */
export function torrentKey(
  torrent: { id: number; infoHash?: string } | null | undefined,
): string {
  if (!torrent) return "";
  return torrent.infoHash || `id:${torrent.id}`;
}

/**
 * Finds the torrent ids whose cached state belongs to another download.
 *
 * The engine reuses an id after a torrent is deleted, so a cache entry can
 * belong to a torrent that no longer exists or to a different download that
 * took its place. Such an entry must be dropped instead of being shown for
 * the wrong download.
 */
export function staleTorrentIds(
  knownHashes: ReadonlyMap<number, string>,
  torrents: TorrentListItem[] | null | undefined,
): Set<number> {
  const list = Array.isArray(torrents) ? torrents : [];
  const stale = new Set<number>();
  for (const [id, hash] of knownHashes) {
    const torrent = list.find((item) => item.id === id);
    if (!torrent || torrent.infoHash !== hash) stale.add(id);
  }
  return stale;
}

/**
 * Turns an engine or IPC error into a message a user can act on.
 *
 * The engine reports internal wording such as "torrent is already live",
 * which means nothing to someone who just pressed a button.
 */
export function friendlyError(error: unknown): string {
  const raw =
    typeof (error as { message?: unknown })?.message === "string"
      ? (error as { message: string }).message
      : error === undefined || error === null
        ? ""
        : String(error);
  const text = raw.toLowerCase();
  if (!text) return "The action could not be completed.";
  if (text.includes("already live")) return "This torrent is already running.";
  if (text.includes("not paused") || text.includes("cannot pause")) {
    return "This torrent is not running.";
  }
  if (text.includes("torrent is not resolved") || text.includes("metadata")) {
    return "The torrent metadata is not ready yet. Wait a moment and try again.";
  }
  if (text.includes("initializing") || text.includes("starting")) {
    return "The torrent is still starting up. Try again in a moment.";
  }
  if (text.includes("file exists") || text.includes("already exists")) {
    return "A file with that name already exists in the download folder.";
  }
  if (text.includes("no space") || text.includes("disk")) {
    return "There is not enough space left on the disk.";
  }
  if (text.includes("not registered") || text.includes("unsupported")) {
    return "That magnet link or torrent file is not supported.";
  }
  if (text.includes("permission") || text.includes("access denied")) {
    return "The app is not allowed to write to that folder.";
  }
  if (text.includes("engine") && text.includes("not")) {
    return "The torrent engine is not running. Restart it in the settings.";
  }
  return raw;
}
