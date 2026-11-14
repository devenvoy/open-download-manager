import { formatBytes } from "@/lib/format";
import { type TorrentFileItem } from "@/types/ipc";

export interface TorrentFilesTabProps {
  files: TorrentFileItem[];
  loading: boolean;
  selection: Set<number> | null;
  dirty: boolean;
  applying: boolean;
  onToggle: (index: number) => void;
  onSetAll: (selected: boolean) => void;
  onApply: () => void;
}

export function TorrentFilesTab({
  files,
  loading,
  selection,
  dirty,
  applying,
  onToggle,
  onSetAll,
  onApply,
}: TorrentFilesTabProps) {
  if (loading) {
    return (
      <div id="torrent-tab-files" className="torrent-tab-panel" role="tabpanel">
        <p className="torrent-files-hint">Reading files…</p>
      </div>
    );
  }

  return (
    <div id="torrent-tab-files" className="torrent-tab-panel" role="tabpanel">
      <p id="torrent-files-hint" className="torrent-files-hint">
        Tick the files you want. Only the ticked files are downloaded and shared.
      </p>
      <div id="torrent-files-toolbar" className="torrent-files-toolbar">
        <div className="torrent-files-actions">
          <button
            type="button"
            id="torrent-files-all"
            className="small-button"
            onClick={() => onSetAll(true)}
          >
            Tick all
          </button>
          <button
            type="button"
            id="torrent-files-none"
            className="small-button"
            onClick={() => onSetAll(false)}
          >
            Tick none
          </button>
        </div>
        <button
          type="button"
          id="torrent-files-apply"
          className="small-button primary"
          disabled={!dirty || applying}
          onClick={onApply}
        >
          Save selection
        </button>
      </div>
      <div id="torrent-file-list" className="torrent-file-list">
        {files.map((file) => {
          const included =
            selection !== null ? selection.has(file.index) : file.included;
          const filePercent =
            file.length > 0 ? (file.progressBytes / file.length) * 100 : 0;
          return (
            <label key={file.index} className="torrent-file-row">
              <input
                type="checkbox"
                checked={included}
                onChange={() => onToggle(file.index)}
              />
              <div className="torrent-file-main">
                <span className="torrent-file-name" title={file.name}>
                  {file.name}
                </span>
                <progress
                  className="torrent-file-progress"
                  max={100}
                  value={filePercent}
                />
              </div>
              <div className="torrent-file-meta">
                <span>{formatBytes(file.length)}</span>
                <span>{filePercent.toFixed(1)}%</span>
              </div>
            </label>
          );
        })}
      </div>
    </div>
  );
}
