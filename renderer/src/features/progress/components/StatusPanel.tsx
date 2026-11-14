import { type DownloadSummary } from "@/types/ipc";
import { detailFacts, isErrorStatus, statusOf } from "../model/progress-view";
import { ErrorCard } from "./ErrorCard";

export interface StatusPanelProps {
  download: DownloadSummary;
  busy: boolean;
  onRetry: () => void;
}

/** The "Download status" tab: source, error card and the fact list. */
export function StatusPanel({ download, busy, onRetry }: StatusPanelProps) {
  const facts = detailFacts(download);
  const failed = isErrorStatus(statusOf(download));

  return (
    <div className="space-y-3">
      <p
        className="break-all text-xs text-muted-foreground"
        title={download.url}
      >
        {download.url || download.referer || ""}
      </p>

      {failed ? (
        <ErrorCard error={download.error} busy={busy} onRetry={onRetry} />
      ) : null}

      <dl className="grid grid-cols-[minmax(9rem,auto)_1fr] gap-x-4 gap-y-1 text-sm">
        {facts.map((fact) => (
          <div key={fact.label} className="contents">
            <dt className="text-muted-foreground">{fact.label}</dt>
            <dd className="break-words">{fact.value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
