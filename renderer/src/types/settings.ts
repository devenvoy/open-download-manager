import { type CompletionAction, type DownloadCategory } from "./download";

/** Settings domain DTOs — mirrors src-tauri/src/domain/settings/**. */

/** `AppTheme` — src-tauri/src/domain/settings/theme.rs */
export type AppTheme = "system" | "light" | "dark";

/** `ProxyProtocol` — src-tauri/src/domain/settings/proxy.rs */
export type ProxyProtocol = "direct" | "http" | "https" | "socks5";

/** `ProxySettings` — src-tauri/src/domain/settings/proxy.rs */
export interface ProxySettings {
  enabled: boolean;
  protocol: ProxyProtocol;
  host: string;
  port: number;
  username: string | null;
  password: string | null;
  resolveDnsRemotely: boolean;
}

/** `ProxyTestResult` — src-tauri/src/domain/settings/proxy.rs */
export interface ProxyTestResult {
  success: boolean;
  message: string;
}

/** `ConfirmationMode` — src-tauri/src/domain/browser/capture_policy.rs */
export type ConfirmationMode = "automatic" | "askForUnknown" | "alwaysAsk";

/** `CapturePolicy` — src-tauri/src/domain/browser/capture_policy.rs */
export interface CapturePolicy {
  fileTypes: string[];
  excludedHosts: string[];
  excludedAddresses: string[];
  confirmationMode: ConfirmationMode;
  enabled: boolean;
}

/** `ClipboardMonitoringSettings` — src-tauri/src/domain/clipboard.rs */
export interface ClipboardMonitoringSettings {
  enabled: boolean;
  autoEnqueue: boolean;
}

/** `ConnectionOverride` — src-tauri/src/domain/settings/connection_override.rs */
export interface ConnectionOverride {
  hostPattern: string;
  connections: number;
}

/** `DownloadWindowLimit` — src-tauri/src/domain/settings/window_limit.rs */
export interface DownloadWindowLimit {
  enabled: boolean;
  windowHours: number;
  maxBytes: number;
  windowAnchor: string;
  currentBytes: number;
  stoppedUntil: string | null;
}

/** `Settings` — src-tauri/src/domain/settings.rs */
export interface Settings {
  defaultDirectory: string;
  maxConcurrent: number;
  connections: number;
  speedLimitKbps: number;
  categoryDirectories: Partial<Record<DownloadCategory, string>>;
  temporaryDirectory: string | null;
  notifyOnComplete: boolean;
  revealOnComplete: boolean;
  confirmBrowserDownloads: boolean;
  startWithWindows: boolean;
  hideCompletionDialog: boolean;
  proxy: ProxySettings;
  capturePolicy: CapturePolicy;
  completionActions: CompletionAction[];
  clipboardMonitoring: ClipboardMonitoringSettings;
  connectionOverrides: ConnectionOverride[];
  windowLimit: DownloadWindowLimit | null;
  theme: AppTheme;
}

/** `SettingsPatch` — src-tauri/src/domain/settings/patch.rs */
export interface SettingsPatch {
  defaultDirectory?: string;
  maxConcurrent?: number;
  connections?: number;
  speedLimitKbps?: number;
  categoryDirectories?: Partial<Record<DownloadCategory, string>>;
  temporaryDirectory?: string;
  notifyOnComplete?: boolean;
  revealOnComplete?: boolean;
  confirmBrowserDownloads?: boolean;
  startWithWindows?: boolean;
  hideCompletionDialog?: boolean;
  proxy?: ProxySettings;
  capturePolicy?: CapturePolicy;
  completionActions?: CompletionAction[];
  clipboardMonitoring?: ClipboardMonitoringSettings;
  connectionOverrides?: ConnectionOverride[];
  windowLimit?: DownloadWindowLimit;
  theme?: AppTheme;
}

/** `ProtocolRoute` — src-tauri/src/domain/settings/proxy_discovery.rs */
export interface ProtocolRoute {
  protocol: string;
  host: string;
  port: number;
  supported: boolean;
  reason: string | null;
}

/** `ProxySource` — src-tauri/src/domain/settings/proxy_discovery.rs */
export type ProxySource = "system" | "environment" | "manual" | { browser: string };

/** `DetectedProxyMode` — src-tauri/src/domain/settings/proxy_discovery.rs */
export type DetectedProxyMode = "direct" | "manual" | "pac";

/** `DetectedProxyConfig` — src-tauri/src/domain/settings/proxy_discovery.rs */
export interface DetectedProxyConfig {
  source: ProxySource;
  mode: DetectedProxyMode;
  pacUrl: string | null;
  routes: ProtocolRoute[];
  credentialsAvailable: boolean;
  unsupportedFields: string[];
  canImport: boolean;
  importSettings: ProxySettings | null;
}
