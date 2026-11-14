import { Button } from "@/components/ui/button";
import { type MediaToolType } from "@/types/ipc";
import {
  useInstallMediaTool,
  useMediaToolsStatus,
} from "../hooks/use-settings";

export function MediaToolsSettings() {
  const { data: statuses, isPending } = useMediaToolsStatus();
  const installMutation = useInstallMediaTool();

  const ffmpegStatus = statuses?.find((s) => s.toolType === "ffmpeg");
  const ytdlpStatus = statuses?.find((s) => s.toolType === "yt-dlp");

  async function handleInstall(tool: MediaToolType) {
    try {
      await installMutation.mutateAsync(tool);
    } catch {
      // Handled by react-query / notification
    }
  }

  function renderToolRow(
    title: string,
    tool: MediaToolType,
    statusId: string,
    btnId: string,
    item?: { isAvailable: boolean; source: string | null },
  ) {
    const isInstalling =
      installMutation.isPending && installMutation.variables === tool;
    const isAvail = item?.isAvailable === true;

    return (
      <div className="flex items-center justify-between py-2 border-b last:border-0">
        <div className="space-y-0.5">
          <strong className="text-sm font-medium">{title}</strong>
          <div>
            <span
              id={statusId}
              className={`text-xs ${
                isInstalling
                  ? "text-sky-600 dark:text-sky-400"
                  : isPending
                    ? "text-muted-foreground"
                    : isAvail
                      ? "text-green-600 dark:text-green-400 font-medium"
                      : "text-destructive font-medium"
              }`}
            >
              {isInstalling
                ? "Downloading & installing…"
                : isPending
                  ? "Checking…"
                  : isAvail
                    ? `Installed (${item?.source || "ready"})`
                    : "Not installed"}
            </span>
          </div>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          id={btnId}
          disabled={isInstalling}
          onClick={() => void handleInstall(tool)}
        >
          {isInstalling ? "Installing…" : isAvail ? "Reinstall" : "Install"}
        </Button>
      </div>
    );
  }

  return (
    <fieldset className="rounded-md border p-4 space-y-3">
      <legend className="px-2 text-sm font-semibold">Media Tools</legend>
      <div className="divide-y">
        {renderToolRow(
          "FFmpeg",
          "ffmpeg",
          "ffmpeg-status",
          "install-ffmpeg-btn",
          ffmpegStatus,
        )}
        {renderToolRow(
          "yt-dlp",
          "yt-dlp",
          "ytdlp-status",
          "install-ytdlp-btn",
          ytdlpStatus,
        )}
      </div>
      <p className="text-xs text-muted-foreground pt-1">
        Media tools are managed automatically by Open Download Manager to
        combine video and audio streams.
      </p>
    </fieldset>
  );
}
