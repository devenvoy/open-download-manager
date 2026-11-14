import { useState } from "react";
import { ClassicDialog } from "@/components/feedback";
import { ipc } from "@/lib/ipc";

export interface AddTorrentDialogProps {
  initialMode: "magnet" | "file";
  onAddMagnet: (
    magnet: string,
    options: { outputFolder?: string; startPaused: boolean; review: boolean },
  ) => void;
  onAddFile: (
    path: string,
    options: { outputFolder?: string; startPaused: boolean; review: boolean },
  ) => void;
  onClose: () => void;
  pending: boolean;
}

export function AddTorrentDialog({
  initialMode,
  onAddMagnet,
  onAddFile,
  onClose,
  pending,
}: AddTorrentDialogProps) {
  const [mode, setMode] = useState<"magnet" | "file">(initialMode);
  const [magnet, setMagnet] = useState("");
  const [filePath, setFilePath] = useState("");
  const [folder, setFolder] = useState("");
  const [error, setError] = useState<string | null>(null);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    const options = {
      outputFolder: folder.trim() || undefined,
      startPaused: false,
      review: true,
    };

    if (mode === "magnet") {
      const trimmed = magnet.trim();
      if (!trimmed) {
        setError("Please enter a magnet link.");
        return;
      }
      if (!trimmed.toLowerCase().startsWith("magnet:?")) {
        setError("Invalid magnet link. Must start with magnet:?");
        return;
      }
      onAddMagnet(trimmed, options);
      onClose();
    } else {
      const trimmed = filePath.trim();
      if (!trimmed) {
        setError("Please choose a .torrent file.");
        return;
      }
      onAddFile(trimmed, options);
      onClose();
    }
  }

  return (
    <ClassicDialog
      id="torrent-add-dialog"
      className="torrent-dialog"
      open
      onClose={onClose}
      title={<span>Add Torrent</span>}
    >
      <form onSubmit={handleSubmit}>
        <div className="dialog-body torrent-add-body">
          <div className="torrent-source-switch">
            <button
              type="button"
              id="torrent-source-magnet"
              className={`torrent-source-button ${mode === "magnet" ? "active-tab" : ""}`}
              aria-pressed={mode === "magnet"}
              onClick={() => {
                setMode("magnet");
                setError(null);
              }}
            >
              Magnet link
            </button>
            <button
              type="button"
              id="torrent-source-file"
              className={`torrent-source-button ${mode === "file" ? "active-tab" : ""}`}
              aria-pressed={mode === "file"}
              onClick={() => {
                setMode("file");
                setError(null);
              }}
            >
              .torrent file
            </button>
          </div>

          {mode === "magnet" ? (
            <label id="torrent-magnet-panel" className="torrent-field">
              <span>Magnet link</span>
              <textarea
                id="torrent-magnet-input"
                rows={3}
                spellCheck={false}
                placeholder="magnet:?xt=urn:btih:…"
                value={magnet}
                onChange={(e) => {
                  setMagnet(e.target.value);
                  setError(null);
                }}
              />
            </label>
          ) : (
            <div id="torrent-file-panel" className="torrent-field">
              <span>Torrent file</span>
              <div className="torrent-path-row">
                <input
                  id="torrent-file-path"
                  type="text"
                  readOnly
                  placeholder="Choose a .torrent file"
                  value={filePath}
                />
                <button
                  type="button"
                  id="torrent-file-browse"
                  className="small-button"
                  onClick={async () => {
                    const picked = await ipc.chooseFile();
                    if (picked) setFilePath(picked);
                  }}
                >
                  Browse…
                </button>
              </div>
            </div>
          )}

          <div className="torrent-field">
            <span>Save to</span>
            <div className="torrent-path-row">
              <input
                id="torrent-output-folder"
                type="text"
                placeholder="Default torrent folder"
                value={folder}
                onChange={(e) => setFolder(e.target.value)}
              />
              <button
                type="button"
                id="torrent-output-browse"
                className="small-button"
                onClick={async () => {
                  const picked = await ipc.chooseDirectory();
                  if (picked) setFolder(picked);
                }}
              >
                Browse…
              </button>
            </div>
            <small className="field-help">
              Leave empty to use the default folder from settings.
            </small>
          </div>

          {error && (
            <p id="torrent-add-error" className="form-error">
              {error}
            </p>
          )}
        </div>

        <div className="dialog-footer">
          <button
            type="button"
            className="button"
            data-close="torrent-add-dialog"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="submit"
            id="torrent-add-confirm"
            className="button primary"
            disabled={pending}
          >
            Add
          </button>
        </div>
      </form>
    </ClassicDialog>
  );
}
