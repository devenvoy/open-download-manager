import { type TorrentListItem } from "@/types/ipc";

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
 */
export function torrentKey(
  torrent: { id: number; infoHash?: string } | null | undefined,
): string {
  if (!torrent) return "";
  return torrent.infoHash || `id:${torrent.id}`;
}

/**
 * Finds the torrent ids whose cached state belongs to another download.
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
