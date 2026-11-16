import { formatBytes, formatSpeed } from "@/lib/format";
import { type TorrentPeerItem } from "@/types/ipc";

export interface TorrentPeersTabProps {
  peers: TorrentPeerItem[];
  loading: boolean;
}

export function TorrentPeersTab({ peers, loading }: TorrentPeersTabProps) {
  if (loading && peers.length === 0) {
    return (
      <div id="torrent-tab-peers" className="torrent-tab-panel" role="tabpanel">
        <p className="torrent-tab-empty">Loading connected peers…</p>
      </div>
    );
  }

  if (peers.length === 0) {
    return (
      <div id="torrent-tab-peers" className="torrent-tab-panel" role="tabpanel">
        <p className="torrent-tab-empty">No active peer connections in this swarm.</p>
      </div>
    );
  }

  return (
    <div id="torrent-tab-peers" className="torrent-tab-panel" role="tabpanel">
      <div className="torrent-peers-scroll">
        <table className="torrent-peers-table">
          <thead>
            <tr>
              <th>IP:Port</th>
              <th>Type</th>
              <th>Flags</th>
              <th>Down Speed</th>
              <th>Up Speed</th>
              <th>Downloaded</th>
              <th>Uploaded</th>
            </tr>
          </thead>
          <tbody>
            {peers.map((peer, idx) => (
              <tr key={`${peer.address}-${idx}`}>
                <td className="peer-addr-cell">{peer.address}</td>
                <td>
                  <span className={`peer-type-badge ${peer.connectionType.toLowerCase()}`}>
                    {peer.connectionType}
                  </span>
                </td>
                <td className="peer-flags-cell" title={peer.flags}>
                  {peer.flags || "—"}
                </td>
                <td>{peer.downloadSpeed ? formatSpeed(peer.downloadSpeed) : "—"}</td>
                <td>{peer.uploadSpeed ? formatSpeed(peer.uploadSpeed) : "—"}</td>
                <td>{formatBytes(peer.fetchedBytes)}</td>
                <td>{formatBytes(peer.uploadedBytes)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="field-help">
        {peers.length} active connection{peers.length !== 1 ? "s" : ""}
      </p>
    </div>
  );
}
