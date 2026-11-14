import { formatBytes } from "@/lib/format";
import { type DownloadSummary } from "@/types/ipc";

export type SortField = "filename" | "total" | "status" | "createdAt";

export function getStatusLabel(d: DownloadSummary): string {
  const percent =
    d.total && d.total > 0
      ? (Number(d.downloaded) / Number(d.total)) * 100
      : null;

  switch (d.status) {
    case "complete":
      return "Complete";
    case "downloading":
      return percent !== null ? `${percent.toFixed(2)}%` : "Receiving data…";
    case "probing":
      return "Connecting…";
    case "assembling":
      return "Finalizing…";
    case "queued":
      return "Queued";
    case "scheduled":
      return "Scheduled";
    case "paused":
      return "Stopped";
    case "cancelled":
      return "Cancelled";
    case "error":
      return "Error";
    default:
      return d.status;
  }
}

export function getSizeText(d: DownloadSummary): string {
  if (d.status === "complete") {
    return formatBytes(d.total ?? d.downloaded);
  }
  if (d.total && d.total > 0) {
    return `${formatBytes(d.downloaded)} / ${formatBytes(d.total)}`;
  }
  return formatBytes(d.downloaded);
}

export function sortDownloads(
  downloads: DownloadSummary[],
  field: SortField,
  ascending: boolean,
): DownloadSummary[] {
  return [...downloads].sort((a, b) => {
    let cmp = 0;
    if (field === "filename") {
      cmp = a.filename.localeCompare(b.filename);
    } else if (field === "total") {
      cmp = Number(a.total || 0) - Number(b.total || 0);
    } else if (field === "status") {
      cmp = a.status.localeCompare(b.status);
    } else if (field === "createdAt") {
      cmp =
        new Date(a.createdAt || 0).getTime() -
        new Date(b.createdAt || 0).getTime();
    }
    return ascending ? cmp : -cmp;
  });
}
