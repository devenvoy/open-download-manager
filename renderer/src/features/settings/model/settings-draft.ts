import {
  type AppTheme,
  type CapturePolicy,
  type ClipboardMonitoringSettings,
  type CompletionAction,
  type ConnectionOverride,
  type DownloadCategory,
  type DownloadWindowLimit,
  type ProxyProtocol,
  type ProxySettings,
  type Settings,
  type SettingsPatch,
} from "@/types/ipc";

export type PowerKindOption = "none" | "Shutdown" | "Sleep" | "Hibernate";

export interface SettingsDraft {
  theme: AppTheme;
  startWithWindows: boolean;
  clipboardMonitoringEnabled: boolean;
  clipboardAutoEnqueue: boolean;
  defaultDirectory: string;
  categoryDirectories: Partial<Record<DownloadCategory, string>>;
  selectedCategory: DownloadCategory;
  temporaryDirectory: string;
  maxConcurrent: number;
  connections: number;
  speedLimitKbps: number;
  connectionOverrides: ConnectionOverride[];
  windowLimitEnabled: boolean;
  windowLimitMaxMb: number;
  windowLimitHours: number;
  windowLimitUsageText: string;
  proxyEnabled: boolean;
  proxyProtocol: ProxyProtocol;
  proxyHost: string;
  proxyPort: number;
  proxyUsername: string;
  proxyPassword: string;
  proxyResolveDnsRemotely: boolean;
  captureEnabled: boolean;
  captureMode: CapturePolicy["confirmationMode"];
  captureFileTypesText: string;
  captureExcludedHostsText: string;
  captureExcludedAddressesText: string;
  playSound: boolean;
  soundFile: string;
  scanAntivirus: boolean;
  scannerExecutable: string;
  scannerArgsText: string;
  postCommand: boolean;
  commandExecutable: string;
  commandArgsText: string;
  powerActionKind: PowerKindOption;
  powerConfirm: boolean;
}

export function parseFileTypes(input: string): string[] {
  return input
    .split(/[\s,]+/)
    .map((s) => s.trim().toLowerCase().replace(/^\./, ""))
    .filter(Boolean);
}

export function parseList(input: string): string[] {
  return input
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}

