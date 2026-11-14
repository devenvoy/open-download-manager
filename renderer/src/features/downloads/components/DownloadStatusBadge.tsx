import { Badge } from "@/components/ui/badge";
import { type DownloadStatus } from "@/types/ipc";

type BadgeVariant = "default" | "secondary" | "destructive" | "outline";

const LABELS: Record<DownloadStatus, string> = {
  queued: "Queued",
  scheduled: "Scheduled",
  paused: "Paused",
  cancelled: "Cancelled",
  downloading: "Downloading",
  probing: "Probing",
  assembling: "Assembling",
  complete: "Complete",
  error: "Error",
  skipped: "Skipped",
};

const VARIANTS: Record<DownloadStatus, BadgeVariant> = {
  queued: "secondary",
  scheduled: "outline",
  paused: "outline",
  cancelled: "outline",
  downloading: "default",
  probing: "secondary",
  assembling: "secondary",
  complete: "default",
  error: "destructive",
  skipped: "outline",
};

export function DownloadStatusBadge({ status }: { status: DownloadStatus }) {
  return <Badge variant={VARIANTS[status]}>{LABELS[status]}</Badge>;
}
