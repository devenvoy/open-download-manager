import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ipc } from "@/lib/ipc";
import { snapshotQueryKey, snapshotQueryOptions } from "@/lib/ipc/queries";
import { type DownloadRequest } from "@/types/ipc";

/** The Add-window IPC seam. */
export const addApi = {
  add: (request: DownloadRequest) => ipc.addDownload(request),
  queues: () => ipc.listQueues(),
  clipboard: () => ipc.readClipboard(),
  chooseDirectory: () => ipc.chooseDirectory(),
  playlist: (url: string) => ipc.playlistInfo(url),
  formats: (url: string) => ipc.videoFormats(url),
  addPlaylist: (request: {
    playlistId: string;
    playlistTitle: string;
    items: Array<{ id: string; url: string }>;
    commonCategory?: string;
    commonDirectory?: string;
  }) => ipc.addPlaylistDownloads(request),
};

export const addKeys = {
  queues: ["add", "queues"] as const,
};

export const addQueuesQuery = queryOptions({
  queryKey: addKeys.queues,
  queryFn: () => addApi.queues(),
  staleTime: 10_000,
});

export function useAddQueues() {
  return useQuery(addQueuesQuery);
}

/** Playlist metadata, fetched only while its dialog is open. */
export function usePlaylistInfo(url: string | null) {
  return useQuery({
    queryKey: ["add", "playlist", url] as const,
    queryFn: () => addApi.playlist(url as string),
    enabled: url !== null,
    staleTime: 60_000,
    retry: false,
  });
}

/** Video formats, fetched only while its dialog is open. */
export function useVideoFormats(url: string | null) {
  return useQuery({
    queryKey: ["add", "formats", url] as const,
    queryFn: () => addApi.formats(url as string),
    enabled: url !== null,
    staleTime: 60_000,
    retry: false,
  });
}

/** The snapshot carries the default folder and the category overrides. */
export function useAddDefaults() {
  return useQuery({
    ...snapshotQueryOptions,
    refetchInterval: false,
    staleTime: Infinity,
  });
}

export function useSubmitDownload() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (request: DownloadRequest) => addApi.add(request),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: snapshotQueryKey });
    },
    onError: (error) =>
      toast.error(error instanceof Error ? error.message : "Could not add the download."),
  });
}

export function useAddPlaylist() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: addApi.addPlaylist,
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: snapshotQueryKey });
    },
  });
}
