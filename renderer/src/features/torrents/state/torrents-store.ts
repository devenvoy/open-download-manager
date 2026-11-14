import { create } from "zustand";

export type TorrentTab = "general" | "files" | "trackers";

interface TorrentsState {
  /** UI state only: what the window shows, never the engine's truth. */
  selectedId: number | null;
  tab: TorrentTab;
  /** File indices the user ticked, or null to follow the engine's flags. */
  fileSelection: Set<number> | null;
  /** Ticks kept apart from the selected torrent while a review is on screen. */
  reviewSelection: Set<number> | null;
  /** Info hashes the user already answered; a late review must not return. */
  answeredReviews: Set<string>;
  /** Info hash of the torrent the visible file list belongs to. */
  filesTorrentHash: string | null;
  select: (id: number | null) => void;
  setTab: (tab: TorrentTab) => void;
  setFileSelection: (selection: Set<number> | null) => void;
  toggleFile: (index: number) => void;
  setAllFiles: (indices: number[], selected: boolean) => void;
  setReviewSelection: (selection: Set<number> | null) => void;
  toggleReviewFile: (index: number) => void;
  markReviewAnswered: (infoHash: string) => void;
  setFilesTorrentHash: (hash: string | null) => void;
  reset: () => void;
}

export const useTorrentsStore = create<TorrentsState>()((set) => ({
  selectedId: null,
  tab: "general",
  fileSelection: null,
  reviewSelection: null,
  answeredReviews: new Set(),
  filesTorrentHash: null,

  select: (id) => set({ selectedId: id }),
  setTab: (tab) => set({ tab }),
  setFileSelection: (selection) => set({ fileSelection: selection }),
  toggleFile: (index) =>
    set((state) => {
      const next = new Set(state.fileSelection ?? []);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return { fileSelection: next };
    }),
  setAllFiles: (indices, selected) =>
    set({ fileSelection: selected ? new Set(indices) : new Set() }),
  setReviewSelection: (selection) => set({ reviewSelection: selection }),
  toggleReviewFile: (index) =>
    set((state) => {
      const next = new Set(state.reviewSelection ?? []);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return { reviewSelection: next };
    }),
  markReviewAnswered: (infoHash) =>
    set((state) => ({
      answeredReviews: new Set(state.answeredReviews).add(infoHash),
    })),
  setFilesTorrentHash: (hash) => set({ filesTorrentHash: hash }),
  reset: () =>
    set({
      selectedId: null,
      tab: "general",
      fileSelection: null,
      reviewSelection: null,
      filesTorrentHash: null,
    }),
}));
