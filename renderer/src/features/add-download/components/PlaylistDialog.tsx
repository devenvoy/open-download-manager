import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
  category,
  directory,
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

  // A fresh playlist starts with every entry ticked, as in the legacy window.
  useEffect(() => {
    if (entries.length > 0) {
      setSelected(new Set(entries.map((entry) => entry.id)));
    }
  }, [entries]);

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
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>YouTube Playlist</DialogTitle>
          <DialogDescription>
            {info.data
              ? `${info.data.entryCount || entries.length} videos${
                  info.data.uploader ? ` by ${info.data.uploader}` : ""
                }`
              : "Reading the playlist…"}
          </DialogDescription>
        </DialogHeader>

        {info.isError ? (
          <p role="alert" className="text-sm text-destructive">
            {info.error instanceof Error
              ? info.error.message
              : "Could not read the playlist."}
          </p>
        ) : null}

        {entries.length > 0 ? (
          <>
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setSelected(new Set(entries.map((entry) => entry.id)))}
              >
                Select All
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setSelected(new Set())}
              >
                Deselect All
              </Button>
            </div>

            <ul className="max-h-80 divide-y divide-border overflow-auto rounded-md border border-border">
              {entries.map((entry) => (
                <li key={entry.id} className="flex items-center gap-3 px-3 py-2">
                  <Checkbox
                    checked={selected.has(entry.id)}
                    onCheckedChange={() => toggle(entry.id)}
                    aria-label={`Include ${entry.title}`}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm" title={entry.title}>
                      {entry.title}
                    </span>
                    <span className="block truncate text-xs text-muted-foreground">
                      {[entry.channel, formatPlaylistDuration(entry.duration)]
                        .filter(Boolean)
                        .join(" • ")}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          </>
        ) : info.isPending ? (
          <p className="text-sm text-muted-foreground">Reading the playlist…</p>
        ) : null}

        {category || directory ? (
          <p className="text-xs text-muted-foreground">
            {category ? `Category: ${category}. ` : ""}
            {directory ? `Folder: ${directory}.` : ""}
          </p>
        ) : null}

        {error ? (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        ) : null}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            type="button"
            disabled={adding || entries.length === 0}
            onClick={submit}
          >
            Download Selected
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
