import { type BrowserStatus, type IntegrationStatus } from "@/types/ipc";

/**
 * Pure rules for the Browser integration window.
 * Ported 1:1 from `renderBrowserGrid` / `showBrowserDetail` in
 * `reference/legacy-renderer/app.js`, with no DOM access so they are testable.
 */

export type BrowserTone = "connected" | "setup" | "missing";

/** The connection states the backend reports, turned into a label and tone. */
export function browserStatus(browser: BrowserStatus): {
  label: string;
  tone: BrowserTone;
} {
  if (browser.setupState === "not-installed") {
    return { label: "Not installed", tone: "missing" };
  }
  if (browser.setupState === "connected") {
    return { label: "Connected", tone: "connected" };
  }
  if (browser.setupState === "disconnected") {
    return { label: "Disconnected", tone: "setup" };
  }
  return { label: "Setup needed", tone: "setup" };
}

export function lastSeenLabel(lastSeenMs: number | null, now = Date.now()): string {
  if (!lastSeenMs) return "Connected";
  const minutes = Math.max(0, Math.round((now - lastSeenMs) / 60_000));
  if (minutes < 1) return "Connected just now";
  if (minutes < 60) {
    return `Connected ${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `Connected ${hours} hour${hours === 1 ? "" : "s"} ago`;
  }
  const days = Math.round(hours / 24);
  return `Connected ${days} day${days === 1 ? "" : "s"} ago`;
}

export function browserSubtitle(browser: BrowserStatus, now = Date.now()): string {
  if (browser.connected) return lastSeenLabel(browser.lastSeenMs, now);
  if (browser.setupState === "disconnected") {
    return "Extension removed or disabled";
  }
  if (browser.setupState === "not-installed") return "Browser is not installed";
  return "Extension not detected";
}

/** Browsers the window offers: installed, or connected (so a removed one shows). */
export function visibleBrowsers(info: IntegrationStatus | undefined): BrowserStatus[] {
  if (!info || !Array.isArray(info.browsers)) return [];
  return info.browsers.filter((browser) => browser.installed || browser.connected);
}

export function connectionSummary(info: IntegrationStatus | undefined): string {
  if (!info) return "Connecting…";
  if (info.connectionError) return `Setup check failed: ${info.connectionError}`;
  const connected = visibleBrowsers(info).filter((browser) => browser.connected).length;
  return `${connected} browser${connected === 1 ? "" : "s"} connected`;
}

export type BrowserAction = "store" | "prepare" | "open";

export function primaryAction(browser: BrowserStatus): BrowserAction {
  if (browser.storeUrl && !browser.connected) return "store";
  if (browser.name === "Firefox") return "prepare";
  return "open";
}

export function primaryActionLabel(
  browser: BrowserStatus,
  action: BrowserAction = primaryAction(browser),
): string {
  return action === "store" ? "Install browser extension" : `Open ${browser.name}`;
}

/** Whether the "Install from Web Store" fallback button is needed. */
export function needsStoreFallback(browser: BrowserStatus): boolean {
  return Boolean(browser.storeUrl) && primaryAction(browser) !== "store";
}

export function browserSummary(
  browser: BrowserStatus,
  now = Date.now(),
): string {
  if (browser.connected) {
    return `The extension and desktop connector are ready. ${lastSeenLabel(browser.lastSeenMs, now)}. Refresh an existing video tab if its download button is missing.`;
  }
  if (browser.setupState === "disconnected") {
    return `The extension in ${browser.name} was removed or disabled. Load it again to reconnect.`;
  }
  if (browser.setupState === "not-installed") {
    return `${browser.name} is not installed on this computer.`;
  }
  return "Download the extension, then load it in the browser to connect.";
}

export function setupSteps(browser: BrowserStatus): string[] {
  if (browser.storeUrl) {
    return [
      "Click Install browser extension to open the official Chrome Web Store listing in this browser.",
      "Click Add to Chrome (or the equivalent button in your browser), then confirm the installation.",
      "The desktop connector is prepared automatically. Return here and click Refresh, then reload your video page.",
    ];
  }
  if (browser.name === "Firefox") {
    return [
      "Click Download extension. The extension is saved in your Downloads folder and the folder opens in Finder.",
      "Click Open Firefox. Open Download Manager registers its local connector and opens Firefox.",
      `Enter ${browser.extensionPage} in Firefox's address bar.`,
      "On the Firefox page, select This Firefox, then Load Temporary Add-on.",
      "Choose manifest.json inside the extension folder shown below.",
      "Return here, click Refresh, then reload your video page.",
    ];
  }
  return [
    "Click Download extension. The extension is saved in your Downloads folder and the folder opens in Finder.",
    `Click Open ${browser.name}.`,
    `Enter ${browser.extensionPage} in ${browser.name}'s address bar.`,
    "Turn on Developer mode on the Extensions page.",
    "Choose Load unpacked and select the extension folder shown below.",
    "Return here and click Refresh. The browser shows as Connected once the extension answers.",
  ];
}

export function automaticSetupCopy(browser: BrowserStatus): string {
  if (browser.storeUrl) {
    return "Open Download Manager prepares its connector automatically. Your browser asks you to confirm adding the published extension.";
  }
  if (browser.name === "Firefox") {
    return "Firefox requires a signed add-on for permanent installation. Until the add-on is published, Firefox asks you to load the included extension again after restarting the browser.";
  }
  return "Use the included extension folder for this browser.";
}

export type AdvancedStateKind =
  | "disabled"
  | "awaitingConsent"
  | "active"
  | "degraded"
  | "unsupported";

export function advancedStateKind(
  state: { state: string } | string | null | undefined,
): AdvancedStateKind {
  if (!state) return "disabled";
  const raw = typeof state === "string" ? state : state.state;
  if (
    raw === "active" ||
    raw === "degraded" ||
    raw === "unsupported" ||
    raw === "awaitingConsent"
  ) {
    return raw;
  }
  return "disabled";
}
