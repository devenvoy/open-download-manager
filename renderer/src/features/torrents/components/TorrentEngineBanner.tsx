import { Button } from "@/components/ui/button";
import { type TorrentEngineStatus } from "@/types/ipc";

export interface TorrentEngineBannerProps {
  status: TorrentEngineStatus;
  onEnable: () => void;
  onRestart: () => void;
  restarting: boolean;
}

/**
 * The engine's own state, when it is not simply "ready".
 *
 * A disabled engine and a failed engine are the two states where every button
 * in the window would fail, so the reason and the one action that helps are
 * shown above the list instead of a toast that disappears.
 */
export function TorrentEngineBanner({
  status,
  onEnable,
  onRestart,
  restarting,
}: TorrentEngineBannerProps) {
  if (status.state === "ready" && !status.restartRequired) return null;

  if (!status.available) {
    return (
      <div className="rounded-md border border-border bg-muted px-3 py-2 text-sm">
        This build was compiled without torrent support.
      </div>
    );
  }

  if (!status.enabled) {
    return (
      <div className="flex items-center gap-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm">
        <span className="flex-1">Torrent support is turned off.</span>
        <Button size="sm" onClick={onEnable}>
          Enable torrents
        </Button>
      </div>
    );
  }

  if (status.state === "failed") {
    return (
      <div
        role="alert"
        className="flex items-center gap-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm"
      >
        <span className="flex-1">
          The torrent engine failed to start.
          {status.message ? ` ${status.message}` : ""}
        </span>
        <Button size="sm" variant="outline" onClick={onRestart} disabled={restarting}>
          Restart engine
        </Button>
      </div>
    );
  }

  if (status.restartRequired) {
    return (
      <div className="flex items-center gap-3 rounded-md border border-border bg-muted px-3 py-2 text-sm">
        <span className="flex-1">
          Settings changed. Restart the engine to apply them.
        </span>
        <Button size="sm" variant="outline" onClick={onRestart} disabled={restarting}>
          Restart engine
        </Button>
      </div>
    );
  }

  return null;
}
