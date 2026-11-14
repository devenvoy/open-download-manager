/** Media DTOs — mirrors src-tauri/src/domain/media/**,
 *  src-tauri/src/domain/media_tools.rs and src-tauri/src/domain/protocol/mms/asx.rs. */

/** `PlaylistEntry` — src-tauri/src/domain/media/playlist.rs */
export interface PlaylistEntry {
  id: string;
  title: string;
  duration: number | null;
  url: string;
  thumbnail: string | null;
  channel: string | null;
  index: number;
}

/** `PlaylistMetadata` — src-tauri/src/domain/media/playlist.rs */
export interface PlaylistMetadata {
  id: string;
  title: string;
  description: string | null;
  uploader: string | null;
  webpageUrl: string;
  entryCount: number;
  entries: PlaylistEntry[];
}

/** `YouTubeFormatChoice` — src-tauri/src/domain/media/youtube/types.rs */
export interface YouTubeFormatChoice {
  url: string;
  provider: string;
  youtubeFormat: string;
  format: string;
  quality: string;
  fps: number;
  audioState: string;
  title: string;
  outputFormat: string;
  estimatedSize: number;
  duration: number;
  audioUrl?: string | null;
}

/** `MediaToolType` — src-tauri/src/domain/media_tools.rs (kebab-case) */
export type MediaToolType = "ffmpeg" | "yt-dlp";

/** `ToolResolutionSource` — src-tauri/src/domain/media_tools.rs (kebab-case) */
export type ToolResolutionSource =
  | "environment-override"
  | "bundled-sidecar"
  | "managed-user-data"
  | "system-path";

/** `MediaToolStatus` — src-tauri/src/domain/media_tools.rs */
export interface MediaToolStatus {
  toolType: MediaToolType;
  isAvailable: boolean;
  path: string | null;
  source: ToolResolutionSource | null;
}

/** `AsxEntry` — src-tauri/src/domain/protocol/mms/asx.rs */
export interface AsxEntry {
  title: string | null;
  href: string;
  scheme: string | null;
  isSupportedFallback: boolean;
  isMms: boolean;
}

/** `AsxPlaylist` — src-tauri/src/domain/protocol/mms/asx.rs */
export interface AsxPlaylist {
  title: string | null;
  entries: AsxEntry[];
}
