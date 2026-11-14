import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { type TorrentTrackerStatus } from "@/types/ipc";

export function TorrentTrackersTab({
  trackers,
  loading,
}: {
  trackers: TorrentTrackerStatus[];
  loading: boolean;
}) {
  if (loading) {
    return <p className="text-sm text-muted-foreground">Reading trackers…</p>;
  }

  if (trackers.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        This torrent has no trackers. DHT and peer exchange are used instead.
      </p>
    );
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Tracker</TableHead>
          <TableHead className="w-28">State</TableHead>
          <TableHead className="w-20 text-right">Seeders</TableHead>
          <TableHead className="w-20 text-right">Leechers</TableHead>
          <TableHead className="w-20 text-right">Peers</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {trackers.map((tracker) => (
          <TableRow key={tracker.url}>
            <TableCell className="max-w-0">
              <span className="block truncate" title={tracker.url}>
                {tracker.url}
              </span>
              {tracker.lastError ? (
                <span className="block truncate text-xs text-destructive">
                  {tracker.lastError}
                </span>
              ) : null}
            </TableCell>
            <TableCell className="text-xs text-muted-foreground">
              {tracker.state.replaceAll("_", " ")}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {tracker.seeders ?? "—"}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {tracker.leechers ?? "—"}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {tracker.peersReturned ?? "—"}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
