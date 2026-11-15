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
import { FileTypeIcon } from "@/lib/file-icon";

export interface DownloadsTableProps {
  searchRef?: React.RefObject<HTMLInputElement | null>;
  downloads: DownloadSummary[];
  fileIcons?: Record<string, string>;
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
  fileIcons,
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
        className="table-scroll flex-1 overflow-auto bg-white dark:bg-[#1e1e1e] select-none outline-none"
        id="table-scroll"
        tabIndex={0}
        aria-label="Downloads"
        onKeyDown={handleTableKeyDown}
      >
        <table className="downloads-table w-full border-collapse text-[11px] table-fixed" aria-label="Downloads">
          <colgroup>
            <col className="name-col" />
            <col className="queue-col w-[24px]" />
            <col className="size-col w-[75px]" />
            <col className="status-col w-[110px]" />
            <col className="time-col w-[75px]" />
            <col className="rate-col w-[85px]" />
            <col className="date-col w-[120px]" />
            <col className="description-col w-[140px]" />
          </colgroup>
          <thead>
            <tr className="h-[22px] bg-[#f0f0f0] dark:bg-[#252526] border-b border-[#aaa] dark:border-[#383838] sticky top-0 z-10 text-left font-normal text-[#333] dark:text-[#ccc]">
              <th scope="col" className="px-1.5 font-normal border-r border-[#ddd] dark:border-[#333]">
                <button type="button" data-sort="filename" onClick={() => handleSort("filename")} className="w-full text-left font-normal border-0 bg-transparent p-0 cursor-pointer">
                  File Name
                </button>
              </th>
              <th scope="col" title="Queue" className="px-1 text-center font-normal border-r border-[#ddd] dark:border-[#333]">
                Q
              </th>
              <th scope="col" className="px-1.5 font-normal border-r border-[#ddd] dark:border-[#333]">
                <button type="button" data-sort="total" onClick={() => handleSort("total")} className="w-full text-left font-normal border-0 bg-transparent p-0 cursor-pointer">
                  Size
                </button>
              </th>
              <th scope="col" className="px-1.5 font-normal border-r border-[#ddd] dark:border-[#333]">
                <button type="button" data-sort="status" onClick={() => handleSort("status")} className="w-full text-left font-normal border-0 bg-transparent p-0 cursor-pointer">
                  Status
                </button>
              </th>
              <th scope="col" className="px-1.5 font-normal border-r border-[#ddd] dark:border-[#333]">Time left</th>
              <th scope="col" className="px-1.5 font-normal border-r border-[#ddd] dark:border-[#333]">Transfer rate</th>
              <th scope="col" className="px-1.5 font-normal border-r border-[#ddd] dark:border-[#333]">
                <button type="button" data-sort="createdAt" onClick={() => handleSort("createdAt")} className="w-full text-left font-normal border-0 bg-transparent p-0 cursor-pointer">
                  Last Try...
                </button>
              </th>
              <th scope="col" className="px-1.5 font-normal">Description</th>
            </tr>
          </thead>
          <tbody id="download-list">
            {filtered.map((d) => {
              const isSelected = selectedIds.includes(d.id);
              const statusLabel = getStatusLabel(d);
              const isDownloading = d.status === "downloading";
              const isQueued = ["queued", "scheduled", "paused"].includes(d.status);

              return (
                <tr
                  key={d.id}
                  className={`download-row h-[20px] border-b border-[#f0f0f0] dark:border-[#2a2a2a] cursor-default transition-colors text-[11px] ${
                    isSelected ? "selected bg-[#cce8ff] dark:bg-[#1f3d5c] text-[#111] dark:text-white" : "hover:bg-[#eef5fd] dark:hover:bg-[#2d3742] text-[#222] dark:text-[#ddd]"
                  }`}
                  data-id={d.id}
                  data-status={d.status}
                  aria-selected={isSelected}
                  onClick={(e) => {
                    if (onSelectRow) {
                      const additive = e.ctrlKey || e.metaKey;
                      onSelectRow(d.id, { additive, range: e.shiftKey, toggle: additive && !e.shiftKey });
                    } else {
                      onToggle(d.id);
                    }
                  }}
                  onDoubleClick={() => onOpenProperties?.(d.id)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    if (!isSelected) {
                      if (onSelectRow) {
                        onSelectRow(d.id, { additive: false, range: false, toggle: false });
                      } else {
                        onToggle(d.id);
                      }
                    }
                    onOpenProperties?.(d.id);
                  }}
                >
                  <td title={d.filename || d.url} className="px-1.5 overflow-hidden text-ellipsis whitespace-nowrap">
                    <span className="file-name flex items-center gap-1.5">
                      <FileTypeIcon
                        filename={d.filename}
                        iconUrl={fileIcons?.[d.filename]}
                        className="file-type-icon native-file-icon shrink-0"
                      />
                      <span className="overflow-hidden text-ellipsis whitespace-nowrap">{d.filename || "Preparing file…"}</span>
                    </span>
                  </td>
                  <td className="px-1 text-center">{isQueued ? "✓" : ""}</td>
                  <td className="px-1.5 tabular-nums whitespace-nowrap">{getSizeText(d)}</td>
                  <td className={`row-status px-1.5 whitespace-nowrap ${d.status}`} title={d.error || statusLabel}>
                    {statusLabel}
                  </td>
                  <td className="px-1.5 whitespace-nowrap tabular-nums">{isDownloading && d.duration ? formatTimeLeft(d.duration) : ""}</td>
                  <td className="px-1.5 whitespace-nowrap tabular-nums">{isDownloading ? formatSpeed(d.speed) : ""}</td>
                  <td className="px-1.5 whitespace-nowrap tabular-nums">{formatDate(d.createdAt)}</td>
                  <td title={d.error || d.description || ""} className="px-1.5 overflow-hidden text-ellipsis whitespace-nowrap text-[#666] dark:text-[#888]">
                    {d.error || d.description || ""}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {filtered.length === 0 && (
          <div id="empty-state" className="empty-state p-8 text-center text-[#777] dark:text-[#999] text-[11px] leading-[1.6]">
            <span id="empty-message" className="font-semibold text-[#333] dark:text-[#ccc]">No downloads to display.</span>
            <br />
            Click Add URL to start a download.
          </div>
        )}
      </div>

      <div className="list-search h-[28px] px-2 bg-[#f0f0f0] dark:bg-[#252526] border-t border-[#ccc] dark:border-[#383838] flex items-center gap-2 text-[11px] shrink-0 select-none">
        <label htmlFor="search" className="text-[#555] dark:text-[#999]">Find:</label>
        <input
          ref={searchRef}
          id="search"
          type="search"
          aria-label="Search downloads"
          placeholder="File name or URL"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="h-[20px] px-1.5 border border-[#ccc] dark:border-[#444] rounded-xs bg-white dark:bg-[#1e1e1e] text-[#111] dark:text-[#e0e0e0] text-[11px] max-w-[220px]"
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
