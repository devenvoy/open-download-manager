import { type ActionLedgerEntry } from "@/types/ipc";

export interface ActionLedgerProps {
  entries: ActionLedgerEntry[] | undefined;
  canRetry: boolean;
  retrying: boolean;
  onRetry: () => void;
}

const STATUS_COLOR: Record<string, string> = {
  success: "#166534",
  failed: "#991b1b",
  cancelled: "#4b5563",
  skipped: "#4b5563",
};

const ICON: Record<string, string> = {
  success: "✓",
  failed: "⚠️",
  cancelled: "ℹ️",
  skipped: "ℹ️",
};

/** Post-download actions the engine ran, and the manual re-run button. */
export function ActionLedger({
  entries,
  canRetry,
  retrying,
  onRetry,
}: ActionLedgerProps) {
  return (
    <div className="completion-history-section">
      <strong>Post-Download Action History</strong>
      <div id="completion-action-history" style={{ marginTop: 6 }}>
        {!entries || entries.length === 0 ? (
          <p className="field-help" style={{ margin: "4px 0" }}>
            No post-download actions recorded yet.
          </p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: "4px 0" }}>
            {entries.map((entry, index) => {
              const color = STATUS_COLOR[entry.status] ?? "#4b5563";
              const icon = ICON[entry.status] ?? "ℹ️";
              return (
                <li
                  key={`${entry.actionKey}-${index}`}
                  style={{ padding: "3px 0", fontSize: "12px" }}
                >
                  <span style={{ color, fontWeight: "bold" }}>
                    {icon} {entry.actionType}
                  </span>
                  : <span>{entry.status}</span>
                  {entry.message ? <> — <em>{entry.message}</em></> : null}
                </li>
              );
            })}
          </ul>
        )}
      </div>
      {canRetry && (
        <button
          type="button"
          className="button small-button"
          id="detail-retry-actions"
          style={{ marginTop: 6 }}
          disabled={retrying}
          onClick={onRetry}
        >
          {retrying ? "Retrying…" : "Re-run completion actions"}
        </button>
      )}
    </div>
  );
}
