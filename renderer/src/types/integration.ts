/** Browser integration DTOs — mirrors src-tauri/src/domain/snapshot/**
 *  and src-tauri/src/domain/browser/advanced_integration.rs. */

/** `ConnectedBrowser` — src-tauri/src/domain/snapshot/integration_status.rs */
export interface ConnectedBrowser {
  browser: string;
  name: string;
  connected: boolean;
}

/** `BrowserStatus` — src-tauri/src/domain/snapshot/browser_status.rs */
export interface BrowserStatus {
  name: string;
  installed: boolean;
  connected: boolean;
  setupState: string;
  icon: string;
  extensionPage: string;
  storeUrl: string | null;
  extensionPath: string;
  extensionAvailable: boolean;
  lastSeenMs: number | null;
  support: string;
}

/** `IntegrationStatus` — src-tauri/src/domain/snapshot/integration_status.rs */
export interface IntegrationStatus {
  port: number | null;
  connectedBrowsers: ConnectedBrowser[];
  browsers: BrowserStatus[];
  connectionError: string | null;
  extensionPath: string;
  extensionAvailable: boolean;
  integrationPath: string;
  packaged: boolean;
  executable: string;
}

/** `AdvancedIntegrationAvailability` */
export type AdvancedIntegrationAvailability =
  | { status: "supported"; platform: string; details: string }
  | { status: "unsupported"; platform: string; reason: string };

/** `AdvancedIntegrationState` */
export type AdvancedIntegrationState =
  | { state: "disabled" }
  | { state: "awaitingConsent" }
  | { state: "active" }
  | { state: "degraded"; reason: string; rolledBack: boolean }
  | { state: "unsupported"; reason: string };

/** `HealthCheckResult` */
export interface HealthCheckResult {
  success: boolean;
  checkedAt: number;
  latencyMs: number;
  conflictDetected: boolean;
  details: string;
}

/** `RollbackAction` */
export interface RollbackAction {
  executed: boolean;
  reason: string;
  standardIntegrationIntact: boolean;
  uninstalled: boolean;
  timestamp: number;
}

/** `AdvancedIntegrationCapability` */
export interface AdvancedIntegrationCapability {
  availability: AdvancedIntegrationAvailability;
  requiredPermission: string | null;
  state: AdvancedIntegrationState;
  lastHealthCheck: HealthCheckResult | null;
  rollbackAction: RollbackAction | null;
}

/** `BrowserSetupResult` — src-tauri/src/application/integration_service.rs */
export interface BrowserSetupResult {
  browser?: string | null;
  extensionPath?: string | null;
}
