import {
  Magnet,
  FilePlus,
  Pause,
  Play,
  RotateCw,
  Trash2,
  Settings2,
} from "lucide-react";
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
    <div className="torrent-toolbar select-none" role="toolbar" aria-label="Torrent commands">
      <button
        id="torrent-add-magnet"
        type="button"
        className="torrent-tool group"
        title="Add magnet link"
        onClick={() => onOpenAdd("magnet")}
      >
        <Magnet
          size={22}
          strokeWidth={2}
          className="shrink-0 text-orange-500 dark:text-orange-400 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
        <span>Add Magnet</span>
      </button>

      <button
        id="torrent-add-file"
        type="button"
        className="torrent-tool group"
        title="Add a .torrent file"
        onClick={() => onOpenAdd("file")}
      >
        <FilePlus
          size={22}
          strokeWidth={2}
          className="shrink-0 text-blue-600 dark:text-blue-400 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
        <span>Add File</span>
      </button>

      <span className="torrent-tool-divider" />

      <button
        id="torrent-pause"
        type="button"
        className="torrent-tool group"
        title="Pause selected torrent"
        disabled={busy || !actions.pause}
        onClick={() => onAction("pause")}
      >
        <Pause
          size={22}
          strokeWidth={2}
          className="shrink-0 text-amber-600 dark:text-amber-400 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
        <span>Pause</span>
      </button>

      <button
        id="torrent-resume"
        type="button"
        className="torrent-tool group"
        title="Resume selected torrent"
        disabled={busy || !actions.resume}
        onClick={() => onAction("resume")}
      >
        <Play
          size={22}
          strokeWidth={2}
          className="shrink-0 text-emerald-600 dark:text-emerald-400 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
        <span>Resume</span>
      </button>

      <button
        id="torrent-recheck"
        type="button"
        className="torrent-tool group"
        title="Verify selected torrent data"
        disabled={busy || !actions.recheck}
        onClick={() => onAction("recheck")}
      >
        <RotateCw
          size={22}
          strokeWidth={2}
          className="shrink-0 text-sky-600 dark:text-sky-400 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
        <span>Recheck</span>
      </button>

      <button
        id="torrent-remove"
        type="button"
        className="torrent-tool group"
        title="Remove selected torrent"
        disabled={busy || !selected}
        onClick={onOpenRemove}
      >
        <Trash2
          size={22}
          strokeWidth={2}
          className="shrink-0 text-rose-600 dark:text-rose-400 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
        <span>Remove</span>
      </button>

      <span className="torrent-tool-divider" />

      <button
        id="torrent-settings"
        type="button"
        className="torrent-tool group"
        title="Torrent settings"
        onClick={onOpenSettings}
      >
        <Settings2
          size={22}
          strokeWidth={2}
          className="shrink-0 text-slate-700 dark:text-slate-300 transition-transform duration-100 group-hover:scale-105"
          aria-hidden="true"
        />
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
