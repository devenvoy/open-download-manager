import { useEffect } from "react";
import { formatBytes } from "@/lib/format";
import { type TorrentListItem } from "@/types/ipc";
import { FileTypeIcon } from "@/lib/file-icon";
import {
  progressPercent,
  rowSubtitle,
  stateKey,
  stateShortLabel,
} from "../model/torrent-view";

export interface TorrentListProps {
  torrents: TorrentListItem[];
  selectedId: number | null;
  onSelect: (id: number) => void;
}

export function TorrentList({ torrents, selectedId, onSelect }: TorrentListProps) {
  useEffect(() => {
    if (torrents.length > 0) {
      try {
        const clickMarks = performance.getEntriesByName("torrent:add-click");
        if (clickMarks.length > 0) {
          performance.mark("torrent:row-rendered");
          const measure = performance.measure(
            "torrent:click-to-row",
            "torrent:add-click",
            "torrent:row-rendered",
          );
          console.log(`[CLICK_TO_ROW] ${measure.duration.toFixed(2)} ms`);
          performance.clearMarks("torrent:add-click");
          performance.clearMarks("torrent:row-rendered");
        }
      } catch {
        // Ignored if marks unavailable
      }
    }
  }, [torrents.length]);

  return (
    <>
      {torrents.map((torrent) => {
        const selected = torrent.id === selectedId;
        const percent = progressPercent(torrent);
        const sub = rowSubtitle(torrent);
        const isCompleted = percent >= 100 || stateKey(torrent) === "seeding";
        return (
          <div
            key={`${torrent.infoHash}-${torrent.id}`}
            className={`torrent-row ${selected ? "selected" : ""}`}
            role="option"
            aria-selected={selected}
            data-id={torrent.id}
            data-state={stateKey(torrent)}
            tabIndex={0}
            onClick={() => onSelect(torrent.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect(torrent.id);
              }
            }}
          >
            <div className="torrent-row-main">
              <div className="torrent-row-title">
                <FileTypeIcon
                  filename={torrent.name || torrent.infoHash}
                  className="torrent-row-icon shrink-0"
                />
                <span className="torrent-name" title={torrent.name || torrent.infoHash}>
                  {torrent.name || torrent.infoHash}
                </span>
              </div>
              <span className="torrent-row-sub">{sub}</span>
              {!isCompleted && (
                <progress
                  className="torrent-row-progress"
                  max={100}
                  value={percent}
                />
              )}
            </div>
            <span
              className="torrent-size"
              title={
                torrent.uploadedBytes
                  ? `Downloaded: ${formatBytes(torrent.progressBytes || torrent.totalBytes)} · Uploaded: ${formatBytes(torrent.uploadedBytes)}`
                  : undefined
              }
            >
              {formatBytes(torrent.totalBytes)}
            </span>
            <span className="torrent-row-state">
              {stateShortLabel(torrent)}
            </span>
          </div>
        );
      })}
    </>
  );
}
