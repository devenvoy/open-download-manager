import { useState } from "react";
import { toast } from "sonner";
import { type Settings, type SettingsPatch } from "@/types/ipc";
import {
  buildSettingsPatch,
  draftFromSettings,
  type SettingsDraft,
  validateSettingsDraft,
} from "../model/settings-draft";
import { CaptureSettings } from "./CaptureSettings";
import { CompletionActionsSettings } from "./CompletionActionsSettings";
import { ConnectionSettings } from "./ConnectionSettings";
import { CredentialsSettings } from "./CredentialsSettings";
import { GeneralSettings } from "./GeneralSettings";
import { MediaToolsSettings } from "./MediaToolsSettings";
import { ProxySettingsSection } from "./ProxySettingsSection";
import { StorageSettings } from "./StorageSettings";

export interface SettingsPanelProps {
  settings: Settings;
  saving: boolean;
  onSave: (patch: SettingsPatch) => Promise<void>;
  onClose?: () => void;
}

export function SettingsPanel({
  settings,
  saving,
  onSave,
  onClose,
}: SettingsPanelProps) {
  const [draft, setDraft] = useState<SettingsDraft>(() =>
    draftFromSettings(settings),
  );
  const [error, setError] = useState<string | null>(null);

  function handleChange<K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) {
    setDraft((prev) => ({ ...prev, [key]: value }));
    setError(null);
  }

  async function handleSubmit(e: React.FormEvent, shouldClose = true) {
    e.preventDefault();
    setError(null);

    const validationError = validateSettingsDraft(draft);
    if (validationError) {
      setError(validationError);
      return;
    }

    try {
      const patch = buildSettingsPatch(draft, settings);
      await onSave(patch);
      toast.success("Settings saved.");
      if (shouldClose && onClose) onClose();
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to save settings.";
      setError(msg);
      toast.error(msg);
    }
  }

  return (
    <form id="settings-form" onSubmit={(e) => void handleSubmit(e, true)}>
      <div className="dialog-title">
        <strong id="settings-title">Options</strong>
        <button
          type="button"
          className="close-button"
          data-close
          aria-label="Close"
          onClick={onClose}
        >
          ×
        </button>
      </div>

      <div className="dialog-body">
        <GeneralSettings draft={draft} onChange={handleChange} />
        <StorageSettings
          draft={draft}
          onChange={handleChange}
          onError={(msg) => setError(msg)}
        />
        <ConnectionSettings
          draft={draft}
          onChange={handleChange}
          onError={(msg) => setError(msg)}
        />
        <ProxySettingsSection
          draft={draft}
          onChange={handleChange}
          onError={(msg) => setError(msg)}
        />
        <CaptureSettings draft={draft} onChange={handleChange} />
        <CredentialsSettings />
        <MediaToolsSettings />
        <CompletionActionsSettings draft={draft} onChange={handleChange} />

        {error && (
          <p id="settings-error" className="form-error" role="alert">
            {error}
          </p>
        )}
      </div>

      <div className="dialog-footer">
        <button
          type="submit"
          className="button primary"
          id="settings-submit"
          disabled={saving}
        >
          OK
        </button>
        <button
          type="button"
          className="button"
          id="settings-cancel"
          data-close
          onClick={onClose}
        >
          Cancel
        </button>
        <button
          type="button"
          className="button"
          id="settings-apply"
          disabled={saving}
          onClick={(e) => void handleSubmit(e, false)}
        >
          Apply
        </button>
      </div>
    </form>
  );
}
