import { type DownloadCategory } from "@/types/ipc";
import { settingsApi } from "../api";
import { type SettingsDraft } from "../model/settings-draft";

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
    <fieldset>
      <legend>Save to</legend>
      <label htmlFor="default-directory">Default download folder:</label>
      <div className="directory-field">
        <input
          id="default-directory"
          required
          value={draft.defaultDirectory}
          onChange={(e) => onChange("defaultDirectory", e.target.value)}
        />
        <button
          className="small-button"
          id="choose-directory"
          type="button"
          onClick={handleBrowseDefault}
        >
          Browse…
        </button>
      </div>

      <div className="form-line" style={{ marginTop: 10 }}>
        <label htmlFor="options-category-select">
          Category destination folder:
        </label>
        <select
          id="options-category-select"
          value={draft.selectedCategory}
          onChange={(e) =>
            onChange("selectedCategory", e.target.value as DownloadCategory)
          }
        >
          <option value="Archives">Compressed / Archives</option>
          <option value="Documents">Documents</option>
          <option value="Audio">Music / Audio</option>
          <option value="Programs">Programs</option>
          <option value="Video">Video</option>
          <option value="Other">Other</option>
        </select>
      </div>
      <div className="directory-field">
        <input
          id="category-directory"
          placeholder="Use default download folder"
          value={currentCategoryDir}
          onChange={(e) => handleCategoryDirChange(e.target.value)}
        />
        <button
          className="small-button"
          id="choose-category-directory"
          type="button"
          onClick={handleBrowseCategory}
        >
          Browse…
        </button>
        <button
          className="small-button"
          id="reset-category-directory"
          type="button"
          title="Reset this category to use the default download folder"
          onClick={handleResetCategory}
        >
          Reset
        </button>
      </div>
      <p className="field-help" id="category-directory-hint">
        Files in this category are automatically saved to this folder.
      </p>

      <label
        htmlFor="temporary-directory"
        style={{ marginTop: 10, display: "block" }}
      >
        Temporary parts directory:
      </label>
      <div className="directory-field">
        <input
          id="temporary-directory"
          placeholder="Default (same volume / folder as download)"
          value={draft.temporaryDirectory}
          onChange={(e) => onChange("temporaryDirectory", e.target.value)}
        />
        <button
          className="small-button"
          id="choose-temporary-directory"
          type="button"
          onClick={handleBrowseTemporary}
        >
          Browse…
        </button>
        <button
          className="small-button"
          id="reset-temporary-directory"
          type="button"
          title="Reset to default download location"
          onClick={handleResetTemporary}
        >
          Reset
        </button>
      </div>
      <p className="field-help">
        Temporary parts are downloaded here before final assembly into the
        destination folder.
      </p>
    </fieldset>
  );
}
