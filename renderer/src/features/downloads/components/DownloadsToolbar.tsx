import { Moon, Sun } from "lucide-react";
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
import { Separator } from "@/components/ui/separator";
import { useThemeStore } from "@/stores/theme-store";
import { type DownloadActionKind } from "@/types/ipc";
import { AddDownloadDialog } from "./AddDownloadDialog";

export interface DownloadsToolbarProps {
  selectedIds: string[];
  onAction: (id: string, action: DownloadActionKind) => void;
  onRemove: (ids: string[]) => void;
  removing: boolean;
}

export function DownloadsToolbar({
  selectedIds,
  onAction,
  onRemove,
  removing,
}: DownloadsToolbarProps) {
  const effective = useThemeStore((state) => state.effective);
  const toggleTheme = useThemeStore((state) => state.toggle);
  const firstSelected = selectedIds[0];
  const hasSelection = selectedIds.length > 0;

  return (
    <>
      <AddDownloadDialog />
      <Button
        onClick={() => firstSelected && onAction(firstSelected, "resume")}
        disabled={!firstSelected}
      >
        Resume
      </Button>
      <Button
        onClick={() => firstSelected && onAction(firstSelected, "pause")}
        disabled={!firstSelected}
      >
        Pause
      </Button>
      <Button
        variant="outline"
        onClick={() =>
          selectedIds.forEach((id) => onAction(id, "cancel"))
        }
        disabled={!hasSelection}
      >
        Stop selected
      </Button>

      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button variant="destructive" disabled={!hasSelection || removing}>
            Delete selected ({selectedIds.length})
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete the selected downloads?</AlertDialogTitle>
            <AlertDialogDescription>
              The transfer records are removed. Files already written to disk
              stay on disk.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={() => onRemove(selectedIds)}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Separator orientation="vertical" className="mx-1" />

      <Button
        size="icon"
        variant="ghost"
        onClick={toggleTheme}
        aria-label={effective === "dark" ? "Switch to light theme" : "Switch to dark theme"}
      >
        {effective === "dark" ? <Sun /> : <Moon />}
      </Button>
    </>
  );
}
