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
      // Handled by react-query
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
      <div className="media-tool-line">
        <div className="media-tool-meta">
          <strong>{title}</strong>
          <span id={statusId} className="field-help">
            {isInstalling
              ? "Downloading & installing…"
              : isPending
                ? "Checking…"
                : isAvail
                  ? `Installed (${item?.source || "ready"})`
                  : "Not installed"}
          </span>
        </div>
        <button
          className="small-button"
          id={btnId}
          type="button"
          disabled={isInstalling}
          onClick={() => void handleInstall(tool)}
        >
          {isInstalling ? "Installing…" : isAvail ? "Reinstall" : "Install"}
        </button>
      </div>
    );
  }

  return (
    <fieldset>
      <legend>Media Tools</legend>
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
      <p className="field-help">
        Media tools are managed automatically by Open Download Manager to combine
        video and audio streams.
      </p>
    </fieldset>
  );
}
