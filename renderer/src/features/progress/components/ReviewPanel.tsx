import { useState, type FormEvent } from "react";
import { DOWNLOAD_CATEGORIES, type DownloadSummary } from "@/types/ipc";
import {
  draftFromDownload,
  validateReviewDraft,
  type ReviewDraft,
} from "../model/review-form";

export interface ReviewPanelProps {
  download: DownloadSummary;
  submitting: boolean;
  onBrowse: () => Promise<string | null>;
  onSubmit: (draft: ReviewDraft, queueOnly: boolean) => void;
  onCancel: () => void;
}

/**
 * The confirmation dialog the progress window shows while
 * `download.needsReview` is true.
 */
export function ReviewPanel({
  download,
  submitting,
  onBrowse,
  onSubmit,
  onCancel,
}: ReviewPanelProps) {
  const [draft, setDraft] = useState<ReviewDraft>(() => draftFromDownload(download));
  const [error, setError] = useState<string | null>(null);

  function patch(next: Partial<ReviewDraft>) {
    setDraft((current) => ({ ...current, ...next }));
  }

  async function browse() {
    try {
      const picked = await onBrowse();
      if (picked) patch({ directory: picked });
    } catch {
      setError("Could not open the folder picker.");
    }
  }

  function submit(event: FormEvent, queueOnly: boolean) {
    event.preventDefault();
    const problem = validateReviewDraft(draft);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    onSubmit(draft, queueOnly);
  }

  return (
    <dialog
      id="add-dialog"
      className="add-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static"
      open
    >
      <form
        id="add-form"
        className="h-full flex flex-col overflow-hidden"
        onSubmit={(event) => submit(event, false)}
      >
        <div className="dialog-title">
          <strong id="add-title">Download File Info</strong>
          <button
            type="button"
            className="close-button"
            data-close
            aria-label="Close"
            onClick={onCancel}
          >
            ×
          </button>
        </div>

        <div className="dialog-body">
          <div className="form-line address-line">
            <label htmlFor="download-urls">Address:</label>
            <textarea
              id="download-urls"
              rows={2}
              readOnly
              value={download.url}
            />
          </div>

          <div className="form-line">
            <label htmlFor="download-category">Category:</label>
            <select
              id="download-category"
              value={draft.category}
              onChange={(e) =>
                patch({
                  category: (e.target.value as ReviewDraft["category"]) || "",
                })
              }
            >
              <option value="">Automatic</option>
              {DOWNLOAD_CATEGORIES.map((category) => (
                <option key={category} value={category}>
                  {category}
                </option>
              ))}
            </select>
          </div>

          <div className="form-line">
            <label htmlFor="download-filename">Save As:</label>
            <input
              id="download-filename"
              maxLength={240}
              value={draft.filename}
              onChange={(event) => patch({ filename: event.target.value })}
            />
          </div>

          <div className="form-line">
            <label htmlFor="add-directory">Save to:</label>
            <input
              id="add-directory"
              readOnly
              value={draft.directory}
            />
            <button
              type="button"
              id="add-browse"
              className="small-button"
              onClick={() => void browse()}
            >
              …
            </button>
          </div>

          <div className="form-line">
            <label htmlFor="download-description">Description:</label>
            <input
              id="download-description"
              maxLength={1_000}
              value={draft.description}
              onChange={(event) => patch({ description: event.target.value })}
            />
          </div>

          <div className="form-line schedule-line">
            <label htmlFor="download-schedule">Start at:</label>
            <input
              id="download-schedule"
              type="datetime-local"
              value={draft.scheduledAt}
              onChange={(event) => patch({ scheduledAt: event.target.value })}
            />
            <span className="field-help">Optional</span>
          </div>

          <div className="form-line auth-toggle-line">
            <label className="checkbox-label" htmlFor="add-auth-enabled">
              <input
                id="add-auth-enabled"
                type="checkbox"
                checked={draft.authEnabled}
                onChange={(event) =>
                  patch({ authEnabled: event.target.checked })
                }
              />
              Use basic authentication for this download
            </label>
          </div>

          {draft.authEnabled ? (
            <div
              id="add-auth-fields"
              className="form-line auth-fields"
              style={{ display: "flex", gap: 8 }}
            >
              <input
                id="add-auth-user"
                value={draft.authUser}
                onChange={(event) => patch({ authUser: event.target.value })}
                placeholder="Username"
                autoComplete="off"
                aria-label="Username"
                style={{ flex: 1 }}
              />
              <input
                id="add-auth-pass"
                type="password"
                value={draft.authPass}
                onChange={(event) => patch({ authPass: event.target.value })}
                placeholder="Password"
                autoComplete="off"
                aria-label="Password"
                style={{ flex: 1 }}
              />
            </div>
          ) : null}

          {error ? (
            <p id="add-error" role="alert" className="form-error">
              {error}
            </p>
          ) : null}
        </div>

        <div className="dialog-footer">
          <span className="spacer" />
          <button
            type="button"
            className="button"
            id="add-later"
            onClick={(event) => submit(event, true)}
          >
            Download Later
          </button>
          <button
            type="submit"
            className="button primary"
            id="add-submit"
            disabled={submitting}
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
    </dialog>
  );
}
