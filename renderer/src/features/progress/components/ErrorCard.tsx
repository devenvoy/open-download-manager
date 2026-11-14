import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  const kind = errorKind(error);
  const install = useInstallMediaTool();
  const tool = kind === "ffmpeg" ? "ffmpeg" : kind === "yt-dlp" ? "yt-dlp" : null;

  async function installAndRetry() {
    if (!tool) return;
    try {
      await install.mutateAsync(tool);
      onRetry();
    } catch {
      // `useInstallMediaTool` already reports the failure.
    }
  }

  return (
    <section className="space-y-2 rounded-md border border-destructive/40 bg-destructive/5 p-3">
      <div className="flex items-center gap-2">
        <Badge variant="outline" className="border-destructive/40 text-destructive">
          ⚠ Error
        </Badge>
        <strong className="text-sm">{errorTitle(kind)}</strong>
      </div>
      <p className="text-sm text-muted-foreground">
        {error || "Download failed or was interrupted."}
      </p>
      <details className="text-sm">
        <summary className="cursor-pointer text-muted-foreground">
          View full details…
        </summary>
        <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded bg-muted p-2 text-xs">
          {error || "No additional details were recorded."}
        </pre>
      </details>
      <div className="flex flex-wrap gap-2">
        {tool ? (
          <Button
            type="button"
            size="sm"
            disabled={busy || install.isPending}
            onClick={() => void installAndRetry()}
          >
            {install.isPending
              ? `Installing ${tool}…`
              : `Install ${tool} & Fix`}
          </Button>
        ) : (
          <Button type="button" size="sm" disabled={busy} onClick={onRetry}>
            Retry download
          </Button>
        )}
      </div>
    </section>
  );
}
