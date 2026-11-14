import { DOWNLOAD_CATEGORIES, type DownloadCategory } from "@/types/ipc";
import { type AddFormValues } from "../model/add-form";

export interface AddDownloadFormProps {
  values: AddFormValues;
  error: string | null;
  saving: boolean;
  showPlaylistBanner: boolean;
  showVideoBanner: boolean;
  videoLabel: string | null;
  onPatch: (patch: Partial<AddFormValues>) => void;
  onUrlsChange: (value: string) => void;
  onChooseDirectory: () => void;
  onPaste: () => void;
  onOpenPlaylist: () => void;
  onOpenFormats: () => void;
  onSubmit: (queueOnly: boolean) => void;
  onCancel: () => void;
  queueOptions: Array<{ id: string; name: string }>;
}

export function AddDownloadForm({
  values,
  error,
  saving,
  showPlaylistBanner,
  showVideoBanner,
  videoLabel,
  onPatch,
  onUrlsChange,
  onChooseDirectory,
  onPaste,
  onOpenPlaylist,
  onOpenFormats,
  onSubmit,
  onCancel,
  queueOptions,
}: AddDownloadFormProps) {
  return (
    <form
      id="add-form"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(false);
      }}
    >
      <div className="dialog-title">
        <strong id="add-title">Download File Info</strong>
      </div>

      <div className="dialog-body">
        <div className="form-line address-line">
          <label htmlFor="download-urls">URL:</label>
          <textarea
            id="download-urls"
            rows={2}
            required
            spellCheck={false}
            value={values.urlsText}
            onChange={(e) => onUrlsChange(e.target.value)}
            placeholder="https://example.com/file.zip"
          />
        </div>
        <p className="field-help address-help">
          Enter one URL per line for a batch download.
        </p>

        {showPlaylistBanner && (
          <div id="playlist-banner" className="banner-notice">
            <span>YouTube playlist detected!</span>
            <button
              type="button"
              className="button"
              id="playlist-banner-btn"
              onClick={onOpenPlaylist}
            >
              Select Videos…
            </button>
          </div>
        )}

        {showVideoBanner && (
          <div id="video-banner" className="banner-notice">
            <span>{videoLabel || "YouTube video detected!"}</span>
            <button
              type="button"
              className="button"
              id="video-banner-btn"
              onClick={onOpenFormats}
            >
              Choose Quality / Format…
            </button>
          </div>
        )}

        <div className="form-line">
          <label htmlFor="download-category">Category:</label>
          <select
            id="download-category"
            value={values.category}
            onChange={(e) =>
              onPatch({
                category: (e.target.value as DownloadCategory) || "",
              })
            }
          >
            <option value="">Automatic</option>
            {DOWNLOAD_CATEGORIES.map((cat) => (
              <option key={cat} value={cat}>
                {cat}
              </option>
            ))}
          </select>
        </div>

        <div className="form-line">
          <label htmlFor="download-queue">Queue:</label>
          <select
            id="download-queue"
            value={values.queueId}
            onChange={(e) => onPatch({ queueId: e.target.value })}
          >
            {queueOptions.map((q) => (
              <option key={q.id} value={q.id}>
                {q.name}
              </option>
            ))}
          </select>
        </div>

        <div className="form-line">
          <label htmlFor="download-filename">Save As:</label>
          <input
            id="download-filename"
            placeholder="Original file name (optional)"
            maxLength={240}
            value={values.filename}
            onChange={(e) => onPatch({ filename: e.target.value })}
          />
        </div>

        <div className="form-line">
          <label htmlFor="add-directory">Save to:</label>
          <input
            id="add-directory"
            readOnly
            value={values.directory}
            onChange={(e) => onPatch({ directory: e.target.value })}
          />
          <button
            type="button"
            id="add-browse"
            className="small-button"
            aria-label="Choose download folder"
            onClick={onChooseDirectory}
          >
            …
          </button>
        </div>

        <div className="form-line">
          <label htmlFor="download-description">Description:</label>
          <input
            id="download-description"
            maxLength={1000}
            value={values.description}
            onChange={(e) => onPatch({ description: e.target.value })}
          />
        </div>

        <div className="form-line schedule-line">
          <label htmlFor="download-schedule">Start at:</label>
          <input
            id="download-schedule"
            type="datetime-local"
            value={values.scheduledAt}
            onChange={(e) => onPatch({ scheduledAt: e.target.value })}
          />
          <span className="field-help">Optional</span>
        </div>

        <div className="form-line auth-toggle-line">
          <label className="checkbox-label" htmlFor="add-auth-enabled">
            <input
              type="checkbox"
              id="add-auth-enabled"
              checked={values.authEnabled}
              onChange={(e) => onPatch({ authEnabled: e.target.checked })}
            />
            Use basic authentication for this download
          </label>
        </div>

        {values.authEnabled && (
          <div
            id="add-auth-fields"
            className="form-line auth-fields"
            style={{ display: "flex", gap: "8px" }}
          >
            <input
              type="text"
              id="add-auth-user"
              placeholder="Username"
              autoComplete="off"
              style={{ flex: 1 }}
              value={values.authUser}
              onChange={(e) => onPatch({ authUser: e.target.value })}
            />
            <input
              type="password"
              id="add-auth-pass"
              placeholder="Password"
              autoComplete="off"
              style={{ flex: 1 }}
              value={values.authPass}
              onChange={(e) => onPatch({ authPass: e.target.value })}
            />
          </div>
        )}

        {error && (
          <p className="form-error" id="add-error" role="alert">
            {error}
          </p>
        )}
      </div>

      <div className="dialog-footer">
        <button
          type="button"
          className="button"
          id="paste-button"
          onClick={onPaste}
        >
          Paste
        </button>
        <span className="spacer" />
        <button
          type="button"
          className="button"
          id="add-later"
          data-queue-only="true"
          disabled={saving}
          onClick={() => onSubmit(true)}
        >
          Download Later
        </button>
        <button
          type="submit"
          className="button primary"
          id="add-submit"
          disabled={saving}
        >
          Start Download
        </button>
        <button
          type="button"
          className="button"
          data-close
          onClick={onCancel}
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
