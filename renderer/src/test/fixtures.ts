import {
  type DownloadSummary,
  type IntegrationStatus,
  type Settings,
  type Snapshot,
  type TorrentEngineStatus,
  type TorrentFileItem,
  type TorrentListItem,
  type TorrentSettings,
} from "@/types/ipc";

export function makeDownload(
  overrides: Partial<DownloadSummary> = {},
): DownloadSummary {
  return {
    id: "download-1",
    url: "https://example.com/file.zip",
    filename: "file.zip",
    status: "downloading",
    downloaded: 512,
    total: 1024,
    speed: 2048,
    createdAt: "2026-11-13T10:00:00Z",
    needsReview: false,
    scheduledAt: null,
    queueId: null,
    category: "Archives",
    directory: null,
    path: null,
    tempDirectory: null,
    lastTry: null,
    description: null,
    error: null,
    notice: null,
    estimated: false,
    kind: null,
    resumable: null,
    duration: null,
    connections: 4,
    connectionsProgress: [],
    transferKind: null,
    referer: null,
    actionLedger: [],
    ...overrides,
  };
}

export function makeSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    defaultDirectory: "/Users/test/Downloads",
    maxConcurrent: 3,
    connections: 8,
    speedLimitKbps: 0,
    categoryDirectories: {},
    temporaryDirectory: null,
    notifyOnComplete: true,
    revealOnComplete: false,
    confirmBrowserDownloads: true,
    startWithWindows: false,
    hideCompletionDialog: false,
    proxy: {
      enabled: false,
      protocol: "http",
      host: "",
      port: 8080,
      username: null,
      password: null,
      resolveDnsRemotely: false,
    },
    capturePolicy: {
      fileTypes: [],
      excludedHosts: [],
      excludedAddresses: [],
      confirmationMode: "askForUnknown",
      enabled: true,
    },
    completionActions: [],
    clipboardMonitoring: { enabled: false, autoEnqueue: false },
    connectionOverrides: [],
    windowLimit: null,
    theme: "system",
    ...overrides,
  };
}

export function makeIntegrationStatus(
  overrides: Partial<IntegrationStatus> = {},
): IntegrationStatus {
  return {
    port: null,
    connectedBrowsers: [],
    browsers: [],
    connectionError: null,
    extensionPath: "",
    extensionAvailable: false,
    integrationPath: "",
    packaged: false,
    executable: "",
    ...overrides,
  };
}

export function makeSnapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  return {
    downloads: [],
    settings: makeSettings(),
    integration: makeIntegrationStatus(),
    fileIcons: {},
    ...overrides,
  };
}

export function makeTorrent(
  overrides: Partial<TorrentListItem> = {},
): TorrentListItem {
  return {
    id: 1,
    infoHash: "hash-1",
    name: "debian.iso",
    outputFolder: "/Users/test/Downloads/Torrents",
    state: "live",
    error: null,
    progressBytes: 512,
    uploadedBytes: 128,
    totalBytes: 1024,
    finished: false,
    downloadSpeed: 2048,
    uploadSpeed: 128,
    timeRemainingSecs: 30,
    seeds: 3,
    leechers: 2,
    peers: 5,
    ratio: 0.5,
    addedOn: null,
    trackers: [],
    files: [],
    ...overrides,
  };
}

export function makeTorrentFile(
  overrides: Partial<TorrentFileItem> = {},
): TorrentFileItem {
  return {
    index: 0,
    name: "debian.iso",
    length: 1024,
    included: true,
    priority: 1,
    progressBytes: 512,
    ...overrides,
  };
}

export function makeTorrentSettings(
  overrides: Partial<TorrentSettings> = {},
): TorrentSettings {
  return {
    version: 1,
    enabled: true,
    downloadDirectory: "/Users/test/Downloads/Torrents",
    listenPort: 0,
    enableDht: true,
    enableUpnp: true,
    enableUtp: true,
    downloadLimitKbps: 0,
    uploadLimitKbps: 0,
    maxPeersPerTorrent: 50,
    seedRatioLimit: 0,
    seedTimeLimitMinutes: 0,
    startPaused: false,
    notifyOnComplete: true,
    ...overrides,
  };
}

export function makeTorrentStatus(
  overrides: Partial<TorrentEngineStatus> = {},
): TorrentEngineStatus {
  return {
    available: true,
    enabled: true,
    state: "ready",
    message: null,
    listenPort: 51413,
    dhtEnabled: true,
    restartRequired: false,
    ...overrides,
  };
}
