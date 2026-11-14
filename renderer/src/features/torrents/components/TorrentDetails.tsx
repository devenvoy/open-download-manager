import { Progress } from "@/components/ui/progress";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { formatBytes, formatSpeed } from "@/lib/format";
import { type TorrentFileItem, type TorrentListItem, type TorrentTrackerStatus } from "@/types/ipc";
import { useTorrentsStore } from "../state/torrents-store";
import {
  formatEta,
  formatRatio,
  progressPercent,
} from "../model/torrent-view";
import { TorrentFilesTab } from "./TorrentFilesTab";
import { TorrentStateBadge } from "./TorrentStateBadge";
import { TorrentTrackersTab } from "./TorrentTrackersTab";

export interface TorrentDetailsProps {
  torrent: TorrentListItem;
  files: TorrentFileItem[];
  filesLoading: boolean;
  trackers: TorrentTrackerStatus[];
  trackersLoading: boolean;
  filesDirty: boolean;
  applyingFiles: boolean;
  onToggleFile: (index: number) => void;
  onSetAllFiles: (selected: boolean) => void;
  onApplyFiles: () => void;
}

export function TorrentDetails({
  torrent,
  files,
  filesLoading,
  trackers,
  trackersLoading,
  filesDirty,
  applyingFiles,
  onToggleFile,
  onSetAllFiles,
  onApplyFiles,
}: TorrentDetailsProps) {
  const tab = useTorrentsStore((state) => state.tab);
  const setTab = useTorrentsStore((state) => state.setTab);
  const selection = useTorrentsStore((state) => state.fileSelection);

  return (
    <section className="flex min-h-0 flex-col gap-3 rounded-md border border-border p-3">
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold" title={torrent.name}>
            {torrent.name}
          </h2>
          <p className="truncate text-xs text-muted-foreground">
            {torrent.infoHash}
          </p>
        </div>
        <TorrentStateBadge torrent={torrent} />
      </header>

      <div className="space-y-1">
        <Progress value={progressPercent(torrent)} className="h-2" />
        <div className="flex justify-between text-xs tabular-nums text-muted-foreground">
          <span>{progressPercent(torrent).toFixed(1)}%</span>
          <span>
            {formatBytes(torrent.progressBytes)} /{" "}
            {torrent.totalBytes ? formatBytes(torrent.totalBytes) : "unknown"}
          </span>
        </div>
      </div>

      <Tabs value={tab} onValueChange={(value) => setTab(value as typeof tab)}>
        <TabsList>
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="files">
            Files{files.length ? ` (${files.length})` : ""}
          </TabsTrigger>
          <TabsTrigger value="trackers">Trackers</TabsTrigger>
        </TabsList>

        <TabsContent value="general" className="pt-2">
          <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm sm:grid-cols-3">
            <Fact label="Size" value={torrent.totalBytes ? formatBytes(torrent.totalBytes) : "—"} />
            <Fact label="Downloaded" value={formatBytes(torrent.progressBytes)} />
            <Fact label="Uploaded" value={formatBytes(torrent.uploadedBytes)} />
            <Fact label="Ratio" value={formatRatio(torrent.ratio)} />
            <Fact label="Down speed" value={formatSpeed(torrent.downloadSpeed)} />
            <Fact label="Up speed" value={formatSpeed(torrent.uploadSpeed)} />
            <Fact label="Seeds" value={String(torrent.seeds)} />
            <Fact label="Leechers" value={String(torrent.leechers)} />
            <Fact label="Peers" value={String(torrent.peers)} />
            <Fact label="ETA" value={formatEta(torrent.timeRemainingSecs)} />
            <Fact
              label="Added"
              value={
                torrent.addedOn
                  ? new Date(torrent.addedOn * 1000).toLocaleString()
                  : "—"
              }
            />
            <Fact label="Folder" value={torrent.outputFolder} />
          </dl>
          {torrent.error ? (
            <p role="alert" className="mt-3 text-sm text-destructive">
              {torrent.error}
            </p>
          ) : null}
        </TabsContent>

        <TabsContent value="files" className="pt-2">
          {filesLoading ? (
            <p className="text-sm text-muted-foreground">Reading files…</p>
          ) : (
            <TorrentFilesTab
              files={files}
              selection={selection}
              dirty={filesDirty}
              applying={applyingFiles}
              onToggle={onToggleFile}
              onSetAll={onSetAllFiles}
              onApply={onApplyFiles}
            />
          )}
        </TabsContent>

        <TabsContent value="trackers" className="pt-2">
          <TorrentTrackersTab trackers={trackers} loading={trackersLoading} />
        </TabsContent>
      </Tabs>
    </section>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="text-xs uppercase tracking-wide text-muted-foreground">
        {label}
      </dt>
      <dd className="truncate" title={value}>
        {value}
      </dd>
    </div>
  );
}
