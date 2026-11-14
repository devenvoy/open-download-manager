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
    <div id="status-panel" role="tabpanel" aria-labelledby="tab-status">
      <p
        id="transfer-source"
        className="transfer-source"
        title={download.url}
      >
        {download.url || download.referer || ""}
      </p>

      {failed ? (
        <ErrorCard error={download.error} busy={busy} onRetry={onRetry} />
      ) : null}

      <dl id="details-content" className="details-content">
        {facts.map((fact) => (
          <div key={fact.label} style={{ display: "contents" }}>
            <dt>{fact.label}</dt>
            <dd className={fact.label === "Source" ? "source-address" : undefined}>
              {fact.value}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
