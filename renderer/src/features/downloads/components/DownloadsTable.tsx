import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatBytes, formatPercent, formatSpeed } from "@/lib/format";
import {
  type DownloadActionKind,
  type DownloadStatus,
  type DownloadSummary,
} from "@/types/ipc";
import { DownloadStatusBadge } from "./DownloadStatusBadge";

export interface DownloadsTableProps {
  downloads: DownloadSummary[];
  selectedIds: string[];
  onToggle: (id: string) => void;
  onToggleAll: (checked: boolean) => void;
  onAction: (id: string, action: DownloadActionKind) => void;
}

const ROW_ACTIONS: Record<
  DownloadStatus,
  { label: string; action: DownloadActionKind }
> = {
  queued: { label: "Resume", action: "resume" },
  scheduled: { label: "Resume", action: "resume" },
  paused: { label: "Resume", action: "resume" },
  cancelled: { label: "Retry", action: "retry" },
  downloading: { label: "Pause", action: "pause" },
  probing: { label: "Pause", action: "pause" },
  assembling: { label: "Pause", action: "pause" },
  complete: { label: "Open", action: "open" },
  error: { label: "Retry", action: "retry" },
  skipped: { label: "Retry", action: "retry" },
};

export function DownloadsTable({
  downloads,
  selectedIds,
  onToggle,
  onToggleAll,
  onAction,
}: DownloadsTableProps) {
  const allSelected =
    downloads.length > 0 &&
    downloads.every((download) => selectedIds.includes(download.id));

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="w-10">
            <Checkbox
              aria-label="Select all downloads"
              checked={allSelected}
              onCheckedChange={(checked) => onToggleAll(checked === true)}
            />
          </TableHead>
          <TableHead>File</TableHead>
          <TableHead>Category</TableHead>
          <TableHead className="text-right">Size</TableHead>
          <TableHead className="text-right">Speed</TableHead>
          <TableHead>Status</TableHead>
          <TableHead className="w-24 text-right">Actions</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {downloads.map((download) => {
          const rowAction = ROW_ACTIONS[download.status];
          const active = rowAction.action === "pause";
          return (
            <TableRow
              key={download.id}
              data-state={
                selectedIds.includes(download.id) ? "selected" : undefined
              }
            >
              <TableCell>
                <Checkbox
                  aria-label={`Select ${download.filename}`}
                  checked={selectedIds.includes(download.id)}
                  onCheckedChange={() => onToggle(download.id)}
                />
              </TableCell>
              <TableCell className="max-w-[22rem] truncate">
                {download.filename}
              </TableCell>
              <TableCell className="text-muted-foreground">
                {download.category ?? "Other"}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {download.total
                  ? `${formatBytes(download.downloaded)} / ${formatBytes(download.total)}`
                  : formatBytes(download.downloaded)}
                <span className="ml-2 text-muted-foreground">
                  {formatPercent(download.downloaded, download.total)}
                </span>
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {active ? formatSpeed(download.speed) : "—"}
              </TableCell>
              <TableCell>
                <DownloadStatusBadge status={download.status} />
              </TableCell>
              <TableCell className="text-right">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => onAction(download.id, rowAction.action)}
                >
                  {rowAction.label}
                </Button>
              </TableCell>
            </TableRow>
          );
        })}
        {downloads.length === 0 ? (
          <TableRow>
            <TableCell
              colSpan={7}
              className="h-24 text-center text-muted-foreground"
            >
              No downloads yet.
            </TableCell>
          </TableRow>
        ) : null}
      </TableBody>
    </Table>
  );
}
