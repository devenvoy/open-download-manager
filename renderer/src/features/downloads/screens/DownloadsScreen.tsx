import { type ReactNode, useMemo } from "react";
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

export interface DownloadsScreenProps {
  headerActions?: ReactNode;
}

export function DownloadsScreen({ headerActions }: DownloadsScreenProps = {}) {
  const { data, isPending } = useDownloads();
  const selectedIds = useDownloadsStore((state) => state.selectedIds);
  const category = useDownloadsStore((state) => state.category);
  const toggleSelected = useDownloadsStore((state) => state.toggleSelected);
  const setSelection = useDownloadsStore((state) => state.setSelection);
  const clearSelection = useDownloadsStore((state) => state.clearSelection);
  const action = useDownloadAction();
  const remove = useRemoveDownloads();

  useHydrateTheme(data?.settings.theme);

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

  function handleAction(id: string, kind: DownloadActionKind) {
    action.mutate({ id, action: kind });
  }

  function handleResume() {
    selectedVisible.forEach((id) => handleAction(id, "resume"));
  }

  function handleStop() {
    selectedVisible.forEach((id) => handleAction(id, "cancel"));
  }

  function handleStopAll() {
    (data?.downloads ?? [])
      .filter((d) => d.status === "downloading")
      .forEach((d) => handleAction(d.id, "cancel"));
  }

  function handleDelete() {
    if (selectedVisible.length > 0) {
      remove.mutate(selectedVisible, { onSuccess: clearSelection });
    }
  }

  function handleDeleteCompleted() {
    downloadsApi.removeCompleted();
  }

  const activeDownloading = (data?.downloads ?? []).some(
    (d) => d.status === "downloading",
  );

  return (
    <AppShell
      toolbar={
        <>
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
          {headerActions}
        </>
      }
      sidebar={<CategorySidebar />}
      footer={
        <footer className="status-bar">
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
        downloads={visible}
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
      />
    </AppShell>
  );
}
