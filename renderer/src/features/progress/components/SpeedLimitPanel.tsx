import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
    <form className="space-y-3" onSubmit={(event) => void submit(event)}>
      <div className="space-y-2">
        <Label htmlFor="detail-speed-limit">Global speed limit (KB/s)</Label>
        <Input
          id="detail-speed-limit"
          type="number"
          min={0}
          max={10_000_000}
          required
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
      </div>
      <p className="text-xs text-muted-foreground">
        Applies to all downloads. Enter 0 for unlimited speed.
      </p>
      <div className="flex items-center gap-3">
        <Button type="submit" variant="outline" disabled={saving}>
          Apply
        </Button>
        {saved ? (
          <span role="status" className="text-xs text-muted-foreground">
            Saved.
          </span>
        ) : null}
      </div>
      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </form>
  );
}
