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
import { Textarea } from "@/components/ui/textarea";
import { DOWNLOAD_CATEGORIES } from "@/types/ipc";
import { type AddFormValues } from "../model/add-form";

export interface AddDownloadFormProps {
  values: AddFormValues;
  error: string | null;
  saving: boolean;
  showPlaylistBanner: boolean;
  showVideoBanner: boolean;
  videoLabel: string | null;
  onPatch: (patch: Partial<AddFormValues>) => void;
  onUrlsChange: (value: string) => void;
  onChooseDirectory: () => void;
  onPaste: () => void;
  onOpenPlaylist: () => void;
  onOpenFormats: () => void;
  onSubmit: (queueOnly: boolean) => void;
  onCancel: () => void;
  queueOptions: Array<{ id: string; name: string }>;
}

export function AddDownloadForm({
  values,
  error,
  saving,
  showPlaylistBanner,
  showVideoBanner,
  videoLabel,
  onPatch,
  onUrlsChange,
  onChooseDirectory,
  onPaste,
  onOpenPlaylist,
  onOpenFormats,
  onSubmit,
  onCancel,
  queueOptions,
}: AddDownloadFormProps) {
  return (
    <form
      className="space-y-4"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(false);
      }}
    >
      <div className="space-y-2">
        <Label htmlFor="download-urls">URL</Label>
        <Textarea
          id="download-urls"
          rows={3}
          required
          spellCheck={false}
          value={values.urlsText}
          onChange={(event) => onUrlsChange(event.target.value)}
          placeholder="https://example.com/file.zip"
        />
        <p className="text-xs text-muted-foreground">
          Enter one URL per line for a batch download.
        </p>
      </div>

      {showPlaylistBanner ? (
        <div className="flex items-center gap-2 rounded-md border border-primary/30 bg-primary/5 px-3 py-2 text-sm">
          <span className="flex-1">YouTube playlist detected!</span>
          <Button type="button" size="sm" variant="outline" onClick={onOpenPlaylist}>
            Select Videos…
          </Button>
        </div>
      ) : null}

      {showVideoBanner ? (
        <div className="flex items-center gap-2 rounded-md border border-primary/30 bg-primary/5 px-3 py-2 text-sm">
          <span className="flex-1">
            {videoLabel ?? "YouTube video detected!"}
          </span>
          <Button type="button" size="sm" variant="outline" onClick={onOpenFormats}>
            Choose Quality / Format…
          </Button>
        </div>
      ) : null}

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="download-category">Category</Label>
          <Select
            value={values.category || "auto"}
            onValueChange={(value) =>
              onPatch({
                category: value === "auto" ? "" : (value as AddFormValues["category"]),
              })
            }
          >
            <SelectTrigger id="download-category">
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
          <Label htmlFor="download-queue">Queue</Label>
          <Select
            value={values.queueId || "default"}
            onValueChange={(value) => onPatch({ queueId: value })}
          >
            <SelectTrigger id="download-queue">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {queueOptions.map((queue) => (
                <SelectItem key={queue.id} value={queue.id}>
                  {queue.name}
                  {queue.id === "default" ? " (Default)" : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="space-y-2">
        <Label htmlFor="download-filename">Save as</Label>
        <Input
          id="download-filename"
          maxLength={240}
          value={values.filename}
          onChange={(event) => onPatch({ filename: event.target.value })}
          placeholder="Original file name (optional)"
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="add-directory">Save to</Label>
        <div className="flex gap-2">
          <Input
            id="add-directory"
            value={values.directory}
            onChange={(event) => onPatch({ directory: event.target.value })}
            placeholder="Default download folder"
          />
          <Button type="button" variant="outline" onClick={onChooseDirectory}>
            Browse…
          </Button>
        </div>
      </div>

      <div className="space-y-2">
        <Label htmlFor="download-description">Description</Label>
        <Input
          id="download-description"
          maxLength={1000}
          value={values.description}
          onChange={(event) => onPatch({ description: event.target.value })}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="download-schedule">Start at</Label>
        <div className="flex items-center gap-2">
          <Input
            id="download-schedule"
            type="datetime-local"
            value={values.scheduledAt}
            onChange={(event) => onPatch({ scheduledAt: event.target.value })}
          />
          <span className="text-xs text-muted-foreground">Optional</span>
        </div>
      </div>

      <Separator />

      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm" htmlFor="add-auth-enabled">
          <Checkbox
            id="add-auth-enabled"
            checked={values.authEnabled}
            onCheckedChange={(checked) =>
              onPatch({ authEnabled: checked === true })
            }
          />
          Use basic authentication for this download
        </label>
        {values.authEnabled ? (
          <div className="flex gap-2">
            <Input
              value={values.authUser}
              onChange={(event) => onPatch({ authUser: event.target.value })}
              placeholder="Username"
              autoComplete="off"
              aria-label="Username"
            />
            <Input
              type="password"
              value={values.authPass}
              onChange={(event) => onPatch({ authPass: event.target.value })}
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

      <div className="flex flex-wrap items-center gap-2 pt-2">
        <Button type="button" variant="outline" onClick={onPaste}>
          Paste
        </Button>
        <span className="flex-1" />
        <Button
          type="button"
          variant="outline"
          disabled={saving}
          onClick={() => onSubmit(true)}
        >
          Download Later
        </Button>
        <Button type="submit" disabled={saving}>
          Start Download
        </Button>
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
