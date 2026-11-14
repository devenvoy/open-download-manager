import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
    <li className="rounded-md border border-border p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="truncate font-medium">{queueTitle(queue)}</span>
            <Badge
              variant="outline"
              className={
                queue.running
                  ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                  : "bg-muted text-muted-foreground"
              }
            >
              {queue.running ? "Running" : "Paused"}
            </Badge>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{queueMeta(queue)}</p>
        </div>
        <div className="flex shrink-0 gap-2">
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => onToggle(queue)}
          >
            {queue.running ? "Pause" : "Start"}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => onEdit(queue)}
          >
            Edit…
          </Button>
          {canDeleteQueue(queue) ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => onDelete(queue)}
            >
              Delete
            </Button>
          ) : null}
        </div>
      </div>
    </li>
  );
}
