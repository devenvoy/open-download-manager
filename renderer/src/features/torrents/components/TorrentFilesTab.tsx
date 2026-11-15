import { formatBytes } from "@/lib/format";
import { type TorrentFileItem } from "@/types/ipc";
import { FileTypeIcon } from "@/lib/file-icon";

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

  const downloadedFiles = files.filter(
    (file) => file.length > 0 && file.progressBytes >= file.length,
  );
  const availableFiles = files.filter(
    (file) => !(file.length > 0 && file.progressBytes >= file.length),
  );

  return (
    <div id="torrent-tab-files" className="torrent-tab-panel" role="tabpanel">
      <p id="torrent-files-hint" className="torrent-files-hint">
        Select the files you want to download and share.
      </p>
      <div id="torrent-files-toolbar" className="torrent-files-toolbar">
        <div className="torrent-files-actions">
          <button
            type="button"
            id="torrent-files-all"
            className="small-button"
            onClick={() => onSetAll(true)}
          >
            Select All
          </button>
          <button
            type="button"
            id="torrent-files-none"
            className="small-button"
            onClick={() => onSetAll(false)}
          >
            Deselect All
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
        {downloadedFiles.length > 0 && (
          <div className="torrent-file-group">
            <div className="torrent-file-group-header">
              <span>Downloaded ({downloadedFiles.length})</span>
            </div>
            {downloadedFiles.map((file) => (
              <div key={file.index} className="torrent-file-row completed-row">
                <span className="torrent-file-complete-icon" title="Downloaded">
                  ✓
                </span>
                <FileTypeIcon
                  filename={file.name}
                  className="torrent-file-icon shrink-0"
                />
                <div className="torrent-file-main">
                  <span className="torrent-file-name" title={file.name}>
                    {file.name}
                  </span>
                  <span className="torrent-file-complete-badge">
                    <span>Complete</span>
                  </span>
                </div>
                <div className="torrent-file-meta">
                  <span>{formatBytes(file.length)}</span>
                  <span>100.0%</span>
                </div>
              </div>
            ))}
          </div>
        )}

        {availableFiles.length > 0 && (
          <div className="torrent-file-group">
            <div className="torrent-file-group-header">
              <span>Available to Download ({availableFiles.length})</span>
            </div>
            {availableFiles.map((file) => {
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
                  <FileTypeIcon
                    filename={file.name}
                    className="torrent-file-icon shrink-0"
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
        )}

        {files.length === 0 && (
          <div className="p-3 text-center text-xs text-[var(--torrent-muted)]">
            No files found in metadata.
          </div>
        )}
      </div>
    </div>
  );
}
