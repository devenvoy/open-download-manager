import { type TorrentTrackerStatus } from "@/types/ipc";

export interface TorrentTrackersTabProps {
  trackers: TorrentTrackerStatus[];
  loading: boolean;
}

export function TorrentTrackersTab({ trackers, loading }: TorrentTrackersTabProps) {
  if (loading) {
    return (
      <div id="torrent-tab-trackers" className="torrent-tab-panel" role="tabpanel">
        <p className="field-help" style={{ padding: "8px 12px" }}>
          Loading trackers…
        </p>
      </div>
    );
  }

  if (trackers.length === 0) {
    return (
      <div id="torrent-tab-trackers" className="torrent-tab-panel" role="tabpanel">
        <p className="field-help" style={{ padding: "8px 12px" }}>
          No trackers announced.
        </p>
      </div>
    );
  }

  return (
    <div id="torrent-tab-trackers" className="torrent-tab-panel" role="tabpanel">
      <ul id="torrent-tracker-list" className="torrent-trackers">
        {trackers.map((tracker, index) => (
          <li key={`${tracker.url}-${index}`} className="torrent-tracker-item">
            <span className="torrent-tracker-url">{tracker.url}</span>
            <span className="torrent-tracker-status" style={{ marginLeft: 8 }}>
              {tracker.state}
            </span>
            <span className="torrent-tracker-peers" style={{ marginLeft: 8 }}>
              {tracker.peersReturned ?? 0} peers
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
