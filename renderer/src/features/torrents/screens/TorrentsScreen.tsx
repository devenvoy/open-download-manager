import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
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
import { AddTorrentDialog } from "../components/AddTorrentDialog";
import { TorrentDetails } from "../components/TorrentDetails";
import { TorrentEngineBanner } from "../components/TorrentEngineBanner";
import { TorrentList } from "../components/TorrentList";
import { TorrentRemoveDialog } from "../components/TorrentRemoveDialog";
import { TorrentReviewPanel } from "../components/TorrentReviewPanel";
import { TorrentSettingsDialog } from "../components/TorrentSettingsDialog";
import { TorrentToolbar } from "../components/TorrentToolbar";

/** The standalone Torrents window (`torrents.html`). */
export function TorrentsScreen() {
  const status = useTorrentEngine();
  const list = useTorrentList();
  const settings = useTorrentSettings();
  const [review, setReview] = usePendingReview();
  const actions = useTorrentActions();

  const [addMode, setAddMode] = useState<"magnet" | "file" | null>(null);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const selectedId = useTorrentsStore((state) => state.selectedId);
  const select = useTorrentsStore((state) => state.select);
  const tab = useTorrentsStore((state) => state.tab);
  const selection = useTorrentsStore((state) => state.fileSelection);
  const setFileSelection = useTorrentsStore((state) => state.setFileSelection);
  const toggleFile = useTorrentsStore((state) => state.toggleFile);
  const setAllFiles = useTorrentsStore((state) => state.setAllFiles);
  const answered = useTorrentsStore((state) => state.answeredReviews);
  const markReviewAnswered = useTorrentsStore((state) => state.markReviewAnswered);

  // The torrent window needs the app theme, which lives in the manager settings.
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

  const handleAction = useCallback(
    (kind: "pause" | "resume" | "recheck" | "forget" | "delete") => {
      if (!selected) return;
      actions.action.mutate(
        { id: selected.id, action: kind },
        {
          onSuccess: () => {
            if (kind === "forget" || kind === "delete") select(null);
          },
        },
      );
    },
    [selected, actions.action, select],
  );

  const handleApplyFiles = useCallback(() => {
    if (!selected) return;
    const included = [...(selection ?? includedFiles)].sort((a, b) => a - b);
    actions.setFiles.mutate(
      { id: selected.id, included },
      { onSuccess: () => setFileSelection(null) },
    );
  }, [selected, selection, includedFiles, actions.setFiles, setFileSelection]);

  const handleConfirmReview = useCallback(
    (included: number[], startPaused: boolean) => {
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
    },
    [review, markReviewAnswered, actions.setFiles, actions.action, setReview],
  );

  const handleCancelReview = useCallback(() => {
    if (!review) return;
    markReviewAnswered(review.infoHash);
    actions.action.mutate({ id: review.id, action: "forget" });
    setReview(null);
  }, [review, markReviewAnswered, actions.action, setReview]);

  const busy =
    actions.action.isPending ||
    actions.addFile.isPending ||
    actions.addMagnet.isPending ||
    actions.setFiles.isPending;

  return (
    <div className="torrent-shell">
      <TorrentToolbar
        summary={summary}
        selected={selected}
        busy={busy}
        onOpenAdd={(mode) => setAddMode(mode)}
        onOpenRemove={() => setRemoveOpen(true)}
        onOpenSettings={() => setSettingsOpen(true)}
        onAction={handleAction}
      />

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

      <div className="torrent-main">
        <section className="torrent-list-pane" aria-label="Torrents">
          <div className="torrent-list-header">
            <span>Name</span>
            <span>Size</span>
            <span>Status</span>
          </div>
          <div
            id="torrent-list"
            className="torrent-list"
            role="listbox"
            tabIndex={0}
          >
            {list.isPending ? (
              <div className="p-3" aria-label="Loading torrents">
                Loading torrents…
              </div>
            ) : torrents.length === 0 ? (
              <div id="torrent-empty" className="torrent-empty">
                <strong>No torrents yet</strong>
                <span>
                  Add a magnet link or open a .torrent file to start downloading.
                </span>
              </div>
            ) : (
              <TorrentList
                torrents={torrents}
                selectedId={selectedId}
                onSelect={select}
              />
            )}
          </div>
        </section>

        <section className="torrent-detail-pane" aria-label="Torrent details">
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
            <div id="torrent-detail-empty" className="torrent-detail-empty">
              <strong>Select a torrent</strong>
              <span>
                Choose a torrent in the list to see its progress, files, and trackers.
              </span>
            </div>
          )}
        </section>
      </div>

      {addMode && (
        <AddTorrentDialog
          initialMode={addMode}
          onAddMagnet={(magnet, opts) => actions.addMagnet.mutate({ magnet, options: opts })}
          onAddFile={(path, opts) => actions.addFile.mutate({ path, options: opts })}
          onClose={() => setAddMode(null)}
          pending={busy}
        />
      )}

      {removeOpen && selected && (
        <TorrentRemoveDialog
          torrent={selected}
          disabled={busy}
          onAction={handleAction}
          onClose={() => setRemoveOpen(false)}
        />
      )}

      {settingsOpen && settings.data && (
        <TorrentSettingsDialog
          settings={settings.data}
          saving={actions.saveSettings.isPending}
          onSave={(next) => actions.saveSettings.mutate(next)}
          onRestart={() => actions.restartEngine.mutate()}
          restarting={actions.restartEngine.isPending}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}
