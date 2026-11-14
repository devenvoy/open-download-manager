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

export function TorrentReviewPanel({
  review,
  files,
  loading,
  confirming,
  onConfirm,
  onCancel,
}: TorrentReviewPanelProps) {
  const plan = reviewPlan(files, null, false);

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
        <span id="torrent-review-hint">
          {loading
            ? "Fetching torrent metadata…"
            : plan.error
              ? plan.error
              : `Review files before starting (${files.length} file${files.length === 1 ? "" : "s"} found).`}
        </span>
      </div>
      <div className="torrent-review-actions">
        <button
          type="button"
          id="torrent-review-start"
          className="button primary"
          disabled={loading || confirming || Boolean(plan.error)}
          onClick={() => onConfirm(plan.files, false)}
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
