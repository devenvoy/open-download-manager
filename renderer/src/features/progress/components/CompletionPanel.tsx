import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Separator } from "@/components/ui/separator";
import { type DownloadSummary, type Settings } from "@/types/ipc";
import { statusOf } from "../model/progress-view";
import { ActionLedger } from "./ActionLedger";

export interface CompletionPanelProps {
  download: DownloadSummary;
  settings: Settings;
  retrying: boolean;
  onSaveToggles: (patch: {
    notifyOnComplete: boolean;
    revealOnComplete: boolean;
  }) => Promise<void>;
  onRetryActions: () => void;
  onOpen: () => void;
  onReveal: () => void;
}

/** The "Options on completion" tab. */
export function CompletionPanel({
  download,
  settings,
  retrying,
  onSaveToggles,
  onRetryActions,
  onOpen,
  onReveal,
}: CompletionPanelProps) {
  const [notify, setNotify] = useState(settings.notifyOnComplete !== false);
  const [reveal, setReveal] = useState(settings.revealOnComplete === true);
  const [result, setResult] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const completed = statusOf(download) === "completed";

  async function submit(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setResult(null);
    try {
      await onSaveToggles({
        notifyOnComplete: notify,
        revealOnComplete: reveal,
      });
      setResult("Saved.");
    } catch (problem) {
      setResult(
        problem instanceof Error ? problem.message : "Could not save the options.",
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-3">
      <form className="space-y-3" onSubmit={(event) => void submit(event)}>
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={notify}
            onCheckedChange={(checked) => setNotify(checked === true)}
          />
          Show a notification when a download completes
        </label>
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={reveal}
            onCheckedChange={(checked) => setReveal(checked === true)}
          />
          Open the download folder on completion
        </label>
        <p className="text-xs text-muted-foreground">
          These options apply to all downloads.
        </p>
        <div className="flex items-center gap-3">
          <Button type="submit" variant="outline" disabled={saving}>
            Apply
          </Button>
          {result ? (
            <span role="status" className="text-xs text-muted-foreground">
              {result}
            </span>
          ) : null}
        </div>
      </form>

      <Separator />

      <ActionLedger
        entries={download.actionLedger}
        canRetry={completed}
        retrying={retrying}
        onRetry={onRetryActions}
      />

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          disabled={!completed}
          onClick={onOpen}
        >
          Open file
        </Button>
        <Button type="button" variant="outline" onClick={onReveal}>
          Open folder
        </Button>
      </div>
    </div>
  );
}
