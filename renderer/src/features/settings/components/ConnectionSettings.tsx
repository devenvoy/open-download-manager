import { useState } from "react";
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
      onError(
        "Please enter a valid host pattern (e.g. example.com or *.domain.com)",
      );
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
    <>
      <fieldset>
        <legend>Connection</legend>
        <div className="form-line">
          <label htmlFor="max-concurrent">
            Maximum simultaneous downloads:
          </label>
          <input
            type="number"
            id="max-concurrent"
            min={1}
            max={16}
            required
            value={draft.maxConcurrent}
            onChange={(e) =>
              onChange(
                "maxConcurrent",
                Math.max(1, Number(e.target.value) || 1),
              )
            }
          />
        </div>
        <div className="form-line">
          <label htmlFor="connections">Maximum connections per file:</label>
          <input
            type="number"
            id="connections"
            min={1}
            max={32}
            required
            value={draft.connections}
            onChange={(e) =>
              onChange(
                "connections",
                Math.max(1, Number(e.target.value) || 1),
              )
            }
          />
        </div>
        <p className="field-help">
          Multiple connections require server support for byte ranges.
        </p>
        <div className="form-line">
          <label htmlFor="speed-limit">
            Speed limit (KB/s, 0 = unlimited):
          </label>
          <input
            type="number"
            id="speed-limit"
            min={0}
            max={1048576}
            required
            value={draft.speedLimitKbps}
            onChange={(e) =>
              onChange("speedLimitKbps", Number(e.target.value) || 0)
            }
          />
        </div>
      </fieldset>

      <fieldset>
        <legend>Site Connection Exceptions</legend>
        <p className="field-help">
          Configure per-domain connection limits (exact host or *.domain.com).
          Tip: Use 1 connection for servers that do not support resume.
        </p>
        <div className="form-line site-override-inputs">
          <input
            type="text"
            id="new-override-host"
            placeholder="example.com or *.domain.com"
            value={newHost}
            onChange={(e) => setNewHost(e.target.value)}
          />
          <input
            type="number"
            id="new-override-connections"
            min={1}
            max={32}
            value={newConnections}
            title="Connections"
            onChange={(e) =>
              setNewConnections(Number(e.target.value) || 1)
            }
          />
          <button
            type="button"
            id="add-override-button"
            className="small-button"
            onClick={handleAddOverride}
          >
            Add Exception
          </button>
        </div>
        <div id="site-overrides-list" className="site-overrides-list">
          {draft.connectionOverrides.map((item, idx) => (
            <div
              key={idx}
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "3px 0",
              }}
            >
              <span>
                <strong>{item.hostPattern}</strong>: {item.connections}{" "}
                connection(s)
              </span>
              <button
                type="button"
                className="small-button"
                onClick={() => handleRemoveOverride(idx)}
              >
                Remove
              </button>
            </div>
          ))}
        </div>
      </fieldset>

      <fieldset>
        <legend>Rolling Download Limits</legend>
        <label className="checkbox-label" htmlFor="window-limit-enabled">
          <input
            type="checkbox"
            id="window-limit-enabled"
            checked={draft.windowLimitEnabled}
            onChange={(e) => onChange("windowLimitEnabled", e.target.checked)}
          />
          Limit download volume over rolling time window
        </label>
        {draft.windowLimitEnabled && (
          <div id="window-limit-fields" className="window-limit-fields">
            <div className="form-line">
              <label htmlFor="window-limit-max-mb">Max data volume (MB):</label>
              <input
                type="number"
                id="window-limit-max-mb"
                min={1}
                placeholder="2048"
                value={draft.windowLimitMaxMb}
                onChange={(e) =>
                  onChange(
                    "windowLimitMaxMb",
                    Number(e.target.value) || 0,
                  )
                }
              />
            </div>
            <div className="form-line">
              <label htmlFor="window-limit-hours">Time window (hours):</label>
              <input
                type="number"
                id="window-limit-hours"
                min={1}
                max={8760}
                placeholder="12"
                value={draft.windowLimitHours}
                onChange={(e) =>
                  onChange(
                    "windowLimitHours",
                    Number(e.target.value) || 0,
                  )
                }
              />
            </div>
          </div>
        )}
      </fieldset>
    </>
  );
}
