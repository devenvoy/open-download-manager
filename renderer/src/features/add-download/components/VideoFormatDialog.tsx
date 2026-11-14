import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { type YouTubeFormatChoice } from "@/types/ipc";
import { formatChoiceLabel, formatVideoDuration } from "../model/add-form";
import { useVideoFormats } from "../api";

export interface VideoFormatDialogProps {
  open: boolean;
  url: string;
  onOpenChange: (open: boolean) => void;
  onPick: (choice: YouTubeFormatChoice) => void;
}

export function VideoFormatDialog({
  open,
  url,
  onOpenChange,
  onPick,
}: VideoFormatDialogProps) {
  const formats = useVideoFormats(open ? url : null);
  const choices = formats.data ?? [];
  const [index, setIndex] = useState("0");

  useEffect(() => {
    if (open) setIndex("0");
  }, [open]);

  function submit() {
    const choice = choices[Number(index)];
    if (choice) onPick(choice);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Choose Quality / Format</DialogTitle>
          <DialogDescription>
            {choices[0]?.title || "YouTube video"}
            {choices[0]?.duration
              ? ` • ${formatVideoDuration(choices[0].duration)}`
              : ""}
          </DialogDescription>
        </DialogHeader>

        {formats.isError ? (
          <p role="alert" className="text-sm text-destructive">
            {formats.error instanceof Error
              ? formats.error.message
              : "No available formats found for this video."}
          </p>
        ) : null}

        {formats.isPending ? (
          <p className="text-sm text-muted-foreground">Loading formats…</p>
        ) : (
          <div className="space-y-2">
            <Label htmlFor="video-format-select">Format</Label>
            <Select value={index} onValueChange={setIndex}>
              <SelectTrigger id="video-format-select" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {choices.map((choice, choiceIndex) => (
                  <SelectItem key={`${choice.youtubeFormat}-${choiceIndex}`} value={String(choiceIndex)}>
                    {formatChoiceLabel(choice)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            type="button"
            disabled={formats.isPending || choices.length === 0}
            onClick={submit}
          >
            Select Format
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
