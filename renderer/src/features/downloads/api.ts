import { ipc } from "@/lib/ipc";
import {
  type DownloadActionKind,
  type DownloadRequest,
} from "@/types/ipc";

/**
 * The downloads feature's IPC surface (plan §9). Hooks and components call
 * this, never `@/lib/ipc` directly, so the feature has one seam to test.
 */
export const downloadsApi = {
  add: (request: DownloadRequest) => ipc.addDownload(request),
  action: (id: string, action: DownloadActionKind) =>
    ipc.downloadAction(id, action),
  chooseDirectory: () => ipc.chooseDirectory(),
  readClipboard: () => ipc.readClipboard(),
};
