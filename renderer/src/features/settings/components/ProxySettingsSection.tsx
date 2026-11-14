import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  type DetectedProxyConfig,
  type ProxyProtocol,
  type Settings,
} from "@/types/ipc";
import { useDetectProxy, useTestProxy } from "../hooks/use-settings";
import { defaultPortForProtocol } from "../model/proxy-view";
import { type SettingsDraft } from "../model/settings-draft";
import { ProxyDiscoveryDialog } from "./ProxyDiscoveryDialog";

const PROTOCOLS: ReadonlyArray<{ value: ProxyProtocol; label: string }> = [
  { value: "http", label: "HTTP" },
  { value: "https", label: "HTTPS" },
  { value: "socks5", label: "SOCKS5" },
  { value: "direct", label: "Direct (No Proxy)" },
];

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
      message: isDirect ? "Testing direct connection…" : "Testing proxy connection…",
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
        message: res.message || (res.success ? "Connection successful." : "Connection failed."),
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
      onError(`Discovery error: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  function handleImportedProxy(updated: Settings) {
    const p = updated.proxy || detectedProxy?.importSettings;
    if (p) {
      onChange("proxyEnabled", p.enabled === true);
      onChange("proxyProtocol", p.protocol || "http");
      onChange("proxyHost", p.host || "");
      onChange("proxyPort", p.port || 8080);
      onChange("proxyUsername", p.username || "");
      onChange("proxyPassword", "");
      onChange("proxyResolveDnsRemotely", p.resolveDnsRemotely === true);
      setTestStatus({ type: "success", message: "Discovered proxy settings applied." });
    }
  }

  return (
    <fieldset className="rounded-md border p-4 space-y-4">
      <legend className="px-2 text-sm font-semibold">Proxy</legend>
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <Checkbox
            id="proxy-enabled"
            checked={draft.proxyEnabled}
            onCheckedChange={(c) => {
              onChange("proxyEnabled", c === true);
              setTestStatus({ type: "idle", message: "" });
            }}
          />
          <Label htmlFor="proxy-enabled" className="text-sm font-normal cursor-pointer">
            Use proxy server for downloads
          </Label>
        </div>
        <Button
          type="button"
          variant="outline"
          id="detect-proxy-button"
          disabled={detectProxyMutation.isPending}
          onClick={() => void handleDetectProxy()}
        >
          {detectProxyMutation.isPending ? "Detecting…" : "Detect System Proxy…"}
        </Button>
      </div>

      <div id="proxy-fields" className={`space-y-3 pl-6 ${!draft.proxyEnabled ? "opacity-50" : ""}`}>
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="proxy-protocol">Protocol:</Label>
          <Select
            value={draft.proxyProtocol}
            disabled={!draft.proxyEnabled}
            onValueChange={(val) => handleProtocolChange(val as ProxyProtocol)}
          >
            <SelectTrigger id="proxy-protocol" className="w-56"><SelectValue /></SelectTrigger>
            <SelectContent>
              {PROTOCOLS.map((p) => (
                <SelectItem key={p.value} value={p.value}>{p.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="proxy-host">Proxy server (host):</Label>
          <Input
            id="proxy-host"
            value={draft.proxyHost}
            disabled={!draft.proxyEnabled || isDirect}
            placeholder="127.0.0.1 or proxy.example.com"
            className="w-56"
            onChange={(e) => {
              onChange("proxyHost", e.target.value);
              setTestStatus({ type: "idle", message: "" });
            }}
          />
        </div>

        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="proxy-port">Port:</Label>
          <Input
            id="proxy-port"
            type="number"
            min={1}
            max={65535}
            disabled={!draft.proxyEnabled || isDirect}
            placeholder="8080"
            className="w-24 text-right"
            value={draft.proxyPort}
            onChange={(e) =>
              onChange("proxyPort", Math.max(1, Number(e.target.value) || 8080))
            }
          />
        </div>

        {isSocks5 && (
          <div id="proxy-dns-group" className="flex items-center gap-2 pt-1">
            <Checkbox
              id="proxy-resolve-dns-remotely"
              checked={draft.proxyResolveDnsRemotely}
              disabled={!draft.proxyEnabled}
              onCheckedChange={(c) => onChange("proxyResolveDnsRemotely", c === true)}
            />
            <Label htmlFor="proxy-resolve-dns-remotely" className="text-xs font-normal cursor-pointer">
              Resolve DNS through proxy (Remote DNS)
            </Label>
          </div>
        )}

        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="proxy-username">Username (optional):</Label>
          <Input
            id="proxy-username"
            value={draft.proxyUsername}
            disabled={!draft.proxyEnabled || isDirect}
            className="w-56"
            autoComplete="off"
            onChange={(e) => onChange("proxyUsername", e.target.value)}
          />
        </div>

        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="proxy-password">Password (optional):</Label>
          <Input
            id="proxy-password"
            type="password"
            value={draft.proxyPassword}
            disabled={!draft.proxyEnabled || isDirect}
            className="w-56"
            autoComplete="off"
            onChange={(e) => onChange("proxyPassword", e.target.value)}
          />
        </div>

        <div className="flex items-center gap-4 pt-2">
          <Button
            type="button"
            variant="outline"
            id="test-proxy-button"
            disabled={!draft.proxyEnabled || testProxyMutation.isPending}
            onClick={() => void handleTestProxy()}
          >
            Test Connection
          </Button>
          <span
            id="proxy-test-status"
            className={`text-xs ${
              testStatus.type === "success"
                ? "text-green-600 dark:text-green-400"
                : testStatus.type === "error"
                  ? "text-destructive"
                  : "text-muted-foreground"
            }`}
            role="status"
            aria-live="polite"
          >
            {testStatus.message}
          </span>
        </div>
      </div>

      <ProxyDiscoveryDialog
        open={discoveryOpen}
        onOpenChange={setDiscoveryOpen}
        detected={detectedProxy}
        currentProxy={{
          enabled: draft.proxyEnabled,
          protocol: draft.proxyProtocol,
          host: draft.proxyHost,
          port: draft.proxyPort,
          username: draft.proxyUsername || null,
          password: draft.proxyPassword || null,
          resolveDnsRemotely: draft.proxyResolveDnsRemotely,
        }}
        onImport={handleImportedProxy}
      />
    </fieldset>
  );
}
