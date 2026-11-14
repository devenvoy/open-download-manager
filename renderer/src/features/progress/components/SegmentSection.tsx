import { formatBytes } from "@/lib/format";
import { type DownloadSummary } from "@/types/ipc";
import {
  segmentCaption,
  segmentLabel,
  segmentNote,
  segmentStatus,
  segmentsOf,
  statusOf,
} from "../model/progress-view";

/** The connection/segment map below the transfer buttons. */
export function SegmentSection({ download }: { download: DownloadSummary }) {
  const segments = segmentsOf(download);
  const status = statusOf(download);
  if (segments.length === 0) return null;

  return (
    <div id="connection-details">
      <p className="connection-caption">{segmentCaption(download)}</p>

      <div id="segment-map" className="segment-map" aria-hidden="true">
        {segments.map((segment, index) => {
          const total = Number(segment.total) || 0;
          const value = total
            ? Math.min(100, ((Number(segment.downloaded) || 0) / total) * 100)
            : 0;
          return (
            <progress
              key={`${segment.start}-${index}`}
              max={100}
              value={value}
            />
          );
        })}
      </div>

      <div className="connections-scroll">
        <table className="connections-table">
          <thead>
            <tr>
              <th>N.</th>
              <th>Downloaded</th>
              <th>Info</th>
            </tr>
          </thead>
          <tbody id="segment-bars">
            {segments.map((segment, index) => (
              <tr key={`${segment.start}-${index}`}>
                <td>{segmentLabel(download, index)}</td>
                <td>{formatBytes(Number(segment.downloaded) || 0)}</td>
                <td>{segmentStatus(segment, status)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <p id="segment-note" className="field-help">
        {segmentNote(download, segments.length)}
      </p>
    </div>
  );
}
