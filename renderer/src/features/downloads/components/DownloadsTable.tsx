import { useCallback, useMemo, useState } from "react";
import { formatDate, formatSpeed, formatTimeLeft } from "@/lib/format";
import {
  type DownloadActionKind,
  type DownloadSummary,
} from "@/types/ipc";
import {
  getSizeText,
  getStatusLabel,
  sortDownloads,
  type SortField,
} from "../model/download-table-helpers";

export interface DownloadsTableProps {
  searchRef?: React.RefObject<HTMLInputElement | null>;
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

export function DownloadsTable({
  searchRef,
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

  const handleSort = useCallback((field: SortField) => {
    setSortField((current) => {
      if (current === field) {
        setAscending((asc) => !asc);
        return current;
      }
      setAscending(true);
      return field;
    });
  }, []);

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
    return sortDownloads(result, sortField, ascending);
  }, [downloads, search, sortField, ascending]);

  const firstSelected = selectedIds[0];
  const canOpen =
    firstSelected &&
    downloads.find((d) => d.id === firstSelected)?.status === "complete";

  const handleTableKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!filtered.length) return;
      const currentId = selectedIds[0];
      const index = filtered.findIndex((d) => d.id === currentId);

      if (["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) {
        e.preventDefault();
        const nextIndex =
          e.key === "Home"
            ? 0
            : e.key === "End"
              ? filtered.length - 1
              : Math.max(
                  0,
                  Math.min(
                    filtered.length - 1,
                    index + (e.key === "ArrowDown" ? 1 : -1),
                  ),
                );
        const nextItem = filtered[nextIndex];
        if (nextItem) {
          if (onSelectRow) {
            onSelectRow(nextItem.id, {
              additive: false,
              range: e.shiftKey,
              toggle: false,
            });
          } else {
            onToggle(nextItem.id);
          }
        }
      } else if (e.key === "Enter" && currentId) {
        onOpenProperties?.(currentId);
      } else if (e.key === "Delete" && currentId) {
        onAction(currentId, "remove");
      }
    },
    [filtered, selectedIds, onSelectRow, onToggle, onOpenProperties, onAction],
  );

  return (
    <>
      <div
        className="table-scroll"
        id="table-scroll"
        tabIndex={0}
        aria-label="Downloads"
        onKeyDown={handleTableKeyDown}
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
                    if (!isSelected) {
                      if (onSelectRow) {
                        onSelectRow(d.id, {
                          additive: false,
                          range: false,
                          toggle: false,
                        });
                      } else {
                        onToggle(d.id);
                      }
                    }
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
          ref={searchRef}
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
