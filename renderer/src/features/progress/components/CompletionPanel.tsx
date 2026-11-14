import { useState, type FormEvent } from "react";
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
    <div
      id="completion-panel"
      className="tab-panel"
      role="tabpanel"
      aria-labelledby="tab-completion"
    >
      <form id="completion-form" onSubmit={(event) => void submit(event)}>
        <label className="checkbox-label" htmlFor="notify-complete">
          <input
            id="notify-complete"
            type="checkbox"
            checked={notify}
            onChange={(e) => setNotify(e.target.checked)}
          />
          Show a Windows notification when a download completes
        </label>
        <label className="checkbox-label" htmlFor="reveal-complete">
          <input
            id="reveal-complete"
            type="checkbox"
            checked={reveal}
            onChange={(e) => setReveal(e.target.checked)}
          />
          Open the download folder on completion
        </label>
        <p className="field-help">These options apply to all downloads.</p>
        <div>
          <button className="button" type="submit" disabled={saving}>
            Apply
          </button>
          {result && (
            <span
              id="completion-result"
              role="status"
              style={{ marginLeft: 8, fontSize: 11 }}
            >
              {result}
            </span>
          )}
        </div>
      </form>

      <hr
        style={{
          margin: "12px 0",
          border: "none",
          borderTop: "1px solid var(--border-color, #e5e5e5)",
        }}
      />

      <ActionLedger
        entries={download.actionLedger}
        canRetry={completed}
        retrying={retrying}
        onRetry={onRetryActions}
      />

      <p style={{ marginTop: 12 }}>
        <button
          type="button"
          className="button"
          id="detail-open"
          onClick={onOpen}
        >
          Open file
        </button>
        <button
          type="button"
          className="button"
          id="detail-folder"
          onClick={onReveal}
          style={{ marginLeft: 6 }}
        >
          Open folder
        </button>
      </p>
    </div>
  );
}
