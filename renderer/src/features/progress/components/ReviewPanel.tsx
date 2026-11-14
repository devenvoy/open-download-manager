import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { DOWNLOAD_CATEGORIES, type DownloadSummary } from "@/types/ipc";
import {
  draftFromDownload,
  validateReviewDraft,
  type ReviewDraft,
} from "../model/review-form";

export interface ReviewPanelProps {
  download: DownloadSummary;
  submitting: boolean;
  onBrowse: () => Promise<string | null>;
  onSubmit: (draft: ReviewDraft, queueOnly: boolean) => void;
  onCancel: () => void;
}

/**
 * The confirmation form the progress window shows while
 * `download.needsReview` is true.
 */
export function ReviewPanel({
  download,
  submitting,
  onBrowse,
  onSubmit,
  onCancel,
}: ReviewPanelProps) {
  const [draft, setDraft] = useState<ReviewDraft>(() => draftFromDownload(download));
  const [error, setError] = useState<string | null>(null);

  function patch(next: Partial<ReviewDraft>) {
    setDraft((current) => ({ ...current, ...next }));
  }

  async function browse() {
    try {
      const picked = await onBrowse();
      if (picked) patch({ directory: picked });
    } catch {
      setError("Could not open the folder picker.");
    }
  }

  function submit(event: FormEvent, queueOnly: boolean) {
    event.preventDefault();
    const problem = validateReviewDraft(draft);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    onSubmit(draft, queueOnly);
  }

  return (
    <form className="space-y-4" onSubmit={(event) => submit(event, false)}>
      <div className="space-y-2">
        <Label htmlFor="review-url">URL</Label>
        <Input id="review-url" readOnly value={download.url} />
      </div>

      <div className="space-y-2">
        <Label htmlFor="review-category">Category</Label>
        <Select
          value={draft.category || "auto"}
          onValueChange={(value) =>
            patch({
              category:
                value === "auto" ? "" : (value as ReviewDraft["category"]),
            })
          }
        >
          <SelectTrigger id="review-category">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="auto">Automatic</SelectItem>
            {DOWNLOAD_CATEGORIES.map((category) => (
              <SelectItem key={category} value={category}>
                {category}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="space-y-2">
        <Label htmlFor="review-filename">Save as</Label>
        <Input
          id="review-filename"
          maxLength={240}
          value={draft.filename}
          onChange={(event) => patch({ filename: event.target.value })}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="review-directory">Save to</Label>
        <div className="flex gap-2">
          <Input
            id="review-directory"
            value={draft.directory}
            onChange={(event) => patch({ directory: event.target.value })}
          />
          <Button type="button" variant="outline" onClick={() => void browse()}>
            Browse…
          </Button>
        </div>
      </div>

      <div className="space-y-2">
        <Label htmlFor="review-description">Description</Label>
        <Input
          id="review-description"
          maxLength={1_000}
          value={draft.description}
          onChange={(event) => patch({ description: event.target.value })}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="review-schedule">Start at</Label>
        <div className="flex items-center gap-2">
          <Input
            id="review-schedule"
            type="datetime-local"
            value={draft.scheduledAt}
            onChange={(event) => patch({ scheduledAt: event.target.value })}
          />
          <span className="text-xs text-muted-foreground">Optional</span>
        </div>
      </div>

      <Separator />

      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={draft.authEnabled}
            onCheckedChange={(checked) =>
              patch({ authEnabled: checked === true })
            }
          />
          Use basic authentication for this download
        </label>
        {draft.authEnabled ? (
          <div className="flex gap-2">
            <Input
              value={draft.authUser}
              onChange={(event) => patch({ authUser: event.target.value })}
              placeholder="Username"
              autoComplete="off"
              aria-label="Username"
            />
            <Input
              type="password"
              value={draft.authPass}
              onChange={(event) => patch({ authPass: event.target.value })}
              placeholder="Password"
              autoComplete="off"
              aria-label="Password"
            />
          </div>
        ) : null}
      </div>

      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-2 pt-1">
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel download
        </Button>
        <span className="flex-1" />
        <Button
          type="button"
          variant="outline"
          disabled={submitting}
          onClick={(event) => submit(event, true)}
        >
          Download Later
        </Button>
        <Button type="submit" disabled={submitting}>
          Start Download
        </Button>
      </div>
    </form>
  );
}
