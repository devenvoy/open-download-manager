/** Download domain DTOs — mirrors src-tauri/src/domain/download/** and
 *  src-tauri/src/domain/snapshot/download_summary.rs. */

/** `DownloadStatus` — src-tauri/src/domain/download/status.rs */
export type DownloadStatus =
  | "queued"
  | "scheduled"
  | "paused"
  | "cancelled"
  | "downloading"
  | "probing"
  | "assembling"
  | "complete"
  | "error"
  | "skipped";

/** `DownloadCategory` — src-tauri/src/domain/category.rs */
export type DownloadCategory =
  | "Video"
  | "Audio"
  | "Archives"
  | "Documents"
  | "Programs"
  | "Other";

export const DOWNLOAD_CATEGORIES: readonly DownloadCategory[] = [
  "Video",
  "Audio",
  "Archives",
  "Documents",
  "Programs",
  "Other",
];

/** `DownloadAction` — src-tauri/src/domain/download/action.rs */
export type DownloadActionKind =
  | "pause"
  | "resume"
  | "retry"
  | "cancel"
  | "remove"
  | "open"
  | "reveal"
  | "openWith";

/** `DownloadSegmentProgress` — src-tauri/src/domain/download/record.rs */
export interface DownloadSegmentProgress {
  start: number;
  end: number;
  downloaded: number;
  total: number;
  status: string;
}

/** `ActionStatus` — src-tauri/src/domain/completion.rs */
export type ActionStatus = "success" | "failed" | "cancelled" | "skipped";

/** `ActionLedgerEntry` — src-tauri/src/domain/completion.rs */
export interface ActionLedgerEntry {
  actionKey: string;
  actionType: string;
  status: ActionStatus;
  message?: string;
  executedAt: string;
}

/**
 * `CompletionAction` — src-tauri/src/domain/completion.rs
 *
 * The enum is tagged by `type`. Serde's enum `rename_all` renames the
 * variants, not the struct-variant fields, so those stay snake_case.
 */
export type CompletionAction =
  | { type: "notification" }
  | { type: "playSound"; sound_file?: string | null }
  | { type: "openFile" }
  | { type: "openFolder" }
  | { type: "antivirusScan"; executable: string; arguments: string[] }
  | { type: "executeCommand"; executable: string; arguments: string[] }
  | {
      type: "powerAction";
      kind: "Shutdown" | "Sleep" | "Hibernate";
      confirmed: boolean;
    };

/** `DownloadSummary` — src-tauri/src/domain/snapshot/download_summary.rs */
export interface DownloadSummary {
  id: string;
  url: string;
  filename: string;
  status: DownloadStatus;
  downloaded: number;
  total: number | null;
  speed: number;
  createdAt: string;
  needsReview: boolean;
  scheduledAt: string | null;
  queueId: string | null;
  description: string | null;
  category: DownloadCategory | null;
  directory: string | null;
  path: string | null;
  tempDirectory: string | null;
  lastTry: string | null;
  error: string | null;
  notice: string | null;
  estimated: boolean;
  kind: string | null;
  resumable: boolean | null;
  duration: number | null;
  connections: number;
  connectionsProgress: DownloadSegmentProgress[];
  transferKind: string | null;
  referer: string | null;
  completionActions?: CompletionAction[] | null;
  actionLedger?: ActionLedgerEntry[];
}

/** `FileConflictPolicy` — src-tauri/src/domain/download/conflict.rs */
export type FileConflictPolicy = "ask" | "overwrite" | "rename" | "skip" | "cancel";

/** `ConflictResolutionOutcome` — src-tauri/src/domain/download/conflict.rs */
export type ConflictResolutionOutcome =
  | { type: "resolved"; path: string; policy: FileConflictPolicy }
  | {
      type: "conflict";
      existingPath: string;
      suggestedPath: string;
      candidates: FileConflictPolicy[];
    }
  | { type: "skipped"; path: string }
  | { type: "cancelled"; path: string };

/** `DownloadCredentials` — src-tauri/src/domain/download/request.rs */
export interface DownloadCredentials {
  username: string;
  password?: string | null;
}

/** `DownloadRequest` — src-tauri/src/domain/download/request.rs */
export interface DownloadRequest {
  urls: string[];
  filename?: string;
  directory?: string;
  tempDirectory?: string;
  category?: DownloadCategory;
  description?: string;
  queueOnly?: boolean;
  reviewOnly?: boolean;
  scheduledAt?: string;
  queueId?: string;
  audioUrl?: string;
  kind?: string;
  sabr?: unknown;
  reuseActive?: boolean;
  referer?: string;
  youtubeFormat?: string;
  conflictPolicy?: FileConflictPolicy;
  credentials?: DownloadCredentials;
  requestHeaders?: unknown;
  completionActions?: CompletionAction[];
}

/** `ReviewDownloadRequest` — src-tauri/src/domain/download/review_request.rs */
export interface ReviewDownloadRequest {
  id: string;
  filename?: string | null;
  directory?: string | null;
  tempDirectory?: string | null;
  category?: DownloadCategory | null;
  description?: string | null;
  queueOnly?: boolean | null;
  scheduledAt?: string | null;
  conflictPolicy?: FileConflictPolicy | null;
  credentials?: DownloadCredentials | null;
}
