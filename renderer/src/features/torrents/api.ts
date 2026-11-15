import { queryOptions } from "@tanstack/react-query";
import { ipc } from "@/lib/ipc";
import { type TorrentSettings } from "@/types/ipc";

/**
 * The IPC seam for the torrents feature. Components and hooks import from
 * here, never from `@/lib/ipc` directly.
 */
export const torrentsApi = {
  status: () => ipc.torrentStatus(),
  list: () => ipc.torrentList(),
  files: (id: number) => ipc.torrentFiles(id),
  trackers: (id: number) => ipc.torrentTrackerStatus(id),
  settings: () => ipc.torrentSettingsGet(),
  saveSettings: (settings: TorrentSettings) => ipc.torrentSettingsSet(settings),
  addMagnet: (magnet: string, options?: Parameters<typeof ipc.torrentAddMagnet>[1]) =>
    ipc.torrentAddMagnet(magnet, options),
  addFile: (path: string, options?: Parameters<typeof ipc.torrentAddFile>[1]) =>
    ipc.torrentAddFile(path, options),
  action: (id: number, action: Parameters<typeof ipc.torrentAction>[1]) =>
    ipc.torrentAction(id, action),
  setFiles: (id: number, included: number[]) => ipc.torrentSetFiles(id, included),
  setWatching: (watching: boolean) => ipc.torrentSetWatching(watching),
  restartEngine: () => ipc.torrentRestartEngine(),
  takeReview: () => ipc.torrentTakeReview(),
  chooseDirectory: () => ipc.chooseDirectory(),
  chooseTorrentFile: () =>
    ipc.chooseFile([{ name: "Torrent", extensions: ["torrent"] }]),
};

export const torrentKeys = {
  status: ["torrents", "status"] as const,
  list: ["torrents", "list"] as const,
  settings: ["torrents", "settings"] as const,
  files: (id: number) => ["torrents", "files", id] as const,
  trackers: (id: number) => ["torrents", "trackers", id] as const,
  review: ["torrents", "review"] as const,
};

export const torrentStatusQuery = queryOptions({
  queryKey: torrentKeys.status,
  queryFn: () => torrentsApi.status(),
  staleTime: 5_000,
});

/**
 * The list is pushed by `torrent://progress` while the window is open, so the
 * poll here is only the safety net for the gap before the first event and for
 * a dropped engine. It is much slower than the push interval.
 */
export const torrentListQuery = queryOptions({
  queryKey: torrentKeys.list,
  queryFn: () => torrentsApi.list(),
  staleTime: 5_000,
  refetchInterval: 10_000,
});

export const torrentSettingsQuery = queryOptions({
  queryKey: torrentKeys.settings,
  queryFn: () => torrentsApi.settings(),
  staleTime: 30_000,
});

export const torrentFilesQuery = (id: number) =>
  queryOptions({
    queryKey: torrentKeys.files(id),
    queryFn: () => torrentsApi.files(id),
    staleTime: 1_000,
    refetchInterval: 1_000,
  });

export const torrentTrackersQuery = (id: number) =>
  queryOptions({
    queryKey: torrentKeys.trackers(id),
    queryFn: () => torrentsApi.trackers(id),
    staleTime: 4_000,
    refetchInterval: 5_000,
  });
