import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
  currentProxy: ProxySettings;
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

  if (!detected) return null;

  async function handleImport() {
    if (!detected || !detected.canImport || !detected.importSettings) return;
    setError(null);
    try {
      const updated = await applyMutation.mutateAsync(detected);
      onImport(updated);
      onOpenChange(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to import proxy settings");
    }
  }

  const discoveredDisplay = detected.importSettings
    ? formatProxyDisplay(detected.importSettings)
    : detected.mode === "pac"
      ? "PAC Script (Auto-routing unavailable)"
      : "Direct (No Proxy)";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle id="proxy-discovery-title">
            Discovered Proxy Configuration
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 text-sm">
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline" id="proxy-discovery-source-badge">
              {proxySourceLabel(detected.source)}
            </Badge>
            <Badge variant="outline" id="proxy-discovery-mode-badge">
              {proxyModeLabel(detected.mode)}
            </Badge>
            {detected.credentialsAvailable && (
              <Badge variant="secondary" id="proxy-discovery-auth-badge">
                Authentication Detected
              </Badge>
            )}
          </div>

          {detected.mode === "pac" && detected.pacUrl && (
            <div id="proxy-discovery-pac-section" className="space-y-1">
              <label className="text-xs font-medium text-muted-foreground">
                Auto-configuration script (PAC URL):
              </label>
              <div
                id="proxy-discovery-pac-url"
                className="font-mono text-xs bg-muted p-2 rounded break-all"
              >
                {detected.pacUrl}
              </div>
              <p className="text-xs text-amber-600 dark:text-amber-400">
                PAC automatic script routing is not evaluated; route
                resolution is unavailable rather than direct fallback.
              </p>
            </div>
          )}

          {detected.routes && detected.routes.length > 0 && (
            <div id="proxy-discovery-routes-section" className="space-y-1">
              <label className="text-xs font-medium text-muted-foreground">
                Detected Protocol Routes:
              </label>
              <ul id="proxy-discovery-routes-list" className="space-y-1">
                {detected.routes.map((route, idx) => {
                  const badge = formatRouteBadge(route);
                  return (
                    <li
                      key={idx}
                      className="flex items-center justify-between rounded border px-2 py-1 text-xs"
                    >
                      <span className="font-mono">{formatRouteLabel(route)}</span>
                      <Badge
                        variant={badge.supported ? "outline" : "destructive"}
                        className="text-[10px]"
                      >
                        {badge.label}
                      </Badge>
                    </li>
                  );
                })}
              </ul>
            </div>
          )}

          <div id="proxy-discovery-diff-section" className="space-y-1">
            <label className="text-xs font-medium text-muted-foreground">
              Configuration Preview / Diff:
            </label>
            <div className="flex items-center justify-between rounded border p-2 text-xs bg-muted/40">
              <div>
                <span className="text-muted-foreground block text-[10px]">
                  Current Setting:
                </span>
                <span id="proxy-diff-current" className="font-mono font-medium">
                  {formatProxyDisplay(currentProxy)}
                </span>
              </div>
              <div className="text-muted-foreground">→</div>
              <div className="text-right">
                <span className="text-muted-foreground block text-[10px]">
                  Discovered Setting:
                </span>
                <span
                  id="proxy-diff-discovered"
                  className="font-mono font-medium"
                >
                  {discoveredDisplay}
                </span>
              </div>
            </div>
          </div>

          {detected.unsupportedFields &&
            detected.unsupportedFields.length > 0 && (
              <div
                id="proxy-discovery-unsupported-section"
                className="rounded border border-amber-500/50 bg-amber-500/10 p-2 text-xs text-amber-600 dark:text-amber-400"
              >
                <div id="proxy-discovery-unsupported-list" className="space-y-1">
                  {detected.unsupportedFields.map((f, i) => (
                    <div key={i}>• {f}</div>
                  ))}
                </div>
              </div>
            )}

          {detected.credentialsAvailable && (
            <p
              id="proxy-discovery-auth-note"
              className="text-xs text-muted-foreground italic"
            >
              Protected credential notice: Passwords are protected and never
              exposed. If authentication is required, enter credentials
              securely in settings.
            </p>
          )}

          {error && (
            <p id="proxy-discovery-error" role="alert" className="text-xs text-destructive">
              {error}
            </p>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            id="proxy-discovery-cancel-btn"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button
            type="button"
            id="proxy-discovery-import-btn"
            disabled={
              !detected.canImport ||
              !detected.importSettings ||
              applyMutation.isPending
            }
            onClick={() => void handleImport()}
          >
            {applyMutation.isPending ? "Importing…" : "Import Settings"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
