import { type TorrentEngineStatus } from "@/types/ipc";

export interface TorrentEngineBannerProps {
  status: TorrentEngineStatus;
  onEnable: () => void;
  onRestart: () => void;
  restarting: boolean;
}

export function TorrentEngineBanner({
  status,
  onEnable,
  onRestart,
  restarting,
}: TorrentEngineBannerProps) {
  if (status.state === "ready" && !status.message) return null;

  const isError =
    Boolean(status.message) ||
    status.state === "error" ||
    status.state === "failed";
  const kind = isError ? "error" : "warning";
  const message =
    status.message ||
    (status.state === "disabled"
      ? "BitTorrent support is disabled in settings."
      : `BitTorrent engine state: ${status.state}`);

  return (
    <div
      id="torrent-banner"
      className="torrent-banner"
      data-kind={kind}
      role="status"
    >
      <span id="torrent-banner-text">{message}</span>
      {status.state === "disabled" ? (
        <button
          id="torrent-banner-action"
          type="button"
          className="small-button"
          onClick={onEnable}
        >
          Enable
        </button>
      ) : (
        <button
          id="torrent-banner-action"
          type="button"
          className="small-button"
          disabled={restarting}
          onClick={onRestart}
        >
          {restarting ? "Restarting…" : "Restart engine"}
        </button>
      )}
    </div>
  );
}
