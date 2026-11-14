import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatBytes, formatSpeed } from "@/lib/format";
import { type TorrentListItem } from "@/types/ipc";
import { cn } from "@/lib/utils";
import {
  formatEta,
  formatRatio,
  progressPercent,
  stateShortLabel,
} from "../model/torrent-view";

export interface TorrentListProps {
  torrents: TorrentListItem[];
  selectedId: number | null;
  onSelect: (id: number) => void;
}

export function TorrentList({ torrents, selectedId, onSelect }: TorrentListProps) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="min-w-56">Name</TableHead>
          <TableHead className="w-20 text-right">Size</TableHead>
          <TableHead className="w-24 text-right">Done</TableHead>
          <TableHead className="w-24 text-right">Down</TableHead>
          <TableHead className="w-24 text-right">Up</TableHead>
          <TableHead className="w-16 text-right">Peers</TableHead>
          <TableHead className="w-16 text-right">Ratio</TableHead>
          <TableHead className="w-24 text-right">ETA</TableHead>
          <TableHead className="w-32">Status</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {torrents.map((torrent) => {
          const selected = torrent.id === selectedId;
          return (
            <TableRow
              key={`${torrent.infoHash}-${torrent.id}`}
              data-state={selected ? "selected" : undefined}
              onClick={() => onSelect(torrent.id)}
              className={cn("cursor-pointer", selected && "bg-muted/60")}
            >
              <TableCell className="max-w-0">
                <span className="block truncate font-medium">{torrent.name}</span>
                {torrent.error ? (
                  <span className="block truncate text-xs text-destructive">
                    {torrent.error}
                  </span>
                ) : null}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {torrent.totalBytes ? formatBytes(torrent.totalBytes) : "—"}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {progressPercent(torrent).toFixed(1)}%
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {torrent.downloadSpeed ? formatSpeed(torrent.downloadSpeed) : "—"}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {torrent.uploadSpeed ? formatSpeed(torrent.uploadSpeed) : "—"}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {torrent.peers}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {formatRatio(torrent.ratio)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {formatEta(torrent.timeRemainingSecs)}
              </TableCell>
              <TableCell className="text-xs text-muted-foreground">
                {stateShortLabel(torrent)}
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
