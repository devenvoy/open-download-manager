import { useCallback, useEffect, useMemo, useRef } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { formatSpeed } from "@/lib/format";
import { useHydrateTheme } from "@/stores/theme-store";
import { type DownloadActionKind } from "@/types/ipc";
import { downloadsApi } from "../api";
import { CategorySidebar } from "../components/CategorySidebar";
import { DownloadsTable } from "../components/DownloadsTable";
import { DownloadsToolbar } from "../components/DownloadsToolbar";
import {
  useDownloadAction,
  useDownloads,
  useRemoveDownloads,
} from "../hooks/use-downloads";
import {
  filterDownloads,
  useDownloadsStore,
  visibleSelection,
} from "../state/downloads-store";

export function DownloadsScreen() {
  const { data, isPending } = useDownloads();
  const selectedIds = useDownloadsStore((state) => state.selectedIds);
  const category = useDownloadsStore((state) => state.category);
  const toggleSelected = useDownloadsStore((state) => state.toggleSelected);
  const setSelection = useDownloadsStore((state) => state.setSelection);
  const clearSelection = useDownloadsStore((state) => state.clearSelection);
  const action = useDownloadAction();
  const remove = useRemoveDownloads();

  useHydrateTheme(data?.settings.theme);

  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        void downloadsApi.openAddWindow();
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const visible = useMemo(
    () => filterDownloads(data?.downloads ?? [], category),
    [data?.downloads, category],
  );
  const selectedVisible = visibleSelection(selectedIds, visible);

  const totalSpeed = useMemo(() => {
    return (data?.downloads ?? []).reduce(
      (acc, d) => (d.status === "downloading" ? acc + (d.speed || 0) : acc),
      0,
    );
  }, [data?.downloads]);

  const handleAction = useCallback(
    (id: string, kind: DownloadActionKind) => {
      action.mutate({ id, action: kind });
    },
    [action],
  );

  const handleResume = useCallback(() => {
    selectedVisible.forEach((id) => {
      const d = data?.downloads.find((item) => item.id === id);
      if (d?.needsReview) {
        void downloadsApi.openProgressWindow(d.id);
        return;
      }
      const kind: DownloadActionKind =
        d && (d.status === "error" || d.status === "cancelled")
          ? "retry"
          : "resume";
      handleAction(id, kind);
    });
  }, [selectedVisible, data?.downloads, handleAction]);

  const handleStop = useCallback(() => {
    selectedVisible.forEach((id) => handleAction(id, "cancel"));
  }, [selectedVisible, handleAction]);

  const handleStopAll = useCallback(() => {
    (data?.downloads ?? [])
      .filter((d) => d.status === "downloading")
      .forEach((d) => handleAction(d.id, "cancel"));
  }, [data?.downloads, handleAction]);

  const handleDelete = useCallback(() => {
    if (selectedVisible.length > 0) {
      remove.mutate(selectedVisible, { onSuccess: clearSelection });
    }
  }, [selectedVisible, remove, clearSelection]);

  const handleDeleteCompleted = useCallback(() => {
    downloadsApi.removeCompleted();
  }, []);

  const activeDownloading = (data?.downloads ?? []).some(
    (d) => d.status === "downloading",
  );

  return (
    <AppShell
      toolbar={
        <DownloadsToolbar
          selectedIds={selectedVisible}
          canResume={selectedVisible.length > 0}
          canStop={selectedVisible.length > 0}
          canStopAll={activeDownloading}
          canDelete={selectedVisible.length > 0}
          canDeleteCompleted={true}
          onAdd={() => downloadsApi.openAddWindow()}
          onResume={handleResume}
          onStop={handleStop}
          onStopAll={handleStopAll}
          onDelete={handleDelete}
          onDeleteCompleted={handleDeleteCompleted}
          onOpenOptions={() => downloadsApi.openOptionsWindow()}
          onOpenScheduler={() => downloadsApi.openSchedulerWindow()}
          onStartQueue={() => downloadsApi.startQueue()}
          onStopQueue={() => downloadsApi.pauseQueue()}
          onOpenIntegration={() => downloadsApi.openIntegrationWindow()}
          onOpenTorrents={() => downloadsApi.openTorrentWindow()}
        />
      }
      sidebar={<CategorySidebar />}
      footer={
        <footer className="status-bar h-[22px] px-2 bg-[#f0f0f0] dark:bg-[#252526] border-t border-[#ccc] dark:border-[#383838] flex items-center justify-between text-[11px] text-[#555] dark:text-[#999] shrink-0 select-none">
          <span id="list-count">
            {isPending
              ? "Connecting…"
              : `${visible.length} download${visible.length === 1 ? "" : "s"}`}
          </span>
          <span id="status-summary">{formatSpeed(totalSpeed)}</span>
          <span id="connection-label">Connected</span>
        </footer>
      }
    >
      <DownloadsTable
        searchRef={searchInputRef}
        downloads={visible}
        fileIcons={data?.fileIcons}
        selectedIds={selectedVisible}
        onToggle={toggleSelected}
        onSelectRow={(id, { additive, toggle }) => {
          if (toggle) {
            toggleSelected(id);
          } else if (additive) {
            setSelection([...selectedIds, id]);
          } else {
            setSelection([id]);
          }
        }}
        onAction={handleAction}
        onOpenProperties={(id) => downloadsApi.openProgressWindow(id)}
        onOpenFolder={(id) => handleAction(id, "reveal")}
      />
    </AppShell>
  );
}
