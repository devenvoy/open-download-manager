import { type ReactNode } from "react";
import { EmptyState } from "@/components/feedback/EmptyState";
import { AppShell } from "@/components/layout/AppShell";
import { Skeleton } from "@/components/ui/skeleton";
import { useHydrateTheme } from "@/stores/theme-store";
import { type DownloadActionKind } from "@/types/ipc";
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
  /** Composed by the app layer, so this feature never imports another one. */
  headerActions?: ReactNode;
}

/** The main window. */
export function DownloadsScreen({ headerActions }: DownloadsScreenProps) {
  const { data, isPending, isError, error } = useDownloads();
  const selectedIds = useDownloadsStore((state) => state.selectedIds);
  const category = useDownloadsStore((state) => state.category);
  const toggleSelected = useDownloadsStore((state) => state.toggleSelected);
  const setSelection = useDownloadsStore((state) => state.setSelection);
  const clearSelection = useDownloadsStore((state) => state.clearSelection);
  const action = useDownloadAction();
  const remove = useRemoveDownloads();

  useHydrateTheme(data?.settings.theme);

  const visible = filterDownloads(data?.downloads ?? [], category);
  const selectedVisible = visibleSelection(selectedIds, visible);

  function handleAction(id: string, kind: DownloadActionKind) {
    action.mutate({ id, action: kind });
  }

  function handleRemove(ids: string[]) {
    remove.mutate(ids, { onSuccess: clearSelection });
  }

  return (
    <AppShell
      toolbar={
        <>
          <DownloadsToolbar
            selectedIds={selectedVisible}
            onAction={handleAction}
            onRemove={handleRemove}
            removing={remove.isPending}
          />
          {headerActions}
        </>
      }
      sidebar={<CategorySidebar />}
    >
      <div className="space-y-4">
        <h1 className="text-lg font-semibold">Downloads</h1>
        {isPending ? (
          <div className="space-y-2" aria-label="Loading downloads">
            {Array.from({ length: 6 }, (_, index) => (
              <Skeleton key={index} className="h-9 w-full" />
            ))}
          </div>
        ) : isError ? (
          <EmptyState
            title="Could not load downloads"
            description={error instanceof Error ? error.message : undefined}
          />
        ) : (
          <DownloadsTable
            downloads={visible}
            selectedIds={selectedVisible}
            onToggle={toggleSelected}
            onToggleAll={(checked) =>
              checked
                ? setSelection(visible.map((download) => download.id))
                : clearSelection()
            }
            onAction={handleAction}
          />
        )}
      </div>
    </AppShell>
  );
}
