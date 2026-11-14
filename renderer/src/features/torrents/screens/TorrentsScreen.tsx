import { useEffect, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { EmptyState } from "@/components/feedback/EmptyState";
import { AppShell } from "@/components/layout/AppShell";
import { Skeleton } from "@/components/ui/skeleton";
import { snapshotQueryOptions } from "@/lib/ipc/queries";
import { useHydrateTheme } from "@/stores/theme-store";
import {
  usePendingReview,
  useTorrentActions,
  useTorrentEngine,
  useTorrentFiles,
  useTorrentList,
  useTorrentSettings,
  useTorrentTrackers,
} from "../hooks/use-torrents";
import { summarize } from "../model/torrent-view";
import { useTorrentsStore } from "../state/torrents-store";
import { TorrentDetails } from "../components/TorrentDetails";
import { TorrentEngineBanner } from "../components/TorrentEngineBanner";
import { TorrentList } from "../components/TorrentList";
import { TorrentReviewPanel } from "../components/TorrentReviewPanel";
import { TorrentSettingsDialog } from "../components/TorrentSettingsDialog";
import {
  TorrentToolbar,
  type AddTorrentOptionsInput,
} from "../components/TorrentToolbar";

/** The standalone Torrents window (`torrents.html`). */
export function TorrentsScreen() {
  const status = useTorrentEngine();
  const list = useTorrentList();
  const settings = useTorrentSettings();
  const [review, setReview] = usePendingReview();
  const actions = useTorrentActions();

  const selectedId = useTorrentsStore((state) => state.selectedId);
  const select = useTorrentsStore((state) => state.select);
  const tab = useTorrentsStore((state) => state.tab);
  const selection = useTorrentsStore((state) => state.fileSelection);
  const setFileSelection = useTorrentsStore((state) => state.setFileSelection);
  const toggleFile = useTorrentsStore((state) => state.toggleFile);
  const setAllFiles = useTorrentsStore((state) => state.setAllFiles);
  const answered = useTorrentsStore((state) => state.answeredReviews);
  const markReviewAnswered = useTorrentsStore((state) => state.markReviewAnswered);

  // The torrent window needs the app theme, which lives in the manager
  // settings. One snapshot read is enough; this window does not poll it.
  const snapshot = useQuery({
    ...snapshotQueryOptions,
    refetchInterval: false,
    staleTime: Infinity,
  });
  useHydrateTheme(snapshot.data?.settings.theme);

  const torrents = useMemo(() => list.data ?? [], [list.data]);
  const selected = torrents.find((torrent) => torrent.id === selectedId) ?? null;
  const summary = useMemo(() => summarize(torrents), [torrents]);

  const filesQuery = useTorrentFiles(selected?.id ?? null);
  const reviewFilesQuery = useTorrentFiles(review?.id ?? null);
  const trackersQuery = useTorrentTrackers(selected?.id ?? null, tab === "trackers");

  // Keep one row selected so the detail panel is never empty by accident.
  useEffect(() => {
    if (selectedId !== null) {
      if (torrents.some((torrent) => torrent.id === selectedId)) return;
      select(torrents[0]?.id ?? null);
      return;
    }
    if (torrents[0]) select(torrents[0].id);
  }, [torrents, selectedId, select]);

  // A review that the user already answered must not come back.
  useEffect(() => {
    if (review && answered.has(review.infoHash)) setReview(null);
  }, [review, answered, setReview]);

  // A new torrent means a new file list; drop the old ticks.
  useEffect(() => {
    setFileSelection(null);
  }, [selected?.infoHash, setFileSelection]);

  const files = useMemo(() => filesQuery.data ?? [], [filesQuery.data]);
  const includedFiles = useMemo(
    () => new Set(files.filter((file) => file.included).map((file) => file.index)),
    [files],
  );
  const filesDirty =
    selection !== null &&
    (selection.size !== includedFiles.size ||
      [...selection].some((index) => !includedFiles.has(index)));

  function handleAction(kind: "pause" | "resume" | "recheck" | "forget" | "delete") {
    if (!selected) return;
    actions.action.mutate(
      { id: selected.id, action: kind },
      {
        onSuccess: () => {
          if (kind === "forget" || kind === "delete") select(null);
        },
      },
    );
  }

  function handleApplyFiles() {
    if (!selected) return;
    const included = [...(selection ?? includedFiles)].sort((a, b) => a - b);
    actions.setFiles.mutate(
      { id: selected.id, included },
      { onSuccess: () => setFileSelection(null) },
    );
  }

  function handleAddMagnet(magnet: string, options: AddTorrentOptionsInput) {
    actions.addMagnet.mutate({
      magnet,
      options: {
        outputFolder: options.outputFolder,
        startPaused: options.startPaused,
        review: options.review,
      },
    });
  }

  function handleAddFile(path: string, options: AddTorrentOptionsInput) {
    actions.addFile.mutate({
      path,
      options: {
        outputFolder: options.outputFolder,
        startPaused: options.startPaused,
        review: options.review,
      },
    });
  }

  function handleConfirmReview(included: number[], startPaused: boolean) {
    if (!review) return;
    markReviewAnswered(review.infoHash);
    actions.setFiles.mutate(
      { id: review.id, included },
      {
        onSuccess: () => {
          actions.action.mutate({
            id: review.id,
            action: startPaused ? "pause" : "resume",
          });
          setReview(null);
        },
      },
    );
  }

  function handleCancelReview() {
    if (!review) return;
    markReviewAnswered(review.infoHash);
    actions.action.mutate({ id: review.id, action: "forget" });
    setReview(null);
  }

  const busy =
    actions.action.isPending ||
    actions.addFile.isPending ||
    actions.addMagnet.isPending ||
    actions.setFiles.isPending;

  return (
    <AppShell
      toolbar={
        <TorrentToolbar
          summary={summary}
          selected={selected}
          busy={busy}
          onAction={handleAction}
          onAddMagnet={handleAddMagnet}
          onAddFile={handleAddFile}
          settingsButton={
            settings.data ? (
              <TorrentSettingsDialog
                settings={settings.data}
                saving={actions.saveSettings.isPending}
                onSave={(next) => actions.saveSettings.mutate(next)}
              />
            ) : null
          }
        />
      }
    >
      <div className="space-y-3">
        {status.data ? (
          <TorrentEngineBanner
            status={status.data}
            onEnable={() =>
              settings.data &&
              actions.saveSettings.mutate({ ...settings.data, enabled: true })
            }
            onRestart={() => actions.restartEngine.mutate()}
            restarting={actions.restartEngine.isPending}
          />
        ) : null}

        {review ? (
          <TorrentReviewPanel
            review={review}
            files={reviewFilesQuery.data ?? []}
            loading={reviewFilesQuery.isPending}
            confirming={actions.setFiles.isPending}
            onConfirm={handleConfirmReview}
            onCancel={handleCancelReview}
          />
        ) : null}

        <div className="grid min-h-0 gap-3 lg:grid-cols-[1.4fr_1fr]">
          <div className="min-w-0 overflow-auto rounded-md border border-border">
            {list.isPending ? (
              <div className="space-y-2 p-3" aria-label="Loading torrents">
                {Array.from({ length: 5 }, (_, index) => (
                  <Skeleton key={index} className="h-9 w-full" />
                ))}
              </div>
            ) : list.isError ? (
              <EmptyState
                title="Could not read the torrent list"
                description={
                  list.error instanceof Error ? list.error.message : undefined
                }
              />
            ) : torrents.length === 0 ? (
              <EmptyState
                title="No torrents yet"
                description="Add a magnet link or a .torrent file to start."
              />
            ) : (
              <TorrentList
                torrents={torrents}
                selectedId={selectedId}
                onSelect={select}
              />
            )}
          </div>

          {selected ? (
            <TorrentDetails
              torrent={selected}
              files={files}
              filesLoading={filesQuery.isPending}
              trackers={trackersQuery.data ?? []}
              trackersLoading={trackersQuery.isPending}
              filesDirty={filesDirty}
              applyingFiles={actions.setFiles.isPending}
              onToggleFile={toggleFile}
              onSetAllFiles={(checked) =>
                setAllFiles(
                  files.map((file) => file.index),
                  checked,
                )
              }
              onApplyFiles={handleApplyFiles}
            />
          ) : (
            <div className="flex items-center justify-center rounded-md border border-dashed border-border p-6 text-sm text-muted-foreground">
              Select a torrent to see its details.
            </div>
          )}
        </div>
      </div>
    </AppShell>
  );
}