export function cleanHostPattern(input: string): string {
  let host = input.trim().toLowerCase().replace(/^https?:\/\//, "");
  if (host.includes("/")) host = host.split("/")[0] ?? "";
  if (host.includes(":")) host = host.split(":")[0] ?? "";
  return host;
}

export function formatWindowLimitUsage(
  limit: DownloadWindowLimit | null | undefined,
): string {
  if (!limit?.enabled) return "";
  const currentMb = ((limit.currentBytes || 0) / (1024 * 1024)).toFixed(1);
  const maxMb = ((limit.maxBytes || 0) / (1024 * 1024)).toFixed(0);
  let text = `Current usage: ${currentMb} / ${maxMb} MB in rolling window.`;
  if (limit.stoppedUntil) {
    text += ` (Quota exceeded; auto-resumes at ${limit.stoppedUntil})`;
  }
  return text;
}

function normalizePowerKind(raw?: string): PowerKindOption {
  if (!raw) return "none";
  const lower = raw.toLowerCase();
  if (lower === "shutdown") return "Shutdown";
  if (lower === "sleep") return "Sleep";
  if (lower === "hibernate") return "Hibernate";
  return "none";
}

export function draftFromSettings(settings: Settings): SettingsDraft {
  const actions = Array.isArray(settings.completionActions)
    ? settings.completionActions
    : [];
  const soundAction = actions.find((a) => a.type === "playSound") as
    | { type: "playSound"; sound_file?: string | null }
    | undefined;
  const scanAction = actions.find((a) => a.type === "antivirusScan") as
    | { type: "antivirusScan"; executable: string; arguments: string[] }
    | undefined;
  const cmdAction = actions.find((a) => a.type === "executeCommand") as
    | { type: "executeCommand"; executable: string; arguments: string[] }
    | undefined;
  const powerAction = actions.find((a) => a.type === "powerAction") as
    | { type: "powerAction"; kind: string; confirmed?: boolean }
    | undefined;

  const windowLimit = settings.windowLimit;
  const capturePolicy = settings.capturePolicy || ({} as Partial<CapturePolicy>);
  const proxy = settings.proxy || ({} as Partial<ProxySettings>);
  const clipboard =
    settings.clipboardMonitoring || ({} as Partial<ClipboardMonitoringSettings>);

  return {
    theme: settings.theme || "system",
    startWithWindows: settings.startWithWindows !== false,
    clipboardMonitoringEnabled: clipboard.enabled === true,
    clipboardAutoEnqueue: clipboard.autoEnqueue === true,
    defaultDirectory: settings.defaultDirectory || "",
    categoryDirectories: { ...(settings.categoryDirectories || {}) },
    selectedCategory: "Archives",
    temporaryDirectory: settings.temporaryDirectory || "",
    maxConcurrent: settings.maxConcurrent || 3,
    connections: settings.connections || 8,
    speedLimitKbps: settings.speedLimitKbps || 0,
    connectionOverrides: Array.isArray(settings.connectionOverrides)
      ? [...settings.connectionOverrides]
      : [],
    windowLimitEnabled: windowLimit?.enabled === true,
    windowLimitMaxMb: windowLimit?.maxBytes
      ? Math.round(windowLimit.maxBytes / (1024 * 1024))
      : 2048,
    windowLimitHours: windowLimit?.windowHours || 12,
    windowLimitUsageText: formatWindowLimitUsage(windowLimit),
    proxyEnabled: proxy.enabled === true,
    proxyProtocol: proxy.protocol || "http",
    proxyHost: proxy.host || "",
    proxyPort: proxy.port || (proxy.protocol === "socks5" ? 1080 : 8080),
    proxyUsername: proxy.username || "",
    proxyPassword: proxy.password || "",
    proxyResolveDnsRemotely: proxy.resolveDnsRemotely === true,
    captureEnabled: capturePolicy.enabled !== false,
    captureMode: capturePolicy.confirmationMode || "askForUnknown",
    captureFileTypesText: Array.isArray(capturePolicy.fileTypes)
      ? capturePolicy.fileTypes.join(" ").toUpperCase()
      : "",
    captureExcludedHostsText: Array.isArray(capturePolicy.excludedHosts)
      ? capturePolicy.excludedHosts.join(", ")
      : "",
    captureExcludedAddressesText: Array.isArray(capturePolicy.excludedAddresses)
      ? capturePolicy.excludedAddresses.join(", ")
      : "",
    playSound: Boolean(soundAction),
    soundFile: soundAction?.sound_file || "",
    scanAntivirus: Boolean(scanAction),
    scannerExecutable: scanAction?.executable || "",
    scannerArgsText: Array.isArray(scanAction?.arguments)
      ? scanAction.arguments.join(" ")
      : "",
    postCommand: Boolean(cmdAction),
    commandExecutable: cmdAction?.executable || "",
    commandArgsText: Array.isArray(cmdAction?.arguments)
      ? cmdAction.arguments.join(" ")
      : "",
    powerActionKind: normalizePowerKind(powerAction?.kind),
    powerConfirm: powerAction ? powerAction.confirmed !== false : true,
  };
}

export function validateSettingsDraft(draft: SettingsDraft): string | null {
  if (!draft.defaultDirectory.trim()) {
    return "Default download folder is required.";
  }
  if (draft.maxConcurrent < 1 || draft.maxConcurrent > 16) {
    return "Maximum simultaneous downloads must be between 1 and 16.";
  }
  if (draft.connections < 1 || draft.connections > 32) {
    return "Maximum connections per file must be between 1 and 32.";
  }
  if (draft.speedLimitKbps < 0 || draft.speedLimitKbps > 1048576) {
    return "Speed limit must be between 0 and 1,048,576 KB/s.";
  }
  if (draft.windowLimitEnabled) {
    if (draft.windowLimitMaxMb < 1) {
      return "Max data volume must be at least 1 MB.";
    }
    if (draft.windowLimitHours < 1 || draft.windowLimitHours > 8760) {
      return "Time window must be between 1 and 8760 hours.";
    }
  }
  if (draft.proxyEnabled && draft.proxyProtocol !== "direct") {
    if (!draft.proxyHost.trim()) {
      return "Proxy host is required when proxy is enabled.";
    }
    if (draft.proxyPort < 1 || draft.proxyPort > 65535) {
      return "Proxy port must be between 1 and 65535.";
    }
  }
  return null;
}

export function buildSettingsPatch(
  draft: SettingsDraft,
  existingSettings?: Settings,
): SettingsPatch {
  const completionActions: CompletionAction[] = [];
  if (draft.playSound) {
    completionActions.push({
      type: "playSound",
      sound_file: draft.soundFile.trim() || null,
    });
  }
  if (draft.scanAntivirus && draft.scannerExecutable.trim()) {
    completionActions.push({
      type: "antivirusScan",
      executable: draft.scannerExecutable.trim(),
      arguments: draft.scannerArgsText.trim()
        ? draft.scannerArgsText.trim().split(/\s+/).filter(Boolean)
        : [],
    });
  }
  if (draft.postCommand && draft.commandExecutable.trim()) {
    completionActions.push({
      type: "executeCommand",
      executable: draft.commandExecutable.trim(),
      arguments: draft.commandArgsText.trim()
        ? draft.commandArgsText.trim().split(/\s+/).filter(Boolean)
        : [],
    });
  }
  if (draft.powerActionKind !== "none") {
    completionActions.push({
      type: "powerAction",
      kind: draft.powerActionKind,
      confirmed: draft.powerConfirm,
    });
  }

  const existingLimit = existingSettings?.windowLimit;
  const windowLimit: DownloadWindowLimit = {
    enabled: draft.windowLimitEnabled,
    windowHours: Number(draft.windowLimitHours) || 12,
    maxBytes: (Number(draft.windowLimitMaxMb) || 2048) * 1024 * 1024,
    windowAnchor: existingLimit?.windowAnchor || new Date().toISOString(),
    currentBytes: existingLimit?.currentBytes || 0,
    stoppedUntil: existingLimit?.stoppedUntil || null,
  };

  return {
    theme: draft.theme,
    startWithWindows: draft.startWithWindows,
    defaultDirectory: draft.defaultDirectory.trim(),
    maxConcurrent: Number(draft.maxConcurrent),
    connections: Number(draft.connections),
    speedLimitKbps: Number(draft.speedLimitKbps),
    categoryDirectories: draft.categoryDirectories,
    temporaryDirectory: draft.temporaryDirectory.trim() || "",
    proxy: {
      enabled: draft.proxyEnabled,
      protocol: draft.proxyProtocol,
      host: draft.proxyHost.trim(),
      port:
        Number(draft.proxyPort) ||
        (draft.proxyProtocol === "socks5" ? 1080 : 8080),
      username: draft.proxyUsername.trim() || null,
      password: draft.proxyPassword || null,
      resolveDnsRemotely: draft.proxyResolveDnsRemotely,
    },
    capturePolicy: {
      enabled: draft.captureEnabled,
      confirmationMode: draft.captureMode,
      fileTypes: parseFileTypes(draft.captureFileTypesText),
      excludedHosts: parseList(draft.captureExcludedHostsText),
      excludedAddresses: parseList(draft.captureExcludedAddressesText),
    },
    completionActions,
    clipboardMonitoring: {
      enabled: draft.clipboardMonitoringEnabled,
      autoEnqueue: draft.clipboardAutoEnqueue,
    },
    connectionOverrides: draft.connectionOverrides,
    windowLimit,
  };
}
