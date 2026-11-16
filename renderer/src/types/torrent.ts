/**
 * Torrent DTOs — mirrors src-tauri/src/domain/torrent/**.
 *
 * The torrent subsystem is independent from the download history: these types
 * never appear inside `Snapshot`.
 */

/** `TorrentEngineStatus` — `state` is `disabled | starting | ready | failed | stopped`. */
export interface TorrentEngineStatus {
  available: boolean;
  enabled: boolean;
  state: string;
  message: string | null;
  listenPort: number | null;
  dhtEnabled: boolean;
  restartRequired: boolean;
}

/** `TorrentFileItem` */
export interface TorrentFileItem {
  index: number;
  name: string;
  length: number;
  included: boolean;
  priority: number;
  progressBytes: number;
}

/** `TorrentListItem` — `state` is `initializing | live | paused | error | unknown`. */
export interface TorrentListItem {
  id: number;
  infoHash: string;
  name: string;
  outputFolder: string;
  state: string;
  error: string | null;
  progressBytes: number;
  uploadedBytes: number;
  totalBytes: number;
  finished: boolean;
  downloadSpeed: number;
  uploadSpeed: number;
  timeRemainingSecs: number | null;
  seeds: number;
  leechers: number;
  peers: number;
  ratio: number;
  addedOn: number | null;
  trackers: string[];
  files: TorrentFileItem[];
}

/** `TorrentAddOptions` — serde default, so every field is optional. */
export interface TorrentAddOptions {
  outputFolder?: string | null;
  startPaused?: boolean;
  onlyFiles?: number[] | null;
  review?: boolean;
  initialPeers?: string[] | null;
}

/** `TorrentAddResult` */
export interface TorrentAddResult {
  id: number | null;
  infoHash: string;
  name: string;
  alreadyManaged: boolean;
  outputFolder: string;
}

/** `TorrentReviewRequest` */
export interface TorrentReviewRequest {
  id: number;
  name: string;
  infoHash: string;
}

/** `TorrentTrackerStatus` — `state` is `not_contacted | updating | working | error | disabled`. */
export interface TorrentTrackerStatus {
  url: string;
  state: string;
  seeders: number | null;
  leechers: number | null;
  peersReturned: number | null;
  lastError: string | null;
}

/** `TorrentPeerItem` */
export interface TorrentPeerItem {
  address: string;
  client: string | null;
  downloadSpeed: number;
  uploadSpeed: number;
  progressPercent: number;
  flags: string;
  connectionType: string;
  fetchedBytes: number;
  uploadedBytes: number;
}

/** `TorrentPiecesInfo` */
export interface TorrentPiecesInfo {
  totalPieces: number;
  pieceLength: number;
  completedPieces: number;
  pieceStates: number[];
}

/** `TorrentAction` (snake_case on the wire). */
export type TorrentActionKind = "pause" | "resume" | "recheck" | "forget" | "delete";

/** `TorrentSettings` — stored separately from the manager settings. */
export interface TorrentSettings {
  version: number;
  enabled: boolean;
  downloadDirectory: string | null;
  listenPort: number;
  enableDht: boolean;
  enableUpnp: boolean;
  enableUtp: boolean;
  downloadLimitKbps: number;
  uploadLimitKbps: number;
  maxPeersPerTorrent: number;
  seedRatioLimit: number;
  seedTimeLimitMinutes: number;
  startPaused: boolean;
  notifyOnComplete: boolean;
}
