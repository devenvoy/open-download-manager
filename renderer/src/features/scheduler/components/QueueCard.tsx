import { type DownloadQueue } from "@/types/ipc";
import { canDeleteQueue, queueMeta, queueTitle } from "../model/queue-view";

export interface QueueCardProps {
  queue: DownloadQueue;
  busy: boolean;
  onToggle: (queue: DownloadQueue) => void;
  onEdit: (queue: DownloadQueue) => void;
  onDelete: (queue: DownloadQueue) => void;
}

export function QueueCard({
  queue,
  busy,
  onToggle,
  onEdit,
  onDelete,
}: QueueCardProps) {
  return (
    <div className="queue-card">
      <div className="queue-card-info">
        <div className="queue-card-header">
          <span>{queueTitle(queue)}</span>
          <span className={`queue-badge ${queue.running ? "running" : ""}`}>
            {queue.running ? "Running" : "Paused"}
          </span>
        </div>
        <div className="queue-card-meta">{queueMeta(queue)}</div>
      </div>
      <div className="queue-card-actions">
        <button
          type="button"
          className="small-button"
          disabled={busy}
          onClick={() => onToggle(queue)}
        >
          {queue.running ? "Pause" : "Start"}
        </button>
        <button
          type="button"
          className="small-button"
          disabled={busy}
          onClick={() => onEdit(queue)}
        >
          Edit…
        </button>
        {canDeleteQueue(queue) ? (
          <button
            type="button"
            className="small-button"
            disabled={busy}
            onClick={() => onDelete(queue)}
          >
            Delete
          </button>
        ) : null}
      </div>
    </div>
  );
}
