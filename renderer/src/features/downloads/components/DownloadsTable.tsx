import { useMemo, useState } from "react";
import { formatBytes, formatDate, formatSpeed, formatTimeLeft } from "@/lib/format";
import {
  type DownloadActionKind,
  type DownloadSummary,
} from "@/types/ipc";

export interface DownloadsTableProps {
  downloads: DownloadSummary[];
  selectedIds: string[];
  onToggle: (id: string) => void;
  onSelectRow?: (
    id: string,
    options: { additive: boolean; range: boolean; toggle: boolean },
  ) => void;
  onAction: (id: string, action: DownloadActionKind) => void;
  onOpenProperties?: (id: string) => void;
  onOpenFolder?: (id: string) => void;
}

type SortField = "filename" | "total" | "status" | "createdAt";

function getStatusLabel(d: DownloadSummary): string {
  const percent =
    d.total && d.total > 0
      ? (Number(d.downloaded) / Number(d.total)) * 100
      : null;

  switch (d.status) {
    case "complete":
      return "Complete";
    case "downloading":
      return percent !== null ? `${percent.toFixed(2)}%` : "Receiving data…";
    case "probing":
      return "Connecting…";
    case "assembling":
      return "Finalizing…";
    case "queued":
      return "Queued";
    case "scheduled":
      return "Scheduled";
    case "paused":
      return "Stopped";
    case "cancelled":
      return "Cancelled";
    case "error":
      return "Error";
    default:
      return d.status;
  }
}

function getSizeText(d: DownloadSummary): string {
  if (d.status === "complete") {
    return formatBytes(d.total ?? d.downloaded);
  }
  if (d.total && d.total > 0) {
    return `${formatBytes(d.downloaded)} / ${formatBytes(d.total)}`;
  }
  return formatBytes(d.downloaded);
}

export function DownloadsTable({
  downloads,
  selectedIds,
  onToggle,
  onSelectRow,
  onAction,
  onOpenProperties,
  onOpenFolder,
}: DownloadsTableProps) {
  const [sortField, setSortField] = useState<SortField>("createdAt");
  const [ascending, setAscending] = useState(false);
  const [search, setSearch] = useState("");

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setAscending(!ascending);
    } else {
      setSortField(field);
      setAscending(true);
    }
  };

  const filtered = useMemo(() => {
    let result = downloads;
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(
        (d) =>
          d.filename.toLowerCase().includes(q) ||
          d.url.toLowerCase().includes(q),
      );
    }
    return [...result].sort((a, b) => {
      let cmp = 0;
      if (sortField === "filename") {
        cmp = a.filename.localeCompare(b.filename);
      } else if (sortField === "total") {
        cmp = Number(a.total || 0) - Number(b.total || 0);
      } else if (sortField === "status") {
        cmp = a.status.localeCompare(b.status);
      } else if (sortField === "createdAt") {
        cmp =
          new Date(a.createdAt || 0).getTime() -
          new Date(b.createdAt || 0).getTime();
      }
      return ascending ? cmp : -cmp;
    });
  }, [downloads, search, sortField, ascending]);

  const firstSelected = selectedIds[0];
  const canOpen =
    firstSelected &&
    downloads.find((d) => d.id === firstSelected)?.status === "complete";

  return (
    <>
      <div
        className="table-scroll"
        id="table-scroll"
        tabIndex={0}
        aria-label="Downloads"
      >
        <table className="downloads-table" aria-label="Downloads">
          <colgroup>
            <col className="name-col" />
            <col className="queue-col" />
            <col className="size-col" />
            <col className="status-col" />
            <col className="time-col" />
            <col className="rate-col" />
            <col className="date-col" />
            <col className="description-col" />
          </colgroup>
          <thead>
            <tr>
              <th scope="col">
                <button
                  type="button"
                  data-sort="filename"
                  onClick={() => handleSort("filename")}
                >
                  File Name
                </button>
              </th>
              <th scope="col" title="Queue">
                Q
              </th>
              <th scope="col">
                <button
                  type="button"
                  data-sort="total"
                  onClick={() => handleSort("total")}
                >
                  Size
                </button>
              </th>
              <th scope="col">
                <button
                  type="button"
                  data-sort="status"
                  onClick={() => handleSort("status")}
                >
                  Status
                </button>
              </th>
              <th scope="col">Time left</th>
              <th scope="col">Transfer rate</th>
              <th scope="col">
                <button
                  type="button"
                  data-sort="createdAt"
                  onClick={() => handleSort("createdAt")}
                >
                  Last Try...
                </button>
              </th>
              <th scope="col">Description</th>
            </tr>
          </thead>
          <tbody id="download-list">
            {filtered.map((d) => {
              const isSelected = selectedIds.includes(d.id);
              const statusLabel = getStatusLabel(d);
              const isDownloading = d.status === "downloading";
              const isQueued = ["queued", "scheduled", "paused"].includes(
                d.status,
              );

              return (
                <tr
                  key={d.id}
                  className={`download-row ${isSelected ? "selected" : ""}`}
                  data-id={d.id}
                  data-status={d.status}
                  aria-selected={isSelected}
                  onClick={(e) => {
                    if (onSelectRow) {
                      const additive = e.ctrlKey || e.metaKey;
                      onSelectRow(d.id, {
                        additive,
                        range: e.shiftKey,
                        toggle: additive && !e.shiftKey,
                      });
                    } else {
                      onToggle(d.id);
                    }
                  }}
                  onDoubleClick={() => onOpenProperties?.(d.id)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    onOpenProperties?.(d.id);
                  }}
                >
                  <td title={d.filename || d.url}>
                    <span className="file-name">
                      <span className="file-type-icon file-type-fallback" />
                      <span>{d.filename || "Preparing file…"}</span>
                    </span>
                  </td>
                  <td>{isQueued ? "✓" : ""}</td>
                  <td>{getSizeText(d)}</td>
                  <td
                    className={`row-status ${d.status}`}
                    title={d.error || statusLabel}
                  >
                    {statusLabel}
                  </td>
                  <td>
                    {isDownloading && d.duration ? formatTimeLeft(d.duration) : ""}
                  </td>
                  <td>{isDownloading ? formatSpeed(d.speed) : ""}</td>
                  <td>{formatDate(d.createdAt)}</td>
                  <td title={d.error || d.description || ""}>
                    {d.error || d.description || ""}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {filtered.length === 0 && (
          <div id="empty-state" className="empty-state">
            <span id="empty-message">No downloads to display.</span>
            <br />
            Click Add URL to start a download.
          </div>
        )}
      </div>

      <div className="list-search">
        <label htmlFor="search">Find:</label>
        <input
          id="search"
          type="search"
          aria-label="Search downloads"
          placeholder="File name or URL"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button
          id="open-selected"
          className="small-button"
          disabled={!canOpen}
          onClick={() => firstSelected && onAction(firstSelected, "open")}
          type="button"
        >
          Open
        </button>
        <button
          id="reveal-selected"
          className="small-button"
          disabled={!firstSelected}
          onClick={() => firstSelected && onOpenFolder?.(firstSelected)}
          type="button"
        >
          Open folder
        </button>
        <button
          id="details-selected"
          className="small-button"
          disabled={!firstSelected}
          onClick={() => firstSelected && onOpenProperties?.(firstSelected)}
          type="button"
        >
          Properties
        </button>
      </div>
    </>
  );
}
