import { useState } from "react";
import { EmptyState } from "@/components/feedback/EmptyState";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useHydrateTheme } from "@/stores/theme-store";
import {
  canCancel,
  canPause,
  canResume,
  percentage,
  statusLabel,
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

/** The `?progress=<id>` window: details for one download, or its review form. */
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

  const download = snapshot.data?.downloads.find((item) => item.id === id) ?? null;

  function close() {
    void progressApi.close();
  }

  if (snapshot.isPending) {
    return (
      <div className="space-y-3 p-4">
        <Skeleton className="h-6 w-2/3" />
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-10 w-full" />
      </div>
    );
  }

  if (snapshot.isError) {
    return (
      <EmptyState
        title="Could not read the download"
        description={
          snapshot.error instanceof Error ? snapshot.error.message : undefined
        }
        action={<Button onClick={close}>Close</Button>}
      />
    );
  }

  if (!download) {
    return (
      <EmptyState
        title="This download is no longer in the list"
        description="It may have been removed or completed in another window."
        action={<Button onClick={close}>Close</Button>}
      />
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
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <header className="flex shrink-0 items-center gap-2 border-b border-border px-4 py-3">
        <h1 className="min-w-0 flex-1 truncate text-base font-semibold">
          {reviewing
            ? "Download File Info"
            : `${percent !== null ? `${Math.floor(percent)}% ` : ""}${
                download.filename || "Download status"
              }`}
        </h1>
        {!reviewing ? (
          <Badge variant="outline" className="shrink-0">
            {statusLabel(status)}
          </Badge>
        ) : null}
      </header>

      <main className="flex-1 space-y-4 overflow-auto p-4">
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
            <Tabs value={tab} onValueChange={setTab}>
              <TabsList>
                <TabsTrigger value="status">Download status</TabsTrigger>
                <TabsTrigger value="speed">Speed Limiter</TabsTrigger>
                <TabsTrigger value="completion">Options on completion</TabsTrigger>
              </TabsList>

              <TabsContent value="status" className="pt-3">
                <StatusPanel
                  download={download}
                  busy={action.isPending}
                  onRetry={() => action.mutate("retry")}
                />
              </TabsContent>

              <TabsContent value="speed" className="pt-3">
                <SpeedLimitPanel settings={settings} onApply={applySpeed} />
              </TabsContent>

              <TabsContent value="completion" className="pt-3">
                <CompletionPanel
                  download={download}
                  settings={settings}
                  retrying={retryActions.isPending}
                  onSaveToggles={saveCompletionToggles}
                  onRetryActions={() => retryActions.mutate()}
                  onOpen={() => action.mutate("open")}
                  onReveal={() => action.mutate("reveal")}
                />
              </TabsContent>
            </Tabs>

            {percent === null &&
            (status === "downloading" || status === "connecting") ? (
              <div
                role="progressbar"
                aria-label="Download progress"
                className="h-1.5 w-full overflow-hidden rounded-full bg-muted"
              >
                <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
              </div>
            ) : (
              <Progress
                value={percent ?? 0}
                aria-label="Download progress"
              />
            )}

            {status !== "completed" ? (
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    const next = !collapsed;
                    setCollapsed(next);
                    void progressApi.resize(!next);
                  }}
                >
                  {collapsed ? ">> Show details" : "<< Hide details"}
                </Button>
                <span className="flex-1" />
                <Button
                  type="button"
                  size="sm"
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
                </Button>
                <Button
                  type="button"
                  size="sm"
                  disabled={!canPause(download) || action.isPending}
                  onClick={() => action.mutate("pause")}
                >
                  Pause
                </Button>
                <Button
                  type="button"
                  size="sm"
                  disabled={!canCancel(download) || action.isPending}
                  onClick={() => action.mutate("cancel")}
                >
                  Cancel
                </Button>
              </div>
            ) : null}

            {!collapsed && status !== "failed" ? (
              <SegmentSection download={download} />
            ) : null}
          </>
        ) : null}
      </main>

      <footer className="flex shrink-0 justify-end border-t border-border px-4 py-3">
        <Button type="button" variant="outline" onClick={close}>
          Close
        </Button>
      </footer>
    </div>
  );
}
