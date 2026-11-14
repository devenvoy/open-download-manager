import { useState } from "react";
import { useHydrateTheme } from "@/stores/theme-store";
import {
  canCancel,
  canPause,
  canResume,
  percentage,
  statusOf,
} from "../model/progress-view";
import { buildReviewPatch, type ReviewDraft } from "../model/review-form";
import {
  progressApi,
  useChooseDirectory,
  useDownloadAction,
  useProgressSnapshot,
  useRetryCompletionActions,
  useReviewDownload,
  useUpdateSettings,
} from "../api";
import { CompletionPanel } from "../components/CompletionPanel";
import { ReviewPanel } from "../components/ReviewPanel";
import { SegmentSection } from "../components/SegmentSection";
import { SpeedLimitPanel } from "../components/SpeedLimitPanel";
import { StatusPanel } from "../components/StatusPanel";

export interface ProgressScreenProps {
  id: string;
}

export function ProgressScreen({ id }: ProgressScreenProps) {
  const snapshot = useProgressSnapshot();
  useHydrateTheme(snapshot.data?.settings.theme);

  const action = useDownloadAction(id);
  const updateSettings = useUpdateSettings();
  const retryActions = useRetryCompletionActions(id);
  const review = useReviewDownload(id);
  const chooseDirectory = useChooseDirectory();

  const [tab, setTab] = useState("status");
  const [collapsed, setCollapsed] = useState(false);

  const download =
    snapshot.data?.downloads.find((item) => item.id === id) ?? null;

  function close() {
    void progressApi.close();
  }

  if (snapshot.isPending) {
    return (
      <dialog id="progress-dialog" className="progress-dialog" open>
        <div className="dialog-body" style={{ padding: 20 }}>
          Loading download progress…
        </div>
      </dialog>
    );
  }

  if (snapshot.isError || !download) {
    return (
      <dialog id="progress-dialog" className="progress-dialog" open>
        <div className="dialog-body" style={{ padding: 20 }}>
          {snapshot.isError
            ? `Could not read download: ${snapshot.error instanceof Error ? snapshot.error.message : "Error"}`
            : "This download is no longer in the list."}
        </div>
        <div className="dialog-footer">
          <button type="button" className="button" onClick={close}>
            Close
          </button>
        </div>
      </dialog>
    );
  }

  function submitReview(draft: ReviewDraft, queueOnly: boolean) {
    const patch = buildReviewPatch(draft, queueOnly);
    review.mutate(patch, {
      onSuccess: () => {
        if (queueOnly || patch.scheduledAt) close();
      },
    });
  }

  function cancelReview() {
    action.mutate("remove", { onSuccess: close });
  }

  async function applySpeed(speedLimitKbps: number) {
    await updateSettings.mutateAsync({ speedLimitKbps });
  }

  async function saveCompletionToggles(patch: {
    notifyOnComplete: boolean;
    revealOnComplete: boolean;
  }) {
    await updateSettings.mutateAsync(patch);
  }

  const status = statusOf(download);
  const percent = percentage(download);
  const reviewing = download.needsReview && !review.isSuccess;
  const settings = snapshot.data?.settings;

  return (
    <dialog id="progress-dialog" className="progress-dialog" open>
      <div className="dialog-title">
        <strong id="detail-title">
          {reviewing
            ? "Download File Info"
            : `${percent !== null ? `${Math.floor(percent)}% ` : ""}${
                download.filename || "Download status"
              }`}
        </strong>
        <button
          type="button"
          className="close-button"
          data-close
          aria-label="Close"
          onClick={close}
        >
          ×
        </button>
      </div>

      <div className="dialog-body">
        {reviewing ? (
          <ReviewPanel
            download={download}
            submitting={review.isPending}
            onBrowse={() => chooseDirectory.mutateAsync()}
            onSubmit={submitReview}
            onCancel={cancelReview}
          />
        ) : settings ? (
          <>
            <div className="dialog-tabs" role="tablist">
              <button
                type="button"
                role="tab"
                aria-selected={tab === "status"}
                className={`dialog-tab ${tab === "status" ? "active" : ""}`}
                onClick={() => setTab("status")}
                onMouseDown={() => setTab("status")}
              >
                Download status
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={tab === "speed"}
                className={`dialog-tab ${tab === "speed" ? "active" : ""}`}
                onClick={() => setTab("speed")}
                onMouseDown={() => setTab("speed")}
              >
                Speed Limiter
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={tab === "completion"}
                className={`dialog-tab ${tab === "completion" ? "active" : ""}`}
                onClick={() => setTab("completion")}
                onMouseDown={() => setTab("completion")}
              >
                Options on completion
              </button>
            </div>

            <div className="tab-body" style={{ marginTop: 8 }}>
              {tab === "status" && (
                <StatusPanel
                  download={download}
                  busy={action.isPending}
                  onRetry={() => action.mutate("retry")}
                />
              )}
              {tab === "speed" && (
                <SpeedLimitPanel settings={settings} onApply={applySpeed} />
              )}
              {tab === "completion" && (
                <CompletionPanel
                  download={download}
                  settings={settings}
                  retrying={retryActions.isPending}
                  onSaveToggles={saveCompletionToggles}
                  onRetryActions={() => retryActions.mutate()}
                  onOpen={() => action.mutate("open")}
                  onReveal={() => action.mutate("reveal")}
                />
              )}
            </div>

            <progress
              id="detail-progress"
              max={100}
              value={percent ?? 0}
              aria-label="Download progress"
              style={{ width: "100%", margin: "10px 0" }}
            />

            {status !== "completed" ? (
              <div className="transfer-buttons">
                <button
                  type="button"
                  className="button"
                  onClick={() => {
                    const next = !collapsed;
                    setCollapsed(next);
                    void progressApi.resize(!next);
                  }}
                >
                  {collapsed ? ">> Show details" : "<< Hide details"}
                </button>
                <span className="spacer" />
                <button
                  type="button"
                  className="button"
                  disabled={!canResume(download) || action.isPending}
                  onClick={() =>
                    action.mutate(
                      status === "failed" || status === "cancelled"
                        ? "retry"
                        : "resume",
                    )
                  }
                >
                  Resume
                </button>
                <button
                  type="button"
                  className="button"
                  disabled={!canPause(download) || action.isPending}
                  onClick={() => action.mutate("pause")}
                >
                  Pause
                </button>
                <button
                  type="button"
                  className="button"
                  disabled={!canCancel(download) || action.isPending}
                  onClick={() => action.mutate("cancel")}
                >
                  Cancel
                </button>
              </div>
            ) : null}

            {!collapsed && status !== "failed" ? (
              <div id="connection-details">
                <p className="connection-caption">
                  Start positions and download progress by connections
                </p>
                <SegmentSection download={download} />
              </div>
            ) : null}
          </>
        ) : null}
      </div>

      <div className="dialog-footer">
        <button type="button" className="button" onClick={close}>
          Close
        </button>
      </div>
    </dialog>
  );
}
