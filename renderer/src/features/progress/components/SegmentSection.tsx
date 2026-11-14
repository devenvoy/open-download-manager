import { Progress } from "@/components/ui/progress";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatBytes } from "@/lib/format";
import { type DownloadSummary } from "@/types/ipc";
import {
  segmentCaption,
  segmentLabel,
  segmentNote,
  segmentStatus,
  segmentsOf,
  statusOf,
} from "../model/progress-view";

/** The connection/segment map below the transfer buttons. */
export function SegmentSection({ download }: { download: DownloadSummary }) {
  const segments = segmentsOf(download);
  const status = statusOf(download);
  if (segments.length === 0) return null;

  return (
    <section className="space-y-3" aria-label="Download connections">
      <p className="text-xs text-muted-foreground">{segmentCaption(download)}</p>

      <div className="flex flex-wrap gap-1" aria-hidden>
        {segments.map((segment, index) => {
          const total = Number(segment.total) || 0;
          const value = total
            ? Math.min(100, ((Number(segment.downloaded) || 0) / total) * 100)
            : 0;
          return (
            <Progress
              key={`${segment.start}-${index}`}
              value={value}
              className="h-2 min-w-16 flex-1"
            />
          );
        })}
      </div>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-10">N.</TableHead>
            <TableHead>Downloaded</TableHead>
            <TableHead>Info</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {segments.map((segment, index) => (
            <TableRow key={`${segment.start}-${index}`}>
              <TableCell>{segmentLabel(download, index)}</TableCell>
              <TableCell className="tabular-nums">
                {formatBytes(Number(segment.downloaded) || 0)}
              </TableCell>
              <TableCell>{segmentStatus(segment, status)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      <p className="text-xs text-muted-foreground">
        {segmentNote(download, segments.length)}
      </p>
    </section>
  );
}
