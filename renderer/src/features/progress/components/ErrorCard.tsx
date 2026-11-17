import { useState } from "react";
import { toast } from "sonner";
import { useInstallMediaTool } from "../api";
import { errorKind, errorTitle } from "../model/progress-view";

export interface ErrorCardProps {
  error: string | null;
  busy: boolean;
  onRetry: () => void;
}

/**
 * The error card from the legacy dialog: the wording decides whether the fix
 * is a media tool, an expired link, or a plain retry.
 */
export function ErrorCard({ error, busy, onRetry }: ErrorCardProps) {
  const [expanded, setExpanded] = useState(false);
  const kind = errorKind(error);
  const install = useInstallMediaTool();
  const tool = kind === "ffmpeg" ? "ffmpeg" : kind === "yt-dlp" ? "yt-dlp" : null;

  async function installAndRetry() {
    if (!tool) return;
    try {
      await install.mutateAsync(tool);
      toast.success(`${tool} installed successfully. Retrying download…`);
      onRetry();
    } catch {
      // `useInstallMediaTool` already reports the failure via toast.
    }
  }

  return (
    <div id="detail-error-card" className="detail-error-card">
      <div className="detail-error-header">
        <span className="detail-error-badge">⚠️ Error</span>
        <strong id="detail-error-title">{errorTitle(kind)}</strong>
      </div>
      <p id="detail-error-summary" className="detail-error-summary">
        {error || "Download failed or was interrupted."}
      </p>
      <div className="detail-error-actions">
        <button
          type="button"
          className="button small-button"
          id="detail-error-view-btn"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? "Hide details" : "View full details…"}
        </button>
        <button
          type="button"
          className="button small-button primary"
          id="detail-error-retry-btn"
          disabled={busy || install.isPending}
          onClick={() => {
            if (tool) {
              void installAndRetry();
            } else {
              onRetry();
            }
          }}
        >
          {install.isPending
            ? `Installing ${tool}…`
            : tool
              ? `Install ${tool} & Fix`
              : "Retry download"}
        </button>
      </div>
      {expanded && (
        <pre
          style={{
            marginTop: 8,
            maxHeight: 120,
            overflow: "auto",
            fontSize: 10,
            background: "rgba(0,0,0,0.05)",
            padding: 6,
            borderRadius: 2,
            whiteSpace: "pre-wrap",
            wordBreak: "break-all",
          }}
        >
          {error || "No additional details were recorded."}
        </pre>
      )}
    </div>
  );
}
