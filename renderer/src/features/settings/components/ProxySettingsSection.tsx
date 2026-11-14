import { useState } from "react";
import {
  type DetectedProxyConfig,
  type ProxyProtocol,
  type Settings,
} from "@/types/ipc";
import { useDetectProxy, useTestProxy } from "../hooks/use-settings";
import { defaultPortForProtocol } from "../model/proxy-view";
import { type SettingsDraft } from "../model/settings-draft";
import { ProxyDiscoveryDialog } from "./ProxyDiscoveryDialog";

export interface ProxySettingsSectionProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
  onError: (msg: string) => void;
}

export function ProxySettingsSection({
  draft,
  onChange,
  onError,
}: ProxySettingsSectionProps) {
  const [testStatus, setTestStatus] = useState<{
    type: "idle" | "testing" | "success" | "error";
    message: string;
  }>({ type: "idle", message: "" });
  const [discoveryOpen, setDiscoveryOpen] = useState(false);
  const [detectedProxy, setDetectedProxy] =
    useState<DetectedProxyConfig | null>(null);

  const testProxyMutation = useTestProxy();
  const detectProxyMutation = useDetectProxy();

  const isDirect = draft.proxyProtocol === "direct";
  const isSocks5 = draft.proxyProtocol === "socks5";

  function handleProtocolChange(proto: ProxyProtocol) {
    onChange("proxyProtocol", proto);
    onChange("proxyPort", defaultPortForProtocol(proto, draft.proxyPort));
    setTestStatus({ type: "idle", message: "" });
  }

  async function handleTestProxy() {
    if (!isDirect && !draft.proxyHost.trim()) {
      setTestStatus({ type: "error", message: "Please enter a proxy host." });
      return;
    }

    setTestStatus({
      type: "testing",
      message: isDirect
        ? "Testing direct connection…"
        : "Testing proxy connection…",
    });

    try {
      const res = await testProxyMutation.mutateAsync({
        proxy: {
          enabled: true,
          protocol: draft.proxyProtocol,
          host: draft.proxyHost.trim(),
          port: Number(draft.proxyPort) || (isSocks5 ? 1080 : 8080),
          username: draft.proxyUsername.trim() || null,
          password: draft.proxyPassword || null,
          resolveDnsRemotely: draft.proxyResolveDnsRemotely,
        },
      });

      setTestStatus({
        type: res.success ? "success" : "error",
        message:
          res.message ||
          (res.success ? "Connection successful." : "Connection failed."),
      });
    } catch (err) {
      setTestStatus({
        type: "error",
        message: err instanceof Error ? err.message : "Connection failed.",
      });
    }
  }

  async function handleDetectProxy() {
    try {
      const config = await detectProxyMutation.mutateAsync();
      setDetectedProxy(config);
      setDiscoveryOpen(true);
    } catch (err) {
      onError(
        err instanceof Error ? err.message : "Failed to detect system proxy.",
      );
    }
  }

  function handleImportDetected(updatedSettings: Settings) {
    const p = updatedSettings.proxy;
    if (p) {
      onChange("proxyEnabled", p.enabled);
      onChange("proxyProtocol", p.protocol);
      onChange("proxyHost", p.host);
      onChange("proxyPort", p.port);
      onChange("proxyUsername", p.username ?? "");
      onChange("proxyPassword", p.password ?? "");
      onChange("proxyResolveDnsRemotely", p.resolveDnsRemotely);
    }
    setDiscoveryOpen(false);
  }

  return (
    <>
      <fieldset>
        <legend>Proxy</legend>
        <div className="proxy-header-actions">
          <label className="checkbox-label" htmlFor="proxy-enabled">
            <input
              type="checkbox"
              id="proxy-enabled"
              checked={draft.proxyEnabled}
              onChange={(e) => onChange("proxyEnabled", e.target.checked)}
            />
            Use proxy server for downloads
          </label>
          <button
            className="small-button"
            id="detect-proxy-button"
            type="button"
            onClick={() => void handleDetectProxy()}
          >
            Detect System Proxy…
          </button>
        </div>

        {draft.proxyEnabled && (
          <div id="proxy-fields" className="proxy-fields">
            <div className="form-line">
              <label htmlFor="proxy-protocol">Protocol:</label>
              <select
                id="proxy-protocol"
                value={draft.proxyProtocol}
                onChange={(e) =>
                  handleProtocolChange(e.target.value as ProxyProtocol)
                }
              >
                <option value="http">HTTP</option>
                <option value="https">HTTPS</option>
                <option value="socks5">SOCKS5</option>
                <option value="direct">Direct (No Proxy)</option>
              </select>
            </div>

            {!isDirect && (
              <>
                <div className="form-line">
                  <label htmlFor="proxy-host">Proxy server (host):</label>
                  <input
                    type="text"
                    id="proxy-host"
                    placeholder="127.0.0.1 or proxy.example.com"
                    value={draft.proxyHost}
                    onChange={(e) => onChange("proxyHost", e.target.value)}
                  />
                </div>
                <div className="form-line">
                  <label htmlFor="proxy-port">Port:</label>
                  <input
                    type="number"
                    id="proxy-port"
                    min={1}
                    max={65535}
                    placeholder="8080"
                    value={draft.proxyPort}
                    onChange={(e) =>
                      onChange("proxyPort", Number(e.target.value) || 0)
                    }
                  />
                </div>
                {isSocks5 && (
                  <div
                    id="proxy-dns-group"
                    className="form-line proxy-dns-line"
                  >
                    <label
                      className="checkbox-label"
                      htmlFor="proxy-resolve-dns-remotely"
                    >
                      <input
                        type="checkbox"
                        id="proxy-resolve-dns-remotely"
                        checked={draft.proxyResolveDnsRemotely}
                        onChange={(e) =>
                          onChange(
                            "proxyResolveDnsRemotely",
                            e.target.checked,
                          )
                        }
                      />
                      Resolve DNS through proxy (Remote DNS)
                    </label>
                  </div>
                )}
                <div className="form-line">
                  <label htmlFor="proxy-username">Username (optional):</label>
                  <input
                    type="text"
                    id="proxy-username"
                    autoComplete="off"
                    value={draft.proxyUsername}
                    onChange={(e) => onChange("proxyUsername", e.target.value)}
                  />
                </div>
                <div className="form-line">
                  <label htmlFor="proxy-password">Password (optional):</label>
                  <input
                    type="password"
                    id="proxy-password"
                    autoComplete="off"
                    value={draft.proxyPassword}
                    onChange={(e) => onChange("proxyPassword", e.target.value)}
                  />
                </div>
              </>
            )}

            <div className="form-line proxy-test-line">
              <button
                className="small-button"
                id="test-proxy-button"
                type="button"
                disabled={testStatus.type === "testing"}
                onClick={() => void handleTestProxy()}
              >
                Test Connection
              </button>
              <span
                id="proxy-test-status"
                className="proxy-test-status"
                role="status"
                aria-live="polite"
              >
                {testStatus.message}
              </span>
            </div>
          </div>
        )}
      </fieldset>

      {discoveryOpen && detectedProxy && (
        <ProxyDiscoveryDialog
          open={discoveryOpen}
          detected={detectedProxy}
          onOpenChange={setDiscoveryOpen}
          onImport={handleImportDetected}
        />
      )}
    </>
  );
}
