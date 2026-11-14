import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { snapshotQueryKey, snapshotQueryOptions } from "@/lib/ipc/queries";
import {
  type DownloadActionKind,
  type DownloadRequest,
} from "@/types/ipc";
import { downloadsApi } from "../api";

/** The one snapshot query every download screen reads (plan R1). */
export function useDownloads() {
  return useQuery(snapshotQueryOptions);
}

function useInvalidateSnapshot() {
  const queryClient = useQueryClient();
  return () =>
    queryClient.invalidateQueries({ queryKey: snapshotQueryKey });
}

export function useDownloadAction() {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: ({
      id,
      action,
    }: {
      id: string;
      action: DownloadActionKind;
    }) => downloadsApi.action(id, action),
    onSuccess: invalidate,
  });
}

export function useRemoveDownloads() {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: (ids: string[]) =>
      Promise.all(ids.map((id) => downloadsApi.action(id, "remove"))),
    onSuccess: invalidate,
  });
}

export function useAddDownload() {
  const invalidate = useInvalidateSnapshot();
  return useMutation({
    mutationFn: (request: DownloadRequest) => downloadsApi.add(request),
    onSuccess: invalidate,
  });
}
