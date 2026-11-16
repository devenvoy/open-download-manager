import { formatBytes } from "@/lib/format";
import { type DownloadSummary, type DownloadSegmentProgress } from "@/types/ipc";
import {
  segmentCaption,
  segmentLabel,
  segmentNote,
  segmentStatus,
  segmentsOf,
  statusOf,
} from "../model/progress-view";

function resolveFileTotal(download: DownloadSummary, segments: DownloadSegmentProgress[]): number {
  if (download.total && download.total > 0) {
    return download.total;
  }
  let maxEnd = 0;
  for (const s of segments) {
    if (s.end > maxEnd) maxEnd = s.end;
  }
  return maxEnd > 0 ? maxEnd + 1 : 0;
}

/**
 * Continuous live segment visualizer bar and connections table
 * representing the full byte range of the file with active worker heads.
 */
export function SegmentSection({ download }: { download: DownloadSummary }) {
  const segments = segmentsOf(download);
  const status = statusOf(download);
  if (segments.length === 0) return null;

  const fileTotal = resolveFileTotal(download, segments);
  const isContinuous = fileTotal > 0;

  return (
    <div id="connection-details">
      <p className="connection-caption">{segmentCaption(download)}</p>

      {/* Continuous Live Segment Bar */}
      <div
        id="segment-map"
        className="segment-map continuous-segment-bar"
        aria-hidden="true"
        role="presentation"
      >
        {isContinuous ? (
          segments.map((segment, index) => {
            const leftPct = Math.max(0, Math.min(100, (segment.start / fileTotal) * 100));
            const spanBytes = Math.max(1, segment.end - segment.start + 1);
            const widthPct = Math.max(0.2, Math.min(100 - leftPct, (spanBytes / fileTotal) * 100));
            const downloadedRatio = segment.total > 0
              ? Math.max(0, Math.min(1, segment.downloaded / segment.total))
              : 0;
            const downloadedPct = downloadedRatio * 100;
            const isActive =
              status === "downloading" &&
              downloadedRatio > 0 &&
              downloadedRatio < 1;

            return (
              <div
                key={`${segment.start}-${index}`}
                className="segment-extent"
                style={{
                  left: `${leftPct}%`,
                  width: `${widthPct}%`,
                }}
                title={`Connection ${index + 1}: ${formatBytes(segment.start)} - ${formatBytes(segment.end)} (${Math.round(downloadedPct)}%)`}
              >
                <div
                  className="segment-fill"
                  style={{ width: `${downloadedPct}%` }}
                />
                {isActive && (
                  <div
                    className="segment-active-head"
                    style={{ left: `${downloadedPct}%` }}
                  />
                )}
              </div>
            );
          })
        ) : (
          segments.map((segment, index) => {
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
          })
        )}
      </div>

      <div className="connections-scroll">
        <table className="connections-table">
          <thead>
            <tr>
              <th>N.</th>
              <th>Range</th>
              <th>Downloaded</th>
              <th>Info</th>
            </tr>
          </thead>
          <tbody id="segment-bars">
            {segments.map((segment, index) => {
              const total = Number(segment.total) || (segment.end - segment.start + 1) || 0;
              const downloaded = Number(segment.downloaded) || 0;
              const pct = total > 0 ? Math.round((downloaded / total) * 100) : 0;
              const rangeStr = `${formatBytes(segment.start)} – ${formatBytes(segment.end)}`;

              return (
                <tr key={`${segment.start}-${index}`}>
                  <td>{segmentLabel(download, index)}</td>
                  <td className="segment-range-cell">{rangeStr}</td>
                  <td>
                    {formatBytes(downloaded)}
                    {total > 0 ? ` / ${formatBytes(total)} (${pct}%)` : ""}
                  </td>
                  <td>{segmentStatus(segment, status)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <p id="segment-note" className="field-help">
        {segmentNote(download, segments.length)}
      </p>
    </div>
  );
}
