import { useEffect, useMemo, useState } from "react";
import { ClassicDialog } from "@/components/feedback";
import { formatPlaylistDuration } from "../model/add-form";
import { usePlaylistInfo } from "../api";

export interface PlaylistDialogProps {
  open: boolean;
  url: string;
  category: string;
  directory: string;
  adding: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: (
    items: Array<{ id: string; url: string }>,
    metadata: { id: string; title: string },
  ) => void;
}

export function PlaylistDialog({
  open,
  url,
  adding,
  onOpenChange,
  onAdd,
}: PlaylistDialogProps) {
  const info = usePlaylistInfo(open ? url : null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const entries = useMemo(() => info.data?.entries ?? [], [info.data]);

  useEffect(() => {
    if (open) {
      setSelected(new Set());
      setError(null);
    }
  }, [open, info.data]);

  useEffect(() => {
    if (entries.length > 0) {
      setSelected(new Set(entries.map((entry) => entry.id)));
    }
  }, [entries]);

  if (!open) return null;

  function toggle(id: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function submit() {
    const items = entries
      .filter((entry) => selected.has(entry.id))
      .map((entry) => ({ id: entry.id, url: entry.url }));
    if (items.length === 0) {
      setError("Select at least one video to download.");
      return;
    }
    if (!info.data) return;
    onAdd(items, { id: info.data.id, title: info.data.title });
  }

  return (
    <ClassicDialog
      id="playlist-dialog"
      className="playlist-dialog"
      open={open}
      onClose={() => onOpenChange(false)}
      title="YouTube Playlist"
      titleId="playlist-title"
    >
      <form
        id="playlist-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="dialog-body playlist-dialog-body">
          <div className="playlist-header">
            <div className="playlist-meta">
              <h3 id="playlist-name">
                {info.data?.title || "YouTube Playlist"}
              </h3>
              {info.data?.uploader && (
                <p id="playlist-uploader" className="field-help">
                  by {info.data.uploader}
                </p>
              )}
              <p id="playlist-count" className="field-help">
                {info.data
                  ? `${info.data.entryCount || entries.length} videos`
                  : "Reading playlist…"}
              </p>
            </div>
            <div className="playlist-actions-bar">
              <button
                type="button"
                className="button"
                id="playlist-select-all"
                onClick={() =>
                  setSelected(new Set(entries.map((entry) => entry.id)))
                }
              >
                Select All
              </button>
              <button
                type="button"
                className="button"
                id="playlist-deselect-all"
                onClick={() => setSelected(new Set())}
              >
                Deselect All
              </button>
            </div>
          </div>
          <div
            className="playlist-items-list"
            id="playlist-items"
            tabIndex={0}
            style={{
              maxHeight: 260,
              overflowY: "auto",
              border: "1px solid var(--border-color, #ccc)",
              padding: 4,
            }}
          >
            {entries.map((entry) => (
              <label
                key={entry.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "4px 6px",
                  cursor: "pointer",
                }}
              >
                <input
                  type="checkbox"
                  checked={selected.has(entry.id)}
                  onChange={() => toggle(entry.id)}
                />
                <span style={{ flex: 1 }} className="truncate">
                  {entry.title || entry.url}
                </span>
                {entry.duration && (
                  <span style={{ fontSize: 10, color: "#666" }}>
                    {formatPlaylistDuration(entry.duration)}
                  </span>
                )}
              </label>
            ))}
          </div>
          {error && (
            <p className="form-error" id="playlist-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <div className="dialog-footer">
          <button
            type="button"
            className="button"
            data-close
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </button>
          <button
            type="submit"
            className="button primary"
            id="playlist-download-btn"
            disabled={adding || entries.length === 0}
          >
            Download Selected
          </button>
        </div>
      </form>
    </ClassicDialog>
  );
}
