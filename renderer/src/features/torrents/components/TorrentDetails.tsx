import { formatBytes, formatDate, formatSpeed } from "@/lib/format";
import {
  type TorrentFileItem,
  type TorrentListItem,
  type TorrentTrackerStatus,
} from "@/types/ipc";
import { useTorrentsStore } from "../state/torrents-store";
import {
  formatEta,
  formatRatio,
  progressPercent,
  stateLabel,
} from "../model/torrent-view";
import { TorrentFilesTab } from "./TorrentFilesTab";
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
    <div id="torrent-detail">
      <header className="torrent-detail-header">
        <h2 id="torrent-detail-name">{torrent.name || torrent.infoHash}</h2>
        <span id="torrent-detail-hash" className="torrent-hash">
          {torrent.infoHash}
        </span>
      </header>

      <div
        className="torrent-tabs"
        role="tablist"
        aria-label="Torrent detail sections"
      >
        <button
          type="button"
          className={`torrent-tab ${tab === "general" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "general"}
          onClick={() => setTab("general")}
        >
          General
        </button>
        <button
          type="button"
          className={`torrent-tab ${tab === "files" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "files"}
          onClick={() => setTab("files")}
        >
          Files{files.length ? ` (${files.length})` : ""}
        </button>
        <button
          type="button"
          className={`torrent-tab ${tab === "trackers" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "trackers"}
          onClick={() => setTab("trackers")}
        >
          Trackers
        </button>
      </div>

      {tab === "general" && (
        <div id="torrent-tab-general" className="torrent-tab-panel" role="tabpanel">
          {torrent.error && (
            <div id="torrent-detail-error" className="torrent-error-card">
              {torrent.error}
            </div>
          )}
          <progress
            id="torrent-detail-progress"
            className="torrent-detail-progress"
            max={100}
            value={progressPercent(torrent)}
          />
          <dl className="torrent-facts">
            <div>
              <dt>Status</dt>
              <dd id="torrent-fact-status">{stateLabel(torrent)}</dd>
            </div>
            <div>
              <dt>Size</dt>
              <dd id="torrent-fact-size">
                {torrent.totalBytes ? formatBytes(torrent.totalBytes) : "—"}
              </dd>
            </div>
            <div>
              <dt>Downloaded</dt>
              <dd id="torrent-fact-downloaded">
                {formatBytes(torrent.progressBytes)}
              </dd>
            </div>
            <div>
              <dt>Uploaded</dt>
              <dd id="torrent-fact-uploaded">
                {formatBytes(torrent.uploadedBytes)}
              </dd>
            </div>
            <div>
              <dt>Ratio</dt>
              <dd id="torrent-fact-ratio">{formatRatio(torrent.ratio)}</dd>
            </div>
            <div>
              <dt>Download speed</dt>
              <dd id="torrent-fact-down-speed">
                {torrent.downloadSpeed ? formatSpeed(torrent.downloadSpeed) : "—"}
              </dd>
            </div>
            <div>
              <dt>Upload speed</dt>
              <dd id="torrent-fact-up-speed">
                {torrent.uploadSpeed ? formatSpeed(torrent.uploadSpeed) : "—"}
              </dd>
            </div>
            <div>
              <dt>Peers</dt>
              <dd id="torrent-fact-peers">{torrent.peers}</dd>
            </div>
            <div>
              <dt>Swarm</dt>
              <dd id="torrent-fact-swarm">
                {`${torrent.seeds} seeds, ${torrent.leechers} leechers`}
              </dd>
            </div>
            <div>
              <dt>Time remaining</dt>
              <dd id="torrent-fact-eta">
                {formatEta(torrent.timeRemainingSecs)}
              </dd>
            </div>
            <div>
              <dt>Added</dt>
              <dd id="torrent-fact-added">
                {torrent.addedOn
                  ? formatDate(new Date(torrent.addedOn * 1000).toISOString())
                  : "—"}
              </dd>
            </div>
            <div className="torrent-fact-wide">
              <dt>Save to</dt>
              <dd id="torrent-fact-folder">{torrent.outputFolder || "—"}</dd>
            </div>
          </dl>
        </div>
      )}

      {tab === "files" && (
        <TorrentFilesTab
          files={files}
          loading={filesLoading}
          selection={selection}
          dirty={filesDirty}
          applying={applyingFiles}
          onToggle={onToggleFile}
          onSetAll={onSetAllFiles}
          onApply={onApplyFiles}
        />
      )}

      {tab === "trackers" && (
        <TorrentTrackersTab trackers={trackers} loading={trackersLoading} />
      )}
    </div>
  );
}
