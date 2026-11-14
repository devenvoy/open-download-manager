import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { type TorrentListItem } from "@/types/ipc";
import { stateKey, stateLabel } from "../model/torrent-view";

const TONE: Record<string, string> = {
  downloading: "bg-primary/10 text-primary border-primary/30",
  waiting: "bg-muted text-muted-foreground",
  seeding: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
  idle: "bg-muted text-muted-foreground",
  paused: "bg-amber-500/10 text-amber-600 border-amber-500/30 dark:text-amber-400",
  metadata: "bg-sky-500/10 text-sky-600 border-sky-500/30 dark:text-sky-400",
  error: "bg-destructive/10 text-destructive border-destructive/30",
  unknown: "bg-muted text-muted-foreground",
};

export function TorrentStateBadge({ torrent }: { torrent: TorrentListItem }) {
  const key = stateKey(torrent);
  return (
    <Badge variant="outline" className={cn("font-normal", TONE[key])}>
      {stateLabel(torrent)}
    </Badge>
  );
}
