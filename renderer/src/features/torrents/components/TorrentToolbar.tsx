import { type TorrentListItem } from "@/types/ipc";
import {
  availableActions,
  summaryText,
  type TorrentSummary,
} from "../model/torrent-view";

export interface TorrentToolbarProps {
  summary: TorrentSummary;
  selected: TorrentListItem | null;
  busy: boolean;
  onOpenAdd: (initialMode: "magnet" | "file") => void;
  onOpenRemove: () => void;
  onOpenSettings: () => void;
  onAction: (action: "pause" | "resume" | "recheck") => void;
  engineInfo?: string;
}

export function TorrentToolbar({
  summary,
  selected,
  busy,
  onOpenAdd,
  onOpenRemove,
  onOpenSettings,
  onAction,
  engineInfo,
}: TorrentToolbarProps) {
  const actions = availableActions(selected);

  return (
    <div className="torrent-toolbar" role="toolbar" aria-label="Torrent commands">
      <button
        id="torrent-add-magnet"
        type="button"
        className="torrent-tool"
        title="Add magnet link"
        onClick={() => onOpenAdd("magnet")}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M8 20v6a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-6" />
          <path d="M16 4v16M10 14l6 6 6-6" />
        </svg>
        <span>Add Magnet</span>
      </button>

      <button
        id="torrent-add-file"
        type="button"
        className="torrent-tool"
        title="Add a .torrent file"
        onClick={() => onOpenAdd("file")}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M7 4h12l6 6v18H7z" />
          <path d="M19 4v6h6M16 15v9M12 20l4 4 4-4" />
        </svg>
        <span>Add File</span>
      </button>

      <span className="torrent-tool-divider" />

      <button
        id="torrent-pause"
        type="button"
        className="torrent-tool"
        title="Pause selected torrent"
        disabled={busy || !actions.pause}
        onClick={() => onAction("pause")}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M11 7v18M21 7v18" />
        </svg>
        <span>Pause</span>
      </button>

      <button
        id="torrent-resume"
        type="button"
        className="torrent-tool"
        title="Resume selected torrent"
        disabled={busy || !actions.resume}
        onClick={() => onAction("resume")}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M10 6l16 10-16 10z" />
        </svg>
        <span>Resume</span>
      </button>

      <button
        id="torrent-recheck"
        type="button"
        className="torrent-tool"
        title="Verify selected torrent data"
        disabled={busy || !actions.recheck}
        onClick={() => onAction("recheck")}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M26 16a10 10 0 1 1-3-7" />
          <path d="M26 5v6h-6" />
        </svg>
        <span>Recheck</span>
      </button>

      <button
        id="torrent-remove"
        type="button"
        className="torrent-tool"
        title="Remove selected torrent"
        disabled={busy || !selected}
        onClick={onOpenRemove}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M6 8h20M13 8V4h6v4M8 11l2 17h12l2-17M14 14v11M18 14v11" />
        </svg>
        <span>Remove</span>
      </button>

      <span className="torrent-tool-divider" />

      <button
        id="torrent-settings"
        type="button"
        className="torrent-tool"
        title="Torrent settings"
        onClick={onOpenSettings}
      >
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <circle cx="16" cy="16" r="5" />
          <path d="M16 3v4M16 25v4M3 16h4M25 16h4M6.8 6.8l2.9 2.9M22.3 22.3l2.9 2.9M25.2 6.8l-2.9 2.9M9.7 22.3l-2.9 2.9" />
        </svg>
        <span>Settings</span>
      </button>

      <div className="torrent-toolbar-status">
        <span className="torrent-beta-badge">Beta</span>
        <span id="torrent-summary">{summaryText(summary)}</span>
        {engineInfo && <span id="torrent-engine-info">{engineInfo}</span>}
      </div>
    </div>
  );
}
