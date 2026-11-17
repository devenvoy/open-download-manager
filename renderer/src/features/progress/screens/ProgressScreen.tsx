import { useCallback, useEffect, useState } from "react";
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
import { CompletedPanel } from "../components/CompletedPanel";
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
  const reviewing = Boolean(download?.needsReview && !review.isSuccess);

  const close = useCallback(() => {
    void progressApi.close();
  }, []);

  // A details window whose download vanished (removed, migrated, or replaced)
  // must not linger on top of other windows with a stale message.
  useEffect(() => {
    if (!snapshot.isPending && !download) {
      const timer = setTimeout(close, 2500);
      return () => clearTimeout(timer);
    }
  }, [snapshot.isPending, download, close]);

  useEffect(() => {
    document.body.classList.toggle("review-window", reviewing);
    return () => {
      document.body.classList.remove("review-window");
    };
  }, [reviewing]);

  useEffect(() => {
    document.body.classList.toggle("details-collapsed", collapsed);
    return () => {
      document.body.classList.remove("details-collapsed");
    };
  }, [collapsed]);

  const submitReview = useCallback(
    (draft: ReviewDraft, queueOnly: boolean) => {
      const patch = buildReviewPatch(draft, queueOnly);
      review.mutate(patch, {
        onSuccess: () => {
          if (queueOnly || patch.scheduledAt) close();
        },
      });
    },
    [review, close],
  );

  const cancelReview = useCallback(() => {
    action.mutate("remove", { onSuccess: close });
  }, [action, close]);

  const applySpeed = useCallback(
    async (speedLimitKbps: number) => {
      await updateSettings.mutateAsync({ speedLimitKbps });
    },
    [updateSettings],
  );

  const saveCompletionToggles = useCallback(
    async (patch: { notifyOnComplete: boolean; revealOnComplete: boolean }) => {
      await updateSettings.mutateAsync(patch);
    },
    [updateSettings],
  );

  if (snapshot.isPending) {
    return (
      <dialog id="details-dialog" className="details-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static" open>
        <div className="dialog-body" style={{ padding: 20 }}>
          Loading download progress…
        </div>
      </dialog>
    );
  }

  if (snapshot.isError || !download) {
    return (
      <dialog id="details-dialog" className="details-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static" open>
        <div className="dialog-body flex flex-col items-center justify-center h-full gap-4 text-center" style={{ padding: 20 }}>
          <p className="text-sm">
            {snapshot.isError
              ? `Could not read download: ${snapshot.error instanceof Error ? snapshot.error.message : "Error"}`
              : "This download is no longer in the list."}
          </p>
          <button type="button" className="button" style={{ minWidth: 80 }} onClick={close}>
            Close
          </button>
        </div>
      </dialog>
    );
  }

  if (reviewing) {
    return (
      <ReviewPanel
        download={download}
        submitting={review.isPending}
        onBrowse={() => chooseDirectory.mutateAsync()}
        onSubmit={submitReview}
        onCancel={cancelReview}
      />
    );
  }

  const status = statusOf(download);
  const percent = percentage(download);
  const settings = snapshot.data?.settings;

  return (
    <dialog
      id="details-dialog"
      className={`details-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static ${status === "completed" ? "is-complete" : ""}`}
      open
      aria-labelledby="details-title"
    >
      <div className="dialog-title">
        <strong id="details-title">
          {status === "completed"
            ? "Download complete"
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
        {settings ? (
          <>
            <div
              className="dialog-tabs"
              role="tablist"
              aria-label="Download properties"
            >
              <button
                type="button"
                id="tab-status"
                role="tab"
                aria-selected={tab === "status"}
                aria-controls="status-panel"
                className={tab === "status" ? "active-tab" : ""}
                onClick={() => setTab("status")}
                onMouseDown={() => setTab("status")}
              >
                Download status
              </button>
              <button
                type="button"
                id="tab-speed"
                role="tab"
                aria-selected={tab === "speed"}
                aria-controls="speed-panel"
                className={tab === "speed" ? "active-tab" : ""}
                onClick={() => setTab("speed")}
                onMouseDown={() => setTab("speed")}
              >
                Speed Limiter
              </button>
              <button
                type="button"
                id="tab-completion"
                role="tab"
                aria-selected={tab === "completion"}
                aria-controls="completion-panel"
                className={tab === "completion" ? "active-tab" : ""}
                onClick={() => setTab("completion")}
                onMouseDown={() => setTab("completion")}
              >
                Options on completion
              </button>
            </div>

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

            <progress
              id="detail-progress"
              max={100}
              value={percent ?? 0}
              aria-label="Download progress"
            />

            {status !== "completed" && (
              <div className="transfer-buttons">
                <button
                  type="button"
                  className="button"
                  id="toggle-details"
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
                  id="detail-resume"
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
                  id="detail-stop"
                  disabled={!canPause(download) || action.isPending}
                  onClick={() => action.mutate("pause")}
                >
                  Pause
                </button>
                <button
                  type="button"
                  className="button"
                  id="detail-cancel"
                  disabled={!canCancel(download) || action.isPending}
                  onClick={() => action.mutate("cancel")}
                >
                  Cancel
                </button>
              </div>
            )}

            {!collapsed && status !== "completed" && (
              <SegmentSection download={download} />
            )}
          </>
        ) : null}
      </div>

      <div className="dialog-footer">
        <button
          type="button"
          className="button"
          data-close
          onClick={close}
        >
          Close
        </button>
      </div>

      {status === "completed" && settings ? (
        <CompletedPanel
          download={download}
          settings={settings}
          onOpen={() => action.mutate("open")}
          onReveal={() => action.mutate("reveal")}
          onClose={close}
          onUpdateSettings={async (patch) => {
            await updateSettings.mutateAsync(patch);
          }}
        />
      ) : null}
    </dialog>
  );
}
