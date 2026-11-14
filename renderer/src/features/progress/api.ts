import {
  queryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { ipc } from "@/lib/ipc";
import { snapshotQueryKey } from "@/lib/ipc/queries";
import {
  type CompletionAction,
  type DownloadActionKind,
  type MediaToolType,
  type SettingsPatch,
} from "@/types/ipc";

/** The progress window's IPC seam. Components never import `@/lib/ipc`. */
export const progressApi = {
  snapshot: () => ipc.snapshot(),
  action: (id: string, action: DownloadActionKind) =>
    ipc.downloadAction(id, action),
  updateSettings: (patch: SettingsPatch) => ipc.updateSettings(patch),
  updateCompletionActions: (id: string, actions: CompletionAction[] | null) =>
    ipc.updateCompletionActions(id, actions),
  retryCompletionActions: (id: string) => ipc.retryCompletionActions(id),
  resize: (expanded: boolean) => ipc.resizeProgressWindow(expanded),
  close: () => ipc.closeAuxiliaryWindow(),
  chooseDirectory: () => ipc.chooseDirectory(),
  installMediaTool: (tool: MediaToolType) => ipc.installMediaTool(tool),
  review: (
    id: string,
    patch: Parameters<typeof ipc.reviewDownload>[1],
  ) => ipc.reviewDownload(id, patch),
};

/**
 * The window reads the shared snapshot, but faster: the transfer panel must
 * follow a running download closely.
 */
export const progressSnapshotQuery = queryOptions({
  queryKey: ["snapshot"] as const,
  queryFn: () => progressApi.snapshot(),
  refetchInterval: 1_000,
  staleTime: 500,
});

export function useProgressSnapshot() {
  return useQuery(progressSnapshotQuery);
}

function useInvalidateSnapshot() {
  const client = useQueryClient();
  return () => client.invalidateQueries({ queryKey: snapshotQueryKey });
}

export function useDownloadAction(id: string) {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: (action: DownloadActionKind) => progressApi.action(id, action),
    onSuccess: invalidate,
    onError: (error) =>
      toast.error(
        error instanceof Error ? error.message : "The action could not be completed.",
      ),
  });
}

export function useUpdateSettings() {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: (patch: SettingsPatch) => progressApi.updateSettings(patch),
    onSuccess: invalidate,
  });
}

export function useRetryCompletionActions(id: string) {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: () => progressApi.retryCompletionActions(id),
    onSuccess: async () => {
      await invalidate();
      toast.success("Post-download actions re-executed.");
    },
    onError: (error) =>
      toast.error(
        error instanceof Error ? error.message : "Could not re-run the actions.",
      ),
  });
}

/** Installs FFmpeg or yt-dlp, then the caller retries the download. */
export function useInstallMediaTool() {
  return useMutation({
    mutationFn: (tool: MediaToolType) => progressApi.installMediaTool(tool),
    onError: (error) =>
      toast.error(
        error instanceof Error ? error.message : "The installation failed.",
      ),
  });
}

export function useReviewDownload(id: string) {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: (patch: Parameters<typeof ipc.reviewDownload>[1]) =>
      progressApi.review(id, patch),
    onSuccess: invalidate,
    onError: (error) =>
      toast.error(
        error instanceof Error ? error.message : "Could not save the download.",
      ),
  });
}

export function useChooseDirectory() {
  return useMutation({ mutationFn: () => progressApi.chooseDirectory() });
}
