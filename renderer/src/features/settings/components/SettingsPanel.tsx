import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
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

  async function handleSubmit(e: React.FormEvent) {
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
      if (onClose) onClose();
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to save settings.";
      setError(msg);
      toast.error(msg);
    }
  }

  return (
    <form id="settings-form" onSubmit={(e) => void handleSubmit(e)} className="space-y-6">
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
        <p
          id="settings-error"
          role="alert"
          className="rounded border border-destructive/50 bg-destructive/10 p-3 text-xs text-destructive font-medium"
        >
          {error}
        </p>
      )}

      <div className="dialog-footer flex justify-end gap-3 pt-4 border-t sticky bottom-0 bg-background/95 backdrop-blur py-3">
        {onClose && (
          <Button
            type="button"
            variant="outline"
            className="button"
            data-close
            onClick={onClose}
          >
            Cancel
          </Button>
        )}
        <Button
          type="submit"
          disabled={saving}
          className="button primary min-w-20"
        >
          {saving ? "Saving…" : "OK"}
        </Button>
      </div>
    </form>
  );
}
