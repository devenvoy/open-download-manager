import { create } from "zustand";
import { type DownloadCategory, type DownloadSummary } from "@/types/ipc";

export type CategoryFilter =
  | DownloadCategory
  | "all"
  | "unfinished"
  | "completed"
  | "queued";

interface DownloadsState {
  /** UI state only, serialisable (plan R6). */
  selectedIds: string[];
  category: CategoryFilter;
  toggleSelected: (id: string) => void;
  setSelection: (ids: string[]) => void;
  clearSelection: () => void;
  setCategory: (category: CategoryFilter) => void;
}

export const useDownloadsStore = create<DownloadsState>()((set) => ({
  selectedIds: [],
  category: "all",
  toggleSelected: (id) =>
    set((state) => ({
      selectedIds: state.selectedIds.includes(id)
        ? state.selectedIds.filter((selected) => selected !== id)
        : [...state.selectedIds, id],
    })),
  setSelection: (ids) => set({ selectedIds: [...new Set(ids)] }),
  clearSelection: () => set({ selectedIds: [] }),
  setCategory: (category) => set({ category }),
}));

/** Pure selector: the rows a category or status filter leaves visible. */
export function filterDownloads(
  downloads: DownloadSummary[],
  category: CategoryFilter,
): DownloadSummary[] {
  if (category === "all") return downloads;
  if (category === "unfinished")
    return downloads.filter((d) => d.status !== "complete");
  if (category === "completed")
    return downloads.filter((d) => d.status === "complete");
  if (category === "queued")
    return downloads.filter((d) =>
      ["queued", "scheduled", "paused"].includes(d.status),
    );
  return downloads.filter((download) => download.category === category);
}

/** Pure selector: the selected ids that are still present in a visible list. */
export function visibleSelection(
  selectedIds: string[],
  visible: DownloadSummary[],
): string[] {
  const present = new Set(visible.map((download) => download.id));
  return selectedIds.filter((id) => present.has(id));
}
