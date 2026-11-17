import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { useBackendEvent } from "@/lib/hooks/use-backend-event";
import { type TorrentReviewRequest, type TorrentSettings } from "@/types/ipc";
import {
  torrentFilesQuery,
  torrentKeys,
  torrentListQuery,
  torrentPeersQuery,
  torrentPiecesQuery,
  torrentSettingsQuery,
  torrentStatusQuery,
  torrentTrackersQuery,
  torrentsApi,
} from "../api";
import { friendlyError } from "../model/torrent-view";

/**
 * Engine status, kept live by `torrent://engine`.
 *
 * The window also asks the engine to start pushing progress while it is
 * mounted; `setWatching(false)` on unmount stops the one-second stream so a
 * closed window costs nothing.
 */
export function useTorrentEngine() {
  const client = useQueryClient();
  const status = useQuery(torrentStatusQuery);

  useBackendEvent("torrent://engine", (next) => {
    client.setQueryData(torrentKeys.status, next);
  });

  useEffect(() => {
    void torrentsApi.setWatching(true);
    return () => {
      void torrentsApi.setWatching(false);
    };
  }, []);

  return status;
}

/** The torrent list, replaced wholesale by `torrent://progress`. */
export function useTorrentList() {
  const client = useQueryClient();
  const list = useQuery(torrentListQuery);

  useBackendEvent("torrent://progress", (next) => {
    client.setQueryData(torrentKeys.list, next);
  });

  return list;
}

export function useTorrentSettings() {
  return useQuery(torrentSettingsQuery);
}

export function useTorrentFiles(id: number | null) {
  return useQuery({
    ...torrentFilesQuery(id ?? 0),
    enabled: id !== null,
  });
}

export function useTorrentTrackers(id: number | null, enabled: boolean) {
  return useQuery({
    ...torrentTrackersQuery(id ?? 0),
    enabled: id !== null && enabled,
  });
}

export function useTorrentPeers(id: number | null, enabled: boolean) {
  return useQuery({
    ...torrentPeersQuery(id ?? 0),
    enabled: id !== null && enabled,
  });
}

export function useTorrentPieces(id: number | null, enabled: boolean) {
  return useQuery({
    ...torrentPiecesQuery(id ?? 0),
    enabled: id !== null && enabled,
  });
}

/**
 * The pending review, from the event and from the one-shot hand-over.
 *
 * A review is only ever handed out once: the event drops the stored copy, and
 * the polling call covers a window that opened before the event was sent.
 */
export function usePendingReview() {
  const [review, setReview] = useState<TorrentReviewRequest | null>(null);

  useBackendEvent("torrent://review", (next) => setReview(next));

  useEffect(() => {
    let cancelled = false;
    void torrentsApi.takeReview().then((pending) => {
      if (!cancelled && pending) setReview(pending);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return [review, setReview] as const;
}

/** Every torrent mutation goes through this hook so errors read the same. */
export function useTorrentActions() {
  const client = useQueryClient();

  const refresh = () =>
    Promise.all([
      client.invalidateQueries({ queryKey: torrentKeys.list }),
      client.invalidateQueries({ queryKey: torrentKeys.status }),
    ]);

  const action = useMutation({
    mutationFn: async ({
      id,
      action: kind,
    }: {
      id: number;
      action: Parameters<typeof torrentsApi.action>[1];
    }) => torrentsApi.action(id, kind),
    onSuccess: refresh,
    onError: (error) => toast.error(friendlyError(error)),
  });

  const setFiles = useMutation({
    mutationFn: ({ id, included }: { id: number; included: number[] }) =>
      torrentsApi.setFiles(id, included),
    onSuccess: async (_data, variables) => {
      await client.invalidateQueries({ queryKey: torrentKeys.files(variables.id) });
      await refresh();
    },
    onError: (error) => toast.error(friendlyError(error)),
  });

  const addMagnet = useMutation({
    mutationFn: ({
      magnet,
      options,
    }: {
      magnet: string;
      options?: Parameters<typeof torrentsApi.addMagnet>[1];
    }) => torrentsApi.addMagnet(magnet, options),
    onSuccess: refresh,
    onError: (error) => toast.error(friendlyError(error)),
  });

  const addFile = useMutation({
    mutationFn: ({
      path,
      options,
    }: {
      path: string;
      options?: Parameters<typeof torrentsApi.addFile>[1];
    }) => torrentsApi.addFile(path, options),
    onSuccess: refresh,
    onError: (error) => toast.error(friendlyError(error)),
  });

  const saveSettings = useMutation({
    mutationFn: (settings: TorrentSettings) => torrentsApi.saveSettings(settings),
    onSuccess: (saved) => {
      client.setQueryData(torrentKeys.settings, saved);
      toast.success("Torrent settings saved.");
    },
    onError: (error) => toast.error(friendlyError(error)),
  });

  const restartEngine = useMutation({
    mutationFn: () => torrentsApi.restartEngine(),
    onSuccess: async () => {
      await refresh();
      toast.success("Torrent engine restarted.");
    },
    onError: (error) => toast.error(friendlyError(error)),
  });

  return { action, setFiles, addMagnet, addFile, saveSettings, restartEngine };
}
