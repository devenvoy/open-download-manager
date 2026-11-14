import { useState } from "react";
import { ClassicDialog } from "@/components/feedback";
import {
  type DetectedProxyConfig,
  type ProxySettings,
  type Settings,
} from "@/types/ipc";
import { useApplyDetectedProxy } from "../hooks/use-settings";
import {
  formatProxyDisplay,
  formatRouteBadge,
  formatRouteLabel,
  proxyModeLabel,
  proxySourceLabel,
} from "../model/proxy-view";

export interface ProxyDiscoveryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  detected: DetectedProxyConfig | null;
  currentProxy?: ProxySettings;
  onImport: (appliedSettings: Settings) => void;
}

export function ProxyDiscoveryDialog({
  open,
  onOpenChange,
  detected,
  currentProxy,
  onImport,
}: ProxyDiscoveryDialogProps) {
  const [error, setError] = useState<string | null>(null);
  const applyMutation = useApplyDetectedProxy();

  if (!open || !detected) return null;

  async function handleImport() {
    if (!detected || !detected.canImport || !detected.importSettings) return;
    setError(null);
    try {
      const updated = await applyMutation.mutateAsync(detected);
      onImport(updated);
      onOpenChange(false);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to import proxy settings",
      );
    }
  }

  const discoveredDisplay = detected.importSettings
    ? formatProxyDisplay(detected.importSettings)
    : detected.mode === "pac"
      ? "PAC Script (Auto-routing unavailable)"
      : "Direct (No Proxy)";

  const currentDisplay = currentProxy
    ? formatProxyDisplay(currentProxy)
    : "Direct (Disabled)";

  return (
    <ClassicDialog
      id="proxy-discovery-dialog"
      className="proxy-discovery-dialog"
      open={open}
      onClose={() => onOpenChange(false)}
      title="Discovered Proxy Configuration"
      titleId="proxy-discovery-title"
    >
      <div className="dialog-body proxy-discovery-body">
        <div className="proxy-discovery-summary">
          <span className="badge" id="proxy-discovery-source-badge">
            {proxySourceLabel(detected.source)}
          </span>
          <span className="badge" id="proxy-discovery-mode-badge">
            {proxyModeLabel(detected.mode)}
          </span>
          {detected.credentialsAvailable && (
            <span className="badge auth-badge" id="proxy-discovery-auth-badge">
              Authentication Detected
            </span>
          )}
        </div>

        {detected.mode === "pac" && detected.pacUrl && (
          <div id="proxy-discovery-pac-section" className="proxy-discovery-section">
            <label>Auto-configuration script (PAC URL):</label>
            <div className="code-preview" id="proxy-discovery-pac-url">
              {detected.pacUrl}
            </div>
            <p className="field-help warning-text">
              PAC automatic script routing is not evaluated; route resolution is
              unavailable rather than direct fallback.
            </p>
          </div>
        )}

        {detected.routes && detected.routes.length > 0 && (
          <div
            id="proxy-discovery-routes-section"
            className="proxy-discovery-section"
          >
            <label>Detected Protocol Routes:</label>
            <ul id="proxy-discovery-routes-list" className="proxy-routes-list">
              {detected.routes.map((route, idx) => {
                const badge = formatRouteBadge(route);
                return (
                  <li
                    key={idx}
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      padding: "2px 0",
                    }}
                  >
                    <span>{formatRouteLabel(route)}</span>
                    <span
                      style={{
                        color: badge.supported ? "green" : "red",
                        fontSize: "10px",
                      }}
                    >
                      {badge.label}
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        <div id="proxy-discovery-diff-section" className="proxy-discovery-section">
          <label>Configuration Preview / Diff:</label>
          <div className="proxy-diff-box">
            <div className="diff-col">
              <span className="diff-title">Current Setting:</span>
              <span id="proxy-diff-current" className="diff-val">
                {currentDisplay}
              </span>
            </div>
            <div className="diff-arrow">→</div>
            <div className="diff-col">
              <span className="diff-title">Discovered Setting:</span>
              <span id="proxy-diff-discovered" className="diff-val">
                {discoveredDisplay}
              </span>
            </div>
          </div>
        </div>

        {detected.credentialsAvailable && (
          <p className="field-help secure-note" id="proxy-discovery-auth-note">
            Protected credential notice: Passwords are protected and never
            exposed. If authentication is required, enter credentials securely
            in settings.
          </p>
        )}

        {error && (
          <p className="form-error" id="proxy-discovery-error" role="alert">
            {error}
          </p>
        )}
      </div>

      <div className="dialog-footer">
        <button
          className="button"
          id="proxy-discovery-cancel-btn"
          type="button"
          onClick={() => onOpenChange(false)}
        >
          Cancel
        </button>
        <button
          className="button primary"
          id="proxy-discovery-import-btn"
          type="button"
          disabled={!detected.canImport || applyMutation.isPending}
          onClick={() => void handleImport()}
        >
          Import Settings
        </button>
      </div>
    </ClassicDialog>
  );
}
