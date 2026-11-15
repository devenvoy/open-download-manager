import { type TorrentFileItem, type TorrentReviewRequest } from "@/types/ipc";
import { reviewPlan } from "../model/torrent-view";

export interface TorrentReviewPanelProps {
  review: TorrentReviewRequest;
  files: TorrentFileItem[];
  loading: boolean;
  confirming: boolean;
  selection?: Set<number> | null;
  startPaused?: boolean;
  onConfirm: (included: number[], startPaused: boolean) => void;
  onCancel: () => void;
}

export function TorrentReviewPanel({
  review,
  files,
  loading,
  confirming,
  selection,
  startPaused = false,
  onConfirm,
  onCancel,
}: TorrentReviewPanelProps) {
  const plan = reviewPlan(files, selection, startPaused);
  const selectedCount = selection ? selection.size : files.length;
  const hint = loading
    ? "Fetching torrent metadata…"
    : plan.error
      ? plan.error
      : `${selectedCount} of ${files.length} file${files.length === 1 ? "" : "s"} ticked. Nothing is downloaded until you start it.`;

  return (
    <div
      id="torrent-review-panel"
      className="torrent-review-panel"
      role="region"
      aria-label="Torrent download review"
    >
      <div className="torrent-review-text">
        <strong id="torrent-review-name">
          {review.name || review.infoHash}
        </strong>
        <span id="torrent-review-hint">{hint}</span>
      </div>
      <div className="torrent-review-actions">
        <button
          type="button"
          id="torrent-review-start"
          className="button primary"
          disabled={loading || confirming || Boolean(plan.error)}
          onClick={() => onConfirm(plan.files, startPaused)}
        >
          Start download
        </button>
        <button
          type="button"
          id="torrent-review-cancel"
          className="button"
          onClick={onCancel}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
