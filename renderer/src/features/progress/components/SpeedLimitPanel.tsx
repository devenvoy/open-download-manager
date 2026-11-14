import { useState, type FormEvent } from "react";
import { type Settings } from "@/types/ipc";

export interface SpeedLimitPanelProps {
  settings: Settings;
  onApply: (speedLimitKbps: number) => Promise<void>;
}

/** The "Speed Limiter" tab: one global limit shared by every download. */
export function SpeedLimitPanel({ settings, onApply }: SpeedLimitPanelProps) {
  const [value, setValue] = useState(String(settings.speedLimitKbps ?? 0));
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSaved(false);
    setSaving(true);
    try {
      await onApply(Number(value));
      setSaved(true);
    } catch (problem) {
      setError(
        problem instanceof Error ? problem.message : "Could not save the limit.",
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <div
      id="speed-panel"
      className="tab-panel"
      role="tabpanel"
      aria-labelledby="tab-speed"
    >
      <form id="detail-speed-form" onSubmit={(event) => void submit(event)}>
        <label htmlFor="detail-speed-limit">Global speed limit (KB/s):</label>
        <input
          id="detail-speed-limit"
          type="number"
          min={0}
          max={10_000_000}
          required
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
        <p className="field-help">
          Applies to all downloads. Enter 0 for unlimited speed.
        </p>
        <div>
          <button className="button" type="submit" disabled={saving}>
            Apply
          </button>
          {saved && (
            <span
              style={{
                marginLeft: 8,
                fontSize: 11,
                color: "var(--text-muted, #666)",
              }}
            >
              Saved.
            </span>
          )}
        </div>
        {error && (
          <p id="detail-speed-error" className="form-error">
            {error}
          </p>
        )}
      </form>
    </div>
  );
}
