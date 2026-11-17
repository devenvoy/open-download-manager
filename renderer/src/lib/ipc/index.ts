import { call } from "./client";
import { chooseDirectory, chooseFile, readClipboard } from "./native";
import {
  type AdvancedIntegrationCapability,
  type CompletionAction,
  type CredentialRule,
  type DetectedProxyConfig,
  type DownloadActionKind,
  type DownloadCredentials,
  type DownloadRequest,
  type QueueDisposition,
  type QueueSchedule,
  type SettingsPatch,
  type TorrentActionKind,
  type TorrentAddOptions,
  type TorrentSettings,
} from "@/types/ipc";

/**
 * One function per command. Components never call `call` or `invoke`
 * directly; they go through a feature `api.ts` or one of these.
 */
export const ipc = {
  // Snapshot and downloads
  snapshot: () => call("snapshot"),
  updateSettings: (patch: SettingsPatch) => call("update_settings", { patch }),
  addDownload: (request: DownloadRequest) => call("add_download", { request }),
  downloadAction: (id: string, action: DownloadActionKind) =>
    call("download_action", { id, action }),
  removeCompleted: () => call("remove_completed"),
  reviewDownload: (id: string, patch: {
    filename?: string | null;
    directory?: string | null;
    tempDirectory?: string | null;
    category?: DownloadRequest["category"] | null;
    description?: string | null;
    queueOnly?: boolean | null;
    scheduledAt?: string | null;
    conflictPolicy?: DownloadRequest["conflictPolicy"] | null;
    credentials?: DownloadCredentials | null;
  }) => call("review_download", { request: { id, ...patch } }),
  checkDestinationConflict: (directory: string, filename: string) =>
    call("check_destination_conflict", { directory, filename }),
  updateCompletionActions: (id: string, actions: CompletionAction[] | null) =>
    call("update_download_completion_actions", { id, actions }),
  retryCompletionActions: (id: string) =>
    call("retry_download_actions", { id }),
  exportDownloads: () => call("export_downloads_to_file"),
  importDownloads: () => call("import_downloads_from_file"),

  // Settings
  validateDirectory: (path: string) => call("validate_directory", { path }),
  showAbout: () => call("show_about_dialog"),
  checkForUpdates: () => call("check_for_updates"),

  // Windows
  openProgressWindow: (id: string) => call("open_progress_window", { id }),
  closeAuxiliaryWindow: () => call("close_auxiliary_window"),
  resizeAuxiliaryWindow: (width?: number, height?: number) =>
    call("resize_auxiliary_window", { width, height }),
  resizeProgressWindow: (expanded: boolean) =>
    call("resize_progress_window", { expanded }),
  openOptionsWindow: () => call("open_options_window"),
  openAddWindow: (options?: { paste?: boolean; scheduled?: string }) =>
    call("open_add_window", { options }),
  openSchedulerWindow: () => call("open_scheduler_window"),
  openIntegrationWindow: () => call("open_integration_window"),
  openTorrentWindow: () => call("open_torrent_window"),
  setActiveDownloads: (count: number) =>
    call("tray_set_active_downloads", { count }),

  // Browser integration
  integrationInfo: () => call("get_integration_info"),
  registerBrowser: (browser: string, extensionId: string) =>
    call("register_browser", { browser, extensionId }),
  unregisterBrowser: (browser: string) =>
    call("unregister_browser", { browser }),
  browserSetupAction: (action: string, browser: string) =>
    call("browser_setup_action", { action, browser }),
  advancedCapability: () => call("get_advanced_integration_capability"),
  enableAdvancedIntegration: (consent: boolean) =>
    call("enable_advanced_integration", { consent }),
  disableAdvancedIntegration: () => call("disable_advanced_integration"),
  repairAdvancedIntegration: () => call("repair_advanced_integration"),

  // Clipboard
  dismissClipboardCandidate: (url: string) =>
    call("dismiss_clipboard_candidate", { url }),

  // Media
  playlistInfo: (url: string) => call("get_playlist_info", { url }),
  resolveAsxPlaylist: (urlOrContent: string) =>
    call("resolve_asx_playlist", { urlOrContent }),
  videoFormats: (url: string) => call("get_video_formats", { url }),
  addPlaylistDownloads: (request: {
    playlistId: string;
    playlistTitle: string;
    items: Array<{ id: string; url: string }>;
    commonCategory?: string | null;
    commonDirectory?: string | null;
  }) => call("add_playlist_downloads", { request }),
  mediaToolsStatus: () => call("get_media_tools_status"),
  installMediaTool: (tool: "ffmpeg" | "yt-dlp") =>
    call("install_media_tool", { tool }),

  // Queues and schedules
  listQueues: () => call("list_queues"),
  createQueue: (
    name: string,
    options?: {
      concurrencyLimit?: number;
      speedLimitKbps?: number;
      schedule?: QueueSchedule | null;
    },
  ) => call("create_queue", { name, ...options }),
  updateQueue: (
    id: string,
    patch: {
      name?: string;
      enabled?: boolean;
      concurrencyLimit?: number;
      speedLimitKbps?: number;
      schedule?: QueueSchedule | null;
    },
  ) => call("update_queue", { id, ...patch }),
  deleteQueue: (id: string, disposition?: QueueDisposition) =>
    call("delete_queue", { id, disposition }),
  startQueue: (id: string) => call("start_queue", { id }),
  pauseQueue: (id: string) => call("pause_queue", { id }),
  assignDownloadToQueue: (downloadId: string, queueId: string) =>
    call("assign_download_to_queue", { downloadId, queueId }),
  reorderQueueItems: (queueId: string, itemIds: string[]) =>
    call("reorder_queue_items", { queueId, itemIds }),

  // Credentials
  listCredentialRules: () => call("list_credential_rules"),
  saveCredentialRule: (rule: CredentialRule) =>
    call("save_credential_rule", { rule }),
  deleteCredentialRule: (id: string) =>
    call("delete_credential_rule", { id }),

  // Proxies
  testProxyConnection: (proxy: SettingsPatch["proxy"], testUrl?: string) =>
    call("test_proxy_connection", { proxy: proxy!, testUrl }),
  detectProxy: () => call("detect_proxy"),
  applyDetectedProxy: (detected: DetectedProxyConfig) =>
    call("apply_detected_proxy", { detected }),

  // Torrents
  torrentStatus: () => call("torrent_status"),
  torrentList: () => call("torrent_list"),
  torrentAddMagnet: (magnet: string, options?: TorrentAddOptions) =>
    call("torrent_add_magnet", { magnet, options }),
  torrentAddFile: (path: string, options?: TorrentAddOptions) =>
    call("torrent_add_file", { path, options }),
  torrentAction: (id: number, action: TorrentActionKind) =>
    call("torrent_action", { id, action }),
  torrentFiles: (id: number) => call("torrent_files", { id }),
  torrentSetFiles: (id: number, included: number[]) =>
    call("torrent_set_files", { id, included }),
  torrentTrackerStatus: (id: number) =>
    call("torrent_tracker_status", { id }),
  torrentPeers: (id: number) => call("torrent_peers", { id }),
  torrentPieces: (id: number) => call("torrent_pieces", { id }),
  torrentSettingsGet: () => call("torrent_settings_get"),
  torrentSettingsSet: (settings: TorrentSettings) =>
    call("torrent_settings_set", { settings }),
  torrentSetWatching: (watching: boolean) =>
    call("torrent_set_watching", { watching }),
  torrentRestartEngine: () => call("torrent_restart_engine"),
  torrentTakeReview: () => call("torrent_take_review"),

  chooseDirectory,
  chooseFile,
  readClipboard,
};

export type AdvancedIntegrationCapabilityView = AdvancedIntegrationCapability;
export { call };
export type { CommandMap } from "./commands";
