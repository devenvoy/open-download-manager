import { type DownloadCategory, type DownloadRequest } from "@/types/ipc";

/** Pure rules for the Add-download window, ported from `add-form` handlers. */

export function isYouTubePlaylistUrl(text: string): boolean {
  const trimmed = text.trim();
  if (!/^https?:\/\//i.test(trimmed)) return false;
  return /(?:youtube\.com\/(?:playlist\?list=|watch\?.*[?&]list=)|youtu\.be\/.*[?&]list=)([\w-]{10,64})/i.test(
    trimmed,
  );
}

export function isYouTubeVideoUrl(text: string): boolean {
  const trimmed = text.trim();
  if (!/^https?:\/\//i.test(trimmed)) return false;
  if (isYouTubePlaylistUrl(trimmed)) return false;
  return /(?:youtube\.com\/(?:watch\?.*v=|shorts\/|embed\/)|youtu\.be\/)([\w-]{11})/i.test(
    trimmed,
  );
}

export function splitUrls(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((url) => url.trim())
    .filter(Boolean);
}

const ALLOWED_PROTOCOLS = new Set(["http:", "https:", "ftp:"]);

/** Validates the whole form and returns the first message a user can act on. */
export function validateAddForm(input: {
  urls: string[];
  filename: string;
  scheduledAt: string;
  now?: number;
}): string | null {
  if (input.urls.length === 0) return "Enter at least one download address.";
  for (const url of input.urls) {
    let parsed: URL;
    try {
      parsed = new URL(url);
    } catch {
      return `Invalid address: ${url}`;
    }
    if (!ALLOWED_PROTOCOLS.has(parsed.protocol)) {
      return "Download addresses must begin with http://, https://, or ftp://.";
    }
  }
  if (input.filename && input.urls.length !== 1) {
    return "A custom file name can only be used for a single address.";
  }
  if (input.scheduledAt) {
    const when = new Date(input.scheduledAt).getTime();
    if (Number.isNaN(when)) return "Choose a valid start time.";
    if (when <= (input.now ?? Date.now())) {
      return "Choose a future start time or leave Start at empty.";
    }
  }
  return null;
}

export interface AddFormValues {
  urlsText: string;
  filename: string;
  directory: string;
  category: DownloadCategory | "";
  description: string;
  queueId: string;
  scheduledAt: string;
  authEnabled: boolean;
  authUser: string;
  authPass: string;
  youtubeFormat: string | null;
}

export const EMPTY_ADD_FORM: AddFormValues = {
  urlsText: "",
  filename: "",
  directory: "",
  category: "",
  description: "",
  queueId: "default",
  scheduledAt: "",
  authEnabled: false,
  authUser: "",
  authPass: "",
  youtubeFormat: null,
};

/**
 * Builds the request the Rust side accepts. `DownloadRequest` denies unknown
 * fields, so every optional key is added only when it has a value.
 */
export function buildAddRequest(
  values: AddFormValues,
  queueOnly: boolean,
): DownloadRequest {
  const request: DownloadRequest = {
    urls: splitUrls(values.urlsText),
    queueOnly,
  };
  if (values.filename.trim()) request.filename = values.filename.trim();
  if (values.directory.trim()) request.directory = values.directory.trim();
  if (values.category) request.category = values.category;
  if (values.description.trim()) request.description = values.description.trim();
  if (values.queueId) request.queueId = values.queueId;
  if (values.scheduledAt) {
    request.scheduledAt = new Date(values.scheduledAt).toISOString();
  }
  if (values.youtubeFormat) request.youtubeFormat = values.youtubeFormat;
  if (values.authEnabled && values.authUser.trim()) {
    request.credentials = {
      username: values.authUser.trim(),
      password: values.authPass || null,
    };
  }
  return request;
}

export function formatVideoDuration(seconds: number | null | undefined): string {
  if (!seconds || seconds <= 0) return "";
  return `${Math.floor(seconds / 60)}m ${Math.floor(seconds % 60)}s`;
}

export function formatPlaylistDuration(seconds: number | null | undefined): string {
  if (!seconds || seconds <= 0) return "";
  return `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
}

export function formatEstimatedSize(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return "";
  return `~${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** The label the Format dialog shows for one choice. */
export function formatChoiceLabel(choice: {
  format: string;
  quality: string;
  audioState: string;
  estimatedSize: number;
}): string {
  const size = formatEstimatedSize(choice.estimatedSize);
  return `${choice.format} ${choice.quality} - ${choice.audioState}${size ? ` (${size})` : ""}`;
}

/** The filename and category the legacy dialog applied when a format is picked. */
export function applyFormatChoice(choice: {
  title: string;
  outputFormat: string;
  format: string;
  quality: string;
  youtubeFormat: string;
}, currentUrl: string): {
  filename: string;
  category: DownloadCategory;
  label: string;
} {
  const ext = choice.outputFormat || (choice.format === "MP3" ? "mp3" : "mp4");
  let title = choice.title;
  if (!title || title.trim().toLowerCase() === "video") {
    try {
      const url = new URL(currentUrl);
      const host = url.hostname.replace(/^www\./, "").replace(/\./g, "_");
      title = host ? `download_${host}` : "download";
    } catch {
      title = "download";
    }
  }
  return {
    filename: `${title}.${ext}`,
    category: ext === "mp3" ? "Audio" : "Video",
    label: `Selected: ${choice.quality || choice.format} (${ext.toUpperCase()})`,
  };
}
