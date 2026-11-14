import { type ReactNode, useState } from "react";
import { type TorrentListItem } from "@/types/ipc";
import {
  availableActions,
  summaryText,
  type TorrentSummary,
} from "../model/torrent-view";
import { AddTorrentDialog } from "./AddTorrentDialog";
import { TorrentRemoveDialog } from "./TorrentRemoveDialog";

export interface AddTorrentOptionsInput {
  outputFolder?: string;
  startPaused: boolean;
  review: boolean;
}

export interface TorrentToolbarProps {
  summary: TorrentSummary;
  selected: TorrentListItem | null;
  busy: boolean;
  onAction: (action: "pause" | "resume" | "recheck" | "forget" | "delete") => void;
  onAddMagnet: (magnet: string, options: AddTorrentOptionsInput) => void;
  onAddFile: (path: string, options: AddTorrentOptionsInput) => void;
  settingsButton: ReactNode;
}

export function TorrentToolbar({
  summary,
  selected,
  busy,
  onAction,
  onAddMagnet,
  onAddFile,
  settingsButton,
}: TorrentToolbarProps) {
  const actions = availableActions(selected);
  const [removeOpen, setRemoveOpen] = useState(false);

  return (
    <div className="torrent-toolbar" role="toolbar" aria-label="Torrent commands">
      <AddTorrentDialog
        onAddMagnet={onAddMagnet}
        onAddFile={onAddFile}
        pending={busy}
      />

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

      {selected ? (
        <>
          <button
            id="torrent-remove"
            type="button"
            className="torrent-tool"
            title="Remove selected torrent"
            disabled={busy}
            onClick={() => setRemoveOpen(true)}
          >
            <svg viewBox="0 0 32 32" aria-hidden="true">
              <path d="M6 8h20M13 8V4h6v4M8 11l2 17h12l2-17M14 14v11M18 14v11" />
            </svg>
            <span>Remove</span>
          </button>
          {removeOpen && (
            <TorrentRemoveDialog
              torrent={selected}
              disabled={busy}
              onAction={(act) => {
                onAction(act);
                setRemoveOpen(false);
              }}
            />
          )}
        </>
      ) : (
        <button
          id="torrent-remove"
          type="button"
          className="torrent-tool"
          title="Remove selected torrent"
          disabled
        >
          <svg viewBox="0 0 32 32" aria-hidden="true">
            <path d="M6 8h20M13 8V4h6v4M8 11l2 17h12l2-17M14 14v11M18 14v11" />
          </svg>
          <span>Remove</span>
        </button>
      )}

      <span className="torrent-tool-divider" />

      {settingsButton}

      <div className="torrent-toolbar-status">
        <span className="torrent-beta-badge">Beta</span>
        <span id="torrent-summary">{summaryText(summary)}</span>
      </div>
    </div>
  );
}
