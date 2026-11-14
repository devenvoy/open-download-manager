import { useState } from "react";
import { ClassicDialog } from "@/components/feedback";
import { ipc } from "@/lib/ipc";
import { type TorrentSettings } from "@/types/ipc";

export interface TorrentSettingsDialogProps {
  settings: TorrentSettings;
  saving: boolean;
  onSave: (settings: TorrentSettings) => void;
  onRestart: () => void;
  restarting: boolean;
  onClose: () => void;
}

export function TorrentSettingsDialog({
  settings,
  saving,
  onSave,
  onRestart,
  restarting,
  onClose,
}: TorrentSettingsDialogProps) {
  const [draft, setDraft] = useState<TorrentSettings>(() => ({ ...settings }));
  const [error, setError] = useState<string | null>(null);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (draft.listenPort < 0 || draft.listenPort > 65535) {
      setError("Listen port must be between 0 and 65535.");
      return;
    }
    if (draft.maxPeersPerTorrent < 1) {
      setError("Max peers per torrent must be at least 1.");
      return;
    }
    onSave(draft);
    onClose();
  }

  return (
    <ClassicDialog
      id="torrent-settings-dialog"
      className="torrent-dialog torrent-settings-dialog"
      open
      onClose={onClose}
      title={<span>Torrent Settings</span>}
    >
      <form onSubmit={handleSubmit}>
        <div className="dialog-body torrent-settings-body">
          <p
            id="torrent-settings-engine-info"
            className="torrent-settings-engine-info"
          >
            Port {draft.listenPort} · DHT {draft.enableDht ? "on" : "off"} · uTP{" "}
            {draft.enableUtp ? "on" : "off"}
          </p>
          <label className="torrent-check">
            <input
              id="torrent-setting-enabled"
              type="checkbox"
              checked={draft.enabled}
              onChange={(e) =>
                setDraft({ ...draft, enabled: e.target.checked })
              }
            />
            <span>Enable BitTorrent support</span>
          </label>
          <div className="torrent-field">
            <span>Default download folder</span>
            <div className="torrent-path-row">
              <input
                id="torrent-setting-directory"
                type="text"
                placeholder="Default: Downloads/Torrents"
                value={draft.downloadDirectory ?? ""}
                onChange={(e) =>
                  setDraft({ ...draft, downloadDirectory: e.target.value || null })
                }
              />
              <button
                type="button"
                id="torrent-setting-directory-browse"
                className="small-button"
                onClick={async () => {
                  const picked = await ipc.chooseDirectory();
                  if (picked) setDraft({ ...draft, downloadDirectory: picked });
                }}
              >
                Browse…
              </button>
            </div>
          </div>
          <div className="torrent-settings-grid">
            <label className="torrent-field">
              <span>Listen port</span>
              <input
                id="torrent-setting-port"
                type="number"
                min="0"
                max="65535"
                step="1"
                value={draft.listenPort}
                onChange={(e) =>
                  setDraft({ ...draft, listenPort: Number(e.target.value) || 0 })
                }
              />
              <small className="field-help">0 chooses a free port.</small>
            </label>
            <label className="torrent-field">
              <span>Max peers per torrent</span>
              <input
                id="torrent-setting-peers"
                type="number"
                min="1"
                max="2000"
                step="1"
                value={draft.maxPeersPerTorrent}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    maxPeersPerTorrent: Number(e.target.value) || 200,
                  })
                }
              />
              <small className="field-help">
                Only connected peers can send data. A small number slows a busy
                torrent down.
              </small>
            </label>
            <label className="torrent-field">
              <span>Download limit (KB/s)</span>
              <input
                id="torrent-setting-download-limit"
                type="number"
                min="0"
                step="1"
                value={draft.downloadLimitKbps}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    downloadLimitKbps: Number(e.target.value) || 0,
                  })
                }
              />
              <small className="field-help">0 means unlimited.</small>
            </label>
            <label className="torrent-field">
              <span>Upload limit (KB/s)</span>
              <input
                id="torrent-setting-upload-limit"
                type="number"
                min="0"
                step="1"
                value={draft.uploadLimitKbps}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    uploadLimitKbps: Number(e.target.value) || 0,
                  })
                }
              />
              <small className="field-help">0 means unlimited.</small>
            </label>
            <label className="torrent-field">
              <span>Seed ratio limit</span>
              <input
                id="torrent-setting-ratio"
                type="number"
                min="0"
                step="0.1"
                value={draft.seedRatioLimit}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    seedRatioLimit: Number(e.target.value) || 0,
                  })
                }
              />
              <small className="field-help">0 means no ratio limit.</small>
            </label>
            <label className="torrent-field">
              <span>Seed time limit (minutes)</span>
              <input
                id="torrent-setting-seed-time"
                type="number"
                min="0"
                step="1"
                value={draft.seedTimeLimitMinutes}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    seedTimeLimitMinutes: Number(e.target.value) || 0,
                  })
                }
              />
              <small className="field-help">0 means no time limit.</small>
            </label>
          </div>
          <div className="torrent-settings-toggles">
            <label className="torrent-check">
              <input
                id="torrent-setting-dht"
                type="checkbox"
                checked={draft.enableDht}
                onChange={(e) =>
                  setDraft({ ...draft, enableDht: e.target.checked })
                }
              />
              <span>Enable DHT</span>
            </label>
            <label className="torrent-check">
              <input
                id="torrent-setting-upnp"
                type="checkbox"
                checked={draft.enableUpnp}
                onChange={(e) =>
                  setDraft({ ...draft, enableUpnp: e.target.checked })
                }
              />
              <span>Enable UPnP port forwarding</span>
            </label>
            <label className="torrent-check">
              <input
                id="torrent-setting-utp"
                type="checkbox"
                checked={draft.enableUtp}
                onChange={(e) =>
                  setDraft({ ...draft, enableUtp: e.target.checked })
                }
              />
              <span>Enable uTP</span>
            </label>
            <label className="torrent-check">
              <input
                id="torrent-setting-start-paused"
                type="checkbox"
                checked={draft.startPaused}
                onChange={(e) =>
                  setDraft({ ...draft, startPaused: e.target.checked })
                }
              />
              <span>Add new torrents paused</span>
            </label>
            <label className="torrent-check">
              <input
                id="torrent-setting-notify"
                type="checkbox"
                checked={draft.notifyOnComplete}
                onChange={(e) =>
                  setDraft({ ...draft, notifyOnComplete: e.target.checked })
                }
              />
              <span>Notify when a torrent completes</span>
            </label>
          </div>
          {error && (
            <p id="torrent-settings-error" className="form-error">
              {error}
            </p>
          )}
        </div>
        <div className="dialog-footer">
          <button
            type="button"
            id="torrent-settings-restart"
            className="button"
            disabled={restarting}
            onClick={onRestart}
          >
            Restart engine
          </button>
          <span className="torrent-footer-spacer" />
          <button
            type="button"
            className="button"
            data-close="torrent-settings-dialog"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="submit"
            id="torrent-settings-save"
            className="button primary"
            disabled={saving}
          >
            Save
          </button>
        </div>
      </form>
    </ClassicDialog>
  );
}
