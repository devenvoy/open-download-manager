import { Info, FolderTree, Users, Radio, LayoutGrid } from "lucide-react";
import { formatBytes, formatDate, formatSpeed } from "@/lib/format";
import {
  type TorrentFileItem,
  type TorrentListItem,
  type TorrentPeerItem,
  type TorrentPiecesInfo,
  type TorrentReviewRequest,
  type TorrentTrackerStatus,
} from "@/types/ipc";
import { useTorrentsStore } from "../state/torrents-store";
import {
  formatEta,
  formatRatio,
  progressPercent,
  stateKey,
  stateLabel,
} from "../model/torrent-view";
import { TorrentFilesTab } from "./TorrentFilesTab";
import { TorrentPeersTab } from "./TorrentPeersTab";
import { TorrentPiecesTab } from "./TorrentPiecesTab";
import { TorrentReviewPanel } from "./TorrentReviewPanel";
import { TorrentTrackersTab } from "./TorrentTrackersTab";

export interface TorrentDetailsProps {
  torrent: TorrentListItem;
  files: TorrentFileItem[];
  filesLoading: boolean;
  trackers: TorrentTrackerStatus[];
  trackersLoading: boolean;
  peers?: TorrentPeerItem[];
  peersLoading?: boolean;
  piecesInfo?: TorrentPiecesInfo;
  piecesLoading?: boolean;
  filesDirty: boolean;
  applyingFiles: boolean;
  review?: TorrentReviewRequest | null;
  reviewConfirming?: boolean;
  startPaused?: boolean;
  onConfirmReview?: (included: number[], startPaused: boolean) => void;
  onCancelReview?: () => void;
  onToggleFile: (index: number) => void;
  onSetAllFiles: (selected: boolean) => void;
  onApplyFiles: () => void;
  onStopSeeding?: () => void;
}

export function TorrentDetails({
  torrent,
  files,
  filesLoading,
  trackers,
  trackersLoading,
  peers = [],
  peersLoading = false,
  piecesInfo,
  piecesLoading = false,
  filesDirty,
  applyingFiles,
  review,
  reviewConfirming,
  startPaused,
  onConfirmReview,
  onCancelReview,
  onToggleFile,
  onSetAllFiles,
  onApplyFiles,
  onStopSeeding,
}: TorrentDetailsProps) {
  const tab = useTorrentsStore((state) => state.tab);
  const setTab = useTorrentsStore((state) => state.setTab);
  const selection = useTorrentsStore((state) => state.fileSelection);
  const percent = progressPercent(torrent);
  const isComplete = percent >= 100;
  const isSeeding = stateKey(torrent) === "seeding";

  return (
    <div id="torrent-detail">
      <header className="torrent-detail-header">
        <h2 id="torrent-detail-name">{torrent.name || torrent.infoHash}</h2>
        <span id="torrent-detail-hash" className="torrent-hash">
          {torrent.infoHash}
        </span>
      </header>

      {review && review.id === torrent.id && onConfirmReview && onCancelReview ? (
        <TorrentReviewPanel
          review={review}
          files={files}
          loading={filesLoading}
          confirming={Boolean(reviewConfirming)}
          selection={selection}
          startPaused={startPaused}
          onConfirm={onConfirmReview}
          onCancel={onCancelReview}
        />
      ) : null}

      <div
        className="torrent-tabs"
        role="tablist"
        aria-label="Torrent detail sections"
      >
        <button
          type="button"
          className={`torrent-tab flex items-center gap-1.5 ${tab === "general" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "general"}
          onClick={() => setTab("general")}
        >
          <Info size={13} className="shrink-0 text-blue-500 dark:text-blue-400" aria-hidden="true" />
          <span>General</span>
        </button>
        <button
          type="button"
          className={`torrent-tab flex items-center gap-1.5 ${tab === "files" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "files"}
          onClick={() => setTab("files")}
        >
          <FolderTree size={13} className="shrink-0 text-amber-500 dark:text-amber-400" aria-hidden="true" />
          <span>Files{files.length ? ` (${files.length})` : ""}</span>
        </button>
        <button
          type="button"
          className={`torrent-tab flex items-center gap-1.5 ${tab === "peers" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "peers"}
          onClick={() => setTab("peers")}
        >
          <Users size={13} className="shrink-0 text-emerald-500 dark:text-emerald-400" aria-hidden="true" />
          <span>Peers{peers.length ? ` (${peers.length})` : ""}</span>
        </button>
        <button
          type="button"
          className={`torrent-tab flex items-center gap-1.5 ${tab === "trackers" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "trackers"}
          onClick={() => setTab("trackers")}
        >
          <Radio size={13} className="shrink-0 text-sky-500 dark:text-sky-400" aria-hidden="true" />
          <span>Trackers</span>
        </button>
        <button
          type="button"
          className={`torrent-tab flex items-center gap-1.5 ${tab === "pieces" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "pieces"}
          onClick={() => setTab("pieces")}
        >
          <LayoutGrid size={13} className="shrink-0 text-purple-500 dark:text-purple-400" aria-hidden="true" />
          <span>Pieces{piecesInfo?.totalPieces ? ` (${piecesInfo.completedPieces}/${piecesInfo.totalPieces})` : ""}</span>
        </button>
      </div>

      {tab === "general" && (
        <div id="torrent-tab-general" className="torrent-tab-panel" role="tabpanel">
          {torrent.error && (
            <div id="torrent-detail-error" className="torrent-error-card">
              {torrent.error}
            </div>
          )}
          {!isComplete && (
            <progress
              id="torrent-detail-progress"
              className="torrent-detail-progress"
              max={100}
              value={percent}
            />
          )}
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
            {torrent.ratio && torrent.ratio > 0.01 ? (
              <div>
                <dt>Ratio</dt>
                <dd id="torrent-fact-ratio">{formatRatio(torrent.ratio)}</dd>
              </div>
            ) : null}
            <div className={isSeeding ? "torrent-fact-dormant" : undefined}>
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
            <div className={isSeeding ? "torrent-fact-dormant" : undefined}>
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
            {isSeeding && onStopSeeding ? (
              <div className="torrent-fact-wide torrent-seeding-action-row">
                <button
                  type="button"
                  className="torrent-button torrent-stop-seeding-btn"
                  onClick={onStopSeeding}
                >
                  Stop Seeding
                </button>
              </div>
            ) : null}
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

      {tab === "peers" && (
        <TorrentPeersTab peers={peers} loading={Boolean(peersLoading)} />
      )}

      {tab === "trackers" && (
        <TorrentTrackersTab trackers={trackers} loading={trackersLoading} />
      )}

      {tab === "pieces" && (
        <TorrentPiecesTab piecesInfo={piecesInfo} loading={Boolean(piecesLoading)} />
      )}
    </div>
  );
}
