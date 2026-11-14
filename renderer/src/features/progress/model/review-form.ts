import { type DownloadCategory, type DownloadSummary } from "@/types/ipc";

/** The confirmation form shown before a captured download starts. */

export interface ReviewDraft {
  filename: string;
  directory: string;
  category: DownloadCategory | "";
  description: string;
  /** `<input type="datetime-local">` value, empty when not scheduled. */
  scheduledAt: string;
  authEnabled: boolean;
  authUser: string;
  authPass: string;
}

export interface ReviewPatch {
  filename?: string;
  directory?: string;
  category?: DownloadCategory;
  description?: string;
  queueOnly?: boolean;
  scheduledAt?: string | null;
  credentials?: { username: string; password: string | null } | null;
}

export function draftFromDownload(download: DownloadSummary): ReviewDraft {
  return {
    filename: download.filename ?? "",
    directory: download.directory ?? "",
    category: download.category ?? "",
    description: download.description ?? "",
    scheduledAt: "",
    authEnabled: false,
    authUser: "",
    authPass: "",
  };
}

function isAbsolutePath(value: string): boolean {
  return value.startsWith("/") || /^[A-Za-z]:[\\/]/.test(value);
}

export function validateReviewDraft(
  draft: ReviewDraft,
  now = Date.now(),
): string | null {
  if (draft.filename.length > 240) {
    return "The file name is limited to 240 characters.";
  }
  if (draft.description.length > 4_096) {
    return "The description is limited to 4096 characters.";
  }
  if (draft.directory && !isAbsolutePath(draft.directory)) {
    return "Choose an absolute save folder.";
  }
  if (draft.scheduledAt) {
    const when = new Date(draft.scheduledAt).getTime();
    if (Number.isNaN(when)) return "Choose a valid start time.";
    if (when <= now) return "Choose a future start time or leave Start at empty.";
  }
  if (draft.authEnabled && !draft.authUser.trim()) {
    return "Enter the username for basic authentication.";
  }
  return null;
}

/**
 * `review_download` applies every key it receives, so optional keys are sent
 * only when the user set a value.
 */
export function buildReviewPatch(
  draft: ReviewDraft,
  queueOnly: boolean,
): ReviewPatch {
  const patch: ReviewPatch = { queueOnly };
  if (draft.filename.trim()) patch.filename = draft.filename.trim();
  if (draft.directory.trim()) patch.directory = draft.directory.trim();
  if (draft.category) patch.category = draft.category;
  patch.description = draft.description.trim();
  patch.scheduledAt = draft.scheduledAt
    ? new Date(draft.scheduledAt).toISOString()
    : null;
  if (draft.authEnabled) {
    patch.credentials = {
      username: draft.authUser.trim(),
      password: draft.authPass || null,
    };
  }
  return patch;
}
