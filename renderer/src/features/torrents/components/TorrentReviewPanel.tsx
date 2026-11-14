import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { formatBytes } from "@/lib/format";
import { type TorrentFileItem, type TorrentReviewRequest } from "@/types/ipc";
import { reviewPlan } from "../model/torrent-view";

export interface TorrentReviewPanelProps {
  review: TorrentReviewRequest;
  files: TorrentFileItem[];
  loading: boolean;
  confirming: boolean;
  onConfirm: (included: number[], startPaused: boolean) => void;
  onCancel: () => void;
}

/**
 * The file picker shown after a torrent was added in review mode.
 *
 * Nothing is transferred until the user answers: `confirm` enables exactly the
 * ticked files and then resumes or pauses the torrent.
 */
export function TorrentReviewPanel({
  review,
  files,
  loading,
  confirming,
  onConfirm,
  onCancel,
}: TorrentReviewPanelProps) {
  const [selection, setSelection] = useState<Set<number> | null>(null);
  const [startPaused, setStartPaused] = useState(false);

  const isIncluded = (file: TorrentFileItem) =>
    selection ? selection.has(file.index) : file.included;

  function toggle(index: number) {
    setSelection((current) => {
      const next = new Set(current ?? files.filter((file) => file.included).map((file) => file.index));
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }

  function setAll(selected: boolean) {
    setSelection(selected ? new Set(files.map((file) => file.index)) : new Set());
  }

  function confirm() {
    const plan = reviewPlan(files, selection, startPaused);
    if (plan.error) return;
    onConfirm(plan.files, startPaused);
  }

  const plan = reviewPlan(files, selection, startPaused);

  return (
    <section className="space-y-3 rounded-md border border-primary/30 bg-primary/5 p-3">
      <header>
        <h2 className="text-sm font-semibold">Choose files to download</h2>
        <p className="truncate text-xs text-muted-foreground" title={review.name}>
          {review.name}
        </p>
      </header>

      {loading ? (
        <p className="text-sm text-muted-foreground">Fetching the file list…</p>
      ) : files.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          The engine has not published a file list yet.
        </p>
      ) : (
        <>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="outline" onClick={() => setAll(true)}>
              Select all
            </Button>
            <Button size="sm" variant="outline" onClick={() => setAll(false)}>
              Select none
            </Button>
            <label className="ml-auto flex items-center gap-2 text-sm">
              <Checkbox
                checked={startPaused}
                onCheckedChange={(checked) => setStartPaused(checked === true)}
              />
              Add paused
            </label>
          </div>

          <ul className="max-h-64 divide-y divide-border overflow-auto rounded-md border border-border bg-background">
            {files.map((file) => (
              <li key={file.index} className="flex items-center gap-3 px-3 py-2">
                <Checkbox
                  checked={isIncluded(file)}
                  onCheckedChange={() => toggle(file.index)}
                  aria-label={`Include ${file.name}`}
                />
                <span className="min-w-0 flex-1 truncate text-sm" title={file.name}>
                  {file.name}
                </span>
                <span className="text-xs tabular-nums text-muted-foreground">
                  {formatBytes(file.length)}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}

      {plan.error && files.length > 0 ? (
        <p role="alert" className="text-sm text-destructive">
          {plan.error}
        </p>
      ) : null}

      <div className="flex justify-end gap-2">
        <Button variant="outline" onClick={onCancel} disabled={confirming}>
          Cancel download
        </Button>
        <Button onClick={confirm} disabled={confirming || Boolean(plan.error)}>
          Start download
        </Button>
      </div>
    </section>
  );
}
