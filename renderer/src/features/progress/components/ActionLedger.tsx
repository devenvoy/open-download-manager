import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { type ActionLedgerEntry } from "@/types/ipc";

export interface ActionLedgerProps {
  entries: ActionLedgerEntry[] | undefined;
  canRetry: boolean;
  retrying: boolean;
  onRetry: () => void;
}

const TONE: Record<string, string> = {
  success: "text-emerald-600 dark:text-emerald-400",
  failed: "text-destructive",
  cancelled: "text-muted-foreground",
  skipped: "text-muted-foreground",
};

const ICON: Record<string, string> = {
  success: "✓",
  failed: "⚠",
  cancelled: "ℹ",
  skipped: "ℹ",
};

/** Post-download actions the engine ran, and the manual re-run button. */
export function ActionLedger({
  entries,
  canRetry,
  retrying,
  onRetry,
}: ActionLedgerProps) {
  if (!entries || entries.length === 0) {
    return (
      <div className="space-y-2">
        <h3 className="text-sm font-semibold">Post-Download Action History</h3>
        <p className="text-xs text-muted-foreground">
          No post-download actions recorded yet.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <h3 className="text-sm font-semibold">Post-Download Action History</h3>
      <ul className="divide-y divide-border rounded-md border border-border">
        {entries.map((entry, index) => (
          <li
            key={`${entry.actionKey}-${index}`}
            className="flex flex-wrap items-baseline gap-x-2 gap-y-1 px-3 py-2 text-xs"
          >
            <span className={`font-semibold ${TONE[entry.status] ?? ""}`}>
              {ICON[entry.status] ?? "ℹ"} {entry.actionType}
            </span>
            <Badge variant="outline">{entry.status}</Badge>
            {entry.message ? (
              <em className="text-muted-foreground">{entry.message}</em>
            ) : null}
          </li>
        ))}
      </ul>
      {canRetry ? (
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={retrying}
          onClick={onRetry}
        >
          Re-run completion actions
        </Button>
      ) : null}
    </div>
  );
}
