import { Button } from "@/components/ui/button";
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
    <div className="space-y-4">
      <section className="space-y-2">
        <h2 className="text-sm font-semibold">Scheduled downloads</h2>
        {sorted.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No scheduled downloads.
          </p>
        ) : (
          <ul className="divide-y divide-border rounded-md border border-border">
            {sorted.map((download) => (
              <li
                key={download.id}
                className="flex items-center gap-3 px-3 py-2 text-sm"
              >
                <span className="min-w-0 flex-1 truncate" title={download.filename}>
                  {download.filename}
                </span>
                <time className="shrink-0 text-xs tabular-nums text-muted-foreground">
                  {formatWhen(download.scheduledAt)}
                </time>
              </li>
            ))}
          </ul>
        )}
        <p className="text-xs text-muted-foreground">
          Scheduled downloads start at the chosen local time while this app is
          running.
        </p>
      </section>

      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" onClick={onAdd}>
          Add scheduled download…
        </Button>
        <Button type="button" variant="outline" disabled={busy} onClick={onStartDefault}>
          Start Default Queue
        </Button>
        <Button type="button" variant="outline" disabled={busy} onClick={onStopDefault}>
          Stop Default Queue
        </Button>
      </div>
    </div>
  );
}
