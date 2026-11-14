import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { type DownloadCategory } from "@/types/ipc";
import { settingsApi } from "../api";
import { type SettingsDraft } from "../model/settings-draft";

const CATEGORIES: ReadonlyArray<{ value: DownloadCategory; label: string }> = [
  { value: "Archives", label: "Compressed / Archives" },
  { value: "Documents", label: "Documents" },
  { value: "Audio", label: "Music / Audio" },
  { value: "Programs", label: "Programs" },
  { value: "Video", label: "Video" },
  { value: "Other", label: "Other" },
];

export interface StorageSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
  onError: (msg: string) => void;
}

export function StorageSettings({
  draft,
  onChange,
  onError,
}: StorageSettingsProps) {
  const currentCategoryDir =
    draft.categoryDirectories[draft.selectedCategory] || "";

  async function handleBrowseDefault() {
    try {
      const selected = await settingsApi.chooseDirectory();
      if (selected) onChange("defaultDirectory", selected);
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to choose folder");
    }
  }

  async function handleBrowseCategory() {
    try {
      const selected = await settingsApi.chooseDirectory();
      if (selected) {
        onChange("categoryDirectories", {
          ...draft.categoryDirectories,
          [draft.selectedCategory]: selected,
        });
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to choose folder");
    }
  }

  function handleResetCategory() {
    const next = { ...draft.categoryDirectories };
    delete next[draft.selectedCategory];
    onChange("categoryDirectories", next);
  }

  function handleCategoryDirChange(val: string) {
    const trimmed = val.trim();
    const next = { ...draft.categoryDirectories };
    if (trimmed) {
      next[draft.selectedCategory] = trimmed;
    } else {
      delete next[draft.selectedCategory];
    }
    onChange("categoryDirectories", next);
  }

  async function handleBrowseTemporary() {
    try {
      const selected = await settingsApi.chooseDirectory();
      if (selected) onChange("temporaryDirectory", selected);
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to choose folder");
    }
  }

  function handleResetTemporary() {
    onChange("temporaryDirectory", "");
  }

  return (
    <fieldset className="rounded-md border p-4 space-y-4">
      <legend className="px-2 text-sm font-semibold">Save to</legend>

      <div className="space-y-1.5">
        <Label htmlFor="default-directory">Default download folder:</Label>
        <div className="flex gap-2">
          <Input
            id="default-directory"
            value={draft.defaultDirectory}
            onChange={(e) => onChange("defaultDirectory", e.target.value)}
            required
          />
          <Button
            type="button"
            variant="outline"
            id="choose-directory"
            onClick={() => void handleBrowseDefault()}
          >
            Browse…
          </Button>
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="options-category-select">
            Category destination folder:
          </Label>
          <Select
            value={draft.selectedCategory}
            onValueChange={(val) =>
              onChange("selectedCategory", val as DownloadCategory)
            }
          >
            <SelectTrigger id="options-category-select" className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {CATEGORIES.map((c) => (
                <SelectItem key={c.value} value={c.value}>
                  {c.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex gap-2">
          <Input
            id="category-directory"
            value={currentCategoryDir}
            placeholder="Use default download folder"
            onChange={(e) => handleCategoryDirChange(e.target.value)}
          />
          <Button
            type="button"
            variant="outline"
            id="choose-category-directory"
            onClick={() => void handleBrowseCategory()}
          >
            Browse…
          </Button>
          <Button
            type="button"
            variant="outline"
            id="reset-category-directory"
            title="Reset this category to use the default download folder"
            onClick={handleResetCategory}
          >
            Reset
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Files in this category are automatically saved to this folder.
        </p>
      </div>

      <div className="space-y-1.5">
        <Label htmlFor="temporary-directory">Temporary parts directory:</Label>
        <div className="flex gap-2">
          <Input
            id="temporary-directory"
            value={draft.temporaryDirectory}
            placeholder="Default (same volume / folder as download)"
            onChange={(e) => onChange("temporaryDirectory", e.target.value)}
          />
          <Button
            type="button"
            variant="outline"
            id="choose-temporary-directory"
            onClick={() => void handleBrowseTemporary()}
          >
            Browse…
          </Button>
          <Button
            type="button"
            variant="outline"
            id="reset-temporary-directory"
            title="Reset to default download location"
            onClick={handleResetTemporary}
          >
            Reset
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Temporary parts are downloaded here before final assembly into the
          destination folder.
        </p>
      </div>
    </fieldset>
  );
}
