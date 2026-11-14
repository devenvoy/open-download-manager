import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  type TorrentEngineStatus,
  type TorrentListItem,
  type TorrentReviewRequest,
} from "@/types/ipc";

/**
 * Event names emitted by the Rust side. Each entry is the payload type the
 * backend sends, so a listener cannot be wired to the wrong shape.
 */
export interface BackendEventMap {
  "torrent://progress": TorrentListItem[];
  "torrent://engine": TorrentEngineStatus;
  "torrent://review": TorrentReviewRequest;
}

export type BackendEvent = keyof BackendEventMap;

/**
 * Subscribes to a backend event and returns the unsubscribe function.
 *
 * `listen` resolves asynchronously, so the caller must be able to cancel
 * before it resolves: the returned function cancels either way.
 */
export function onBackendEvent<K extends BackendEvent>(
  event: K,
  handler: (payload: BackendEventMap[K]) => void,
): () => void {
  let unlisten: UnlistenFn | null = null;
  let cancelled = false;

  void listen<BackendEventMap[K]>(event, (incoming) => {
    handler(incoming.payload);
  }).then((stop) => {
    if (cancelled) stop();
    else unlisten = stop;
  });

  return () => {
    cancelled = true;
    unlisten?.();
    unlisten = null;
  };
}
