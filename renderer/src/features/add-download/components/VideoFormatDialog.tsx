import { useEffect, useState } from "react";
import { ClassicDialog } from "@/components/feedback";
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

  if (!open) return null;

  function submit() {
    const choice = choices[Number(index)];
    if (choice) onPick(choice);
  }

  return (
    <ClassicDialog
      id="video-formats-dialog"
      className="video-formats-dialog"
      open={open}
      onClose={() => onOpenChange(false)}
      title="Choose Quality / Format"
      titleId="video-formats-title"
    >
      <form
        id="video-formats-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="dialog-body">
          <div className="playlist-header">
            <div className="playlist-meta">
              <h3 id="video-formats-name">
                {choices[0]?.title || "YouTube Video"}
              </h3>
              {choices[0]?.duration ? (
                <p id="video-formats-details" className="field-help">
                  Duration: {formatVideoDuration(choices[0].duration)}
                </p>
              ) : null}
            </div>
          </div>
          {formats.isError && (
            <p id="video-formats-error" className="error-notice">
              {formats.error instanceof Error
                ? formats.error.message
                : "No available formats found for this video."}
            </p>
          )}
          <div className="form-line">
            <label htmlFor="video-format-select">Format:</label>
            <select
              id="video-format-select"
              className="full-width"
              style={{ padding: 6, width: "100%" }}
              value={index}
              onChange={(e) => setIndex(e.target.value)}
            >
              {choices.map((choice, choiceIndex) => (
                <option key={choiceIndex} value={String(choiceIndex)}>
                  {formatChoiceLabel(choice)}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="dialog-footer">
          <span className="spacer" />
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
            id="video-formats-apply"
            disabled={choices.length === 0}
          >
            Select Format
          </button>
        </div>
      </form>
    </ClassicDialog>
  );
}
