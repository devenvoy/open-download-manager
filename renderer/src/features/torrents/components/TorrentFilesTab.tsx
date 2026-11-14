import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import { formatBytes } from "@/lib/format";
import { type TorrentFileItem } from "@/types/ipc";
import { fileProgressPercent } from "../model/torrent-view";

export interface TorrentFilesTabProps {
  files: TorrentFileItem[];
  selection: Set<number> | null;
  dirty: boolean;
  applying: boolean;
  onToggle: (index: number) => void;
  onSetAll: (selected: boolean) => void;
  onApply: () => void;
}

export function TorrentFilesTab({
  files,
  selection,
  dirty,
  applying,
  onToggle,
  onSetAll,
  onApply,
}: TorrentFilesTabProps) {
  const isIncluded = (file: TorrentFileItem) =>
    selection ? selection.has(file.index) : file.included;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" variant="outline" onClick={() => onSetAll(true)}>
          Select all
        </Button>
        <Button size="sm" variant="outline" onClick={() => onSetAll(false)}>
          Select none
        </Button>
        <Button size="sm" onClick={onApply} disabled={applying || !dirty}>
          Apply selection
        </Button>
        {dirty ? (
          <span className="text-xs text-muted-foreground">
            Changes are not saved yet.
          </span>
        ) : null}
      </div>

      <ul className="divide-y divide-border rounded-md border border-border">
        {files.map((file) => (
          <li key={file.index} className="flex items-center gap-3 px-3 py-2">
            <Checkbox
              checked={isIncluded(file)}
              onCheckedChange={() => onToggle(file.index)}
              aria-label={`Include ${file.name}`}
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm" title={file.name}>
                {file.name}
              </p>
              <div className="mt-1 flex items-center gap-2">
                <Progress
                  value={fileProgressPercent(file)}
                  className="h-1.5 w-32"
                />
                <span className="text-xs tabular-nums text-muted-foreground">
                  {fileProgressPercent(file).toFixed(0)}%
                </span>
              </div>
            </div>
            <span className="shrink-0 text-sm tabular-nums text-muted-foreground">
              {formatBytes(file.length)}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
