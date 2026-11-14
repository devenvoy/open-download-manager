import { type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { formatSpeed } from "@/lib/format";
import { type TorrentListItem } from "@/types/ipc";
import {
  availableActions,
  summaryText,
  type TorrentSummary,
} from "../model/torrent-view";
import { AddTorrentDialog } from "./AddTorrentDialog";
import { TorrentRemoveDialog } from "./TorrentRemoveDialog";

export interface AddTorrentOptionsInput {
  outputFolder?: string;
  startPaused: boolean;
  review: boolean;
}

export interface TorrentToolbarProps {
  summary: TorrentSummary;
  selected: TorrentListItem | null;
  busy: boolean;
  onAction: (action: "pause" | "resume" | "recheck" | "forget" | "delete") => void;
  onAddMagnet: (magnet: string, options: AddTorrentOptionsInput) => void;
  onAddFile: (path: string, options: AddTorrentOptionsInput) => void;
  settingsButton: ReactNode;
}

export function TorrentToolbar({
  summary,
  selected,
  busy,
  onAction,
  onAddMagnet,
  onAddFile,
  settingsButton,
}: TorrentToolbarProps) {
  const actions = availableActions(selected);

  return (
    <>
      <AddTorrentDialog
        onAddMagnet={onAddMagnet}
        onAddFile={onAddFile}
        pending={busy}
      />

      <Button
        variant="outline"
        disabled={busy || !actions.pause}
        onClick={() => onAction("pause")}
      >
        Pause
      </Button>
      <Button
        variant="outline"
        disabled={busy || !actions.resume}
        onClick={() => onAction("resume")}
      >
        Resume
      </Button>
      <Button
        variant="outline"
        disabled={busy || !actions.recheck}
        onClick={() => onAction("recheck")}
      >
        Recheck
      </Button>
      {selected ? (
        <TorrentRemoveDialog
          torrent={selected}
          disabled={busy}
          onAction={onAction}
        />
      ) : (
        <Button variant="destructive" disabled>
          Remove
        </Button>
      )}

      <Separator orientation="vertical" className="mx-1" />

      {settingsButton}

      <div className="ml-auto flex items-center gap-3 text-xs tabular-nums text-muted-foreground">
        <span>{summaryText(summary)}</span>
        <span aria-hidden>·</span>
        <span>↓ {formatSpeed(summary.downloadSpeed)}</span>
        <span aria-hidden>·</span>
        <span>↑ {formatSpeed(summary.uploadSpeed)}</span>
      </div>
    </>
  );
}
