import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { type TorrentListItem } from "@/types/ipc";
import { availableActions } from "../model/torrent-view";

export interface TorrentRemoveDialogProps {
  torrent: TorrentListItem;
  disabled: boolean;
  onAction: (action: "forget" | "delete") => void;
}

export function TorrentRemoveDialog({
  torrent,
  disabled,
  onAction,
}: TorrentRemoveDialogProps) {
  const actions = availableActions(torrent);

  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="destructive" disabled={disabled || !actions.delete}>
          Remove
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Remove “{torrent.name}”?</AlertDialogTitle>
          <AlertDialogDescription>
            Remove from the list keeps the downloaded files on disk. Delete also
            removes the payload from the download folder.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <Button variant="outline" onClick={() => onAction("forget")}>
            Remove from list
          </Button>
          <AlertDialogAction onClick={() => onAction("delete")}>
            Delete files
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
