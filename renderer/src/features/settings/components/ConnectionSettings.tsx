import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  cleanHostPattern,
  type SettingsDraft,
} from "../model/settings-draft";

export interface ConnectionSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
  onError: (msg: string) => void;
}

export function ConnectionSettings({
  draft,
  onChange,
  onError,
}: ConnectionSettingsProps) {
  const [newHost, setNewHost] = useState("");
  const [newConnections, setNewConnections] = useState(1);

  function handleAddOverride() {
    const cleaned = cleanHostPattern(newHost);
    if (!cleaned) {
      onError("Please enter a valid host pattern (e.g. example.com or *.domain.com)");
      return;
    }
    const conns = Math.max(1, Math.min(32, Number(newConnections) || 1));
    onChange("connectionOverrides", [
      ...draft.connectionOverrides,
      { hostPattern: cleaned, connections: conns },
    ]);
    setNewHost("");
    setNewConnections(1);
  }

  function handleRemoveOverride(index: number) {
    const next = draft.connectionOverrides.filter((_, i) => i !== index);
    onChange("connectionOverrides", next);
  }

  return (
    <div className="space-y-6">
      <fieldset className="rounded-md border p-4 space-y-4">
        <legend className="px-2 text-sm font-semibold">Connection</legend>
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="max-concurrent">
            Maximum simultaneous downloads:
          </Label>
          <Input
            id="max-concurrent"
            type="number"
            min={1}
            max={16}
            className="w-24 text-right"
            value={draft.maxConcurrent}
            onChange={(e) =>
              onChange("maxConcurrent", Math.max(1, Number(e.target.value) || 1))
            }
            required
          />
        </div>
        <div className="space-y-1">
          <div className="flex items-center justify-between gap-4">
            <Label htmlFor="connections">Maximum connections per file:</Label>
            <Input
              id="connections"
              type="number"
              min={1}
              max={32}
              className="w-24 text-right"
              value={draft.connections}
              onChange={(e) =>
                onChange("connections", Math.max(1, Number(e.target.value) || 1))
              }
              required
            />
          </div>
          <p className="text-xs text-muted-foreground">
            Multiple connections require server support for byte ranges.
          </p>
        </div>
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="speed-limit">
            Speed limit (KB/s, 0 = unlimited):
          </Label>
          <Input
            id="speed-limit"
            type="number"
            min={0}
            max={1048576}
            className="w-32 text-right"
            value={draft.speedLimitKbps}
            onChange={(e) =>
              onChange(
                "speedLimitKbps",
                Math.max(0, Number(e.target.value) || 0),
              )
            }
            required
          />
        </div>
      </fieldset>

      <fieldset className="rounded-md border p-4 space-y-3">
        <legend className="px-2 text-sm font-semibold">
          Site Connection Exceptions
        </legend>
        <p className="text-xs text-muted-foreground">
          Configure per-domain connection limits (exact host or *.domain.com).
          Tip: Use 1 connection for servers that do not support resume.
        </p>
        <div className="flex gap-2 items-center">
          <Input
            id="new-override-host"
            placeholder="example.com or *.domain.com"
            value={newHost}
            onChange={(e) => setNewHost(e.target.value)}
          />
          <Input
            id="new-override-connections"
            type="number"
            min={1}
            max={32}
            className="w-20 text-right"
            value={newConnections}
            onChange={(e) =>
              setNewConnections(Math.max(1, Number(e.target.value) || 1))
            }
          />
          <Button
            type="button"
            variant="outline"
            id="add-override-button"
            onClick={handleAddOverride}
          >
            Add Exception
          </Button>
        </div>

        <div id="site-overrides-list" className="space-y-1.5 pt-2">
          {draft.connectionOverrides.length === 0 ? (
            <p className="text-xs text-muted-foreground italic">
              No site exceptions configured.
            </p>
          ) : (
            draft.connectionOverrides.map((override, index) => (
              <div
                key={`${override.hostPattern}-${index}`}
                className="flex items-center justify-between rounded border px-3 py-1.5 text-xs"
              >
                <span className="font-mono">{override.hostPattern}</span>
                <div className="flex items-center gap-3">
                  <span className="text-muted-foreground">
                    {override.connections} connection
                    {override.connections === 1 ? "" : "s"}
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="remove-override-button h-6 px-2 text-destructive"
                    data-index={index}
                    onClick={() => handleRemoveOverride(index)}
                  >
                    Remove
                  </Button>
                </div>
              </div>
            ))
          )}
        </div>
      </fieldset>

      <fieldset className="rounded-md border p-4 space-y-3">
        <legend className="px-2 text-sm font-semibold">
          Rolling Download Limits
        </legend>
        <div className="flex items-center gap-2">
          <Checkbox
            id="window-limit-enabled"
            checked={draft.windowLimitEnabled}
            onCheckedChange={(checked) =>
              onChange("windowLimitEnabled", checked === true)
            }
          />
          <Label
            htmlFor="window-limit-enabled"
            className="text-sm font-normal cursor-pointer"
          >
            Limit download volume over rolling time window
          </Label>
        </div>

        {draft.windowLimitEnabled && (
          <div id="window-limit-fields" className="space-y-3 pl-6 pt-1">
            <div className="flex items-center justify-between gap-4">
              <Label htmlFor="window-limit-max-mb">Max data volume (MB):</Label>
              <Input
                id="window-limit-max-mb"
                type="number"
                min={1}
                placeholder="2048"
                className="w-32 text-right"
                value={draft.windowLimitMaxMb}
                onChange={(e) =>
                  onChange(
                    "windowLimitMaxMb",
                    Math.max(1, Number(e.target.value) || 1),
                  )
                }
              />
            </div>
            <div className="flex items-center justify-between gap-4">
              <Label htmlFor="window-limit-hours">Time window (hours):</Label>
              <Input
                id="window-limit-hours"
                type="number"
                min={1}
                max={8760}
                placeholder="12"
                className="w-32 text-right"
                value={draft.windowLimitHours}
                onChange={(e) =>
                  onChange(
                    "windowLimitHours",
                    Math.max(1, Number(e.target.value) || 1),
                  )
                }
              />
            </div>
            {draft.windowLimitUsageText && (
              <div
                id="window-limit-status-line"
                className="text-xs text-muted-foreground"
              >
                <span id="window-limit-status">{draft.windowLimitUsageText}</span>
              </div>
            )}
          </div>
        )}
      </fieldset>
    </div>
  );
}
