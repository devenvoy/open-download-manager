import { type DownloadSummary } from "@/types/ipc";
import { formatWhen } from "../model/queue-view";

export interface ScheduledListProps {
  downloads: DownloadSummary[];
  onAdd: () => void;
  onStartDefault: () => void;
  onStopDefault: () => void;
  busy: boolean;
}

export function ScheduledList({
  downloads,
  onAdd,
  onStartDefault,
  onStopDefault,
  busy,
}: ScheduledListProps) {
  const sorted = [...downloads].sort(
    (left, right) =>
      new Date(left.scheduledAt ?? 0).getTime() -
      new Date(right.scheduledAt ?? 0).getTime(),
  );

  return (
    <>
      <fieldset>
        <legend>Scheduled downloads</legend>
        <div id="scheduled-list" className="scheduled-list">
          {sorted.length === 0 ? (
            <p className="field-help" style={{ padding: "8px 0" }}>
              No scheduled downloads.
            </p>
          ) : (
            sorted.map((download) => (
              <div
                key={download.id}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  padding: "5px 4px",
                  borderBottom: "1px solid var(--border-color, #e5e5e5)",
                }}
              >
                <span title={download.filename} style={{ fontWeight: 500 }}>
                  {download.filename}
                </span>
                <time style={{ color: "#666", fontSize: "11px" }}>
                  {formatWhen(download.scheduledAt)}
                </time>
              </div>
            ))
          )}
        </div>
      </fieldset>
      <p className="field-help">
        Scheduled downloads start at the chosen local time while this app is
        running.
      </p>
      <div className="scheduler-actions">
        <button
          className="button"
          id="schedule-new"
          type="button"
          onClick={onAdd}
        >
          Add scheduled download…
        </button>
        <button
          className="button"
          id="scheduler-start"
          type="button"
          disabled={busy}
          onClick={onStartDefault}
        >
          Start Default Queue
        </button>
        <button
          className="button"
          id="scheduler-stop"
          type="button"
          disabled={busy}
          onClick={onStopDefault}
        >
          Stop Default Queue
        </button>
      </div>
    </>
  );
}
