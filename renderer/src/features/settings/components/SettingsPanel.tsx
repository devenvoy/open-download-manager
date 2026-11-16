import { useState } from "react";
import { toast } from "sonner";
import { type Settings, type SettingsPatch } from "@/types/ipc";
import {
  buildSettingsPatch,
  draftFromSettings,
  type SettingsDraft,
  validateSettingsDraft,
} from "../model/settings-draft";
import { AppearanceSettings } from "./AppearanceSettings";
import { CaptureSettings } from "./CaptureSettings";
import { CompletionActionsSettings } from "./CompletionActionsSettings";
import { ConnectionSettings } from "./ConnectionSettings";
import { CredentialsSettings } from "./CredentialsSettings";
import { GeneralSettings } from "./GeneralSettings";
import { MediaToolsSettings } from "./MediaToolsSettings";
import { ProxySettingsSection } from "./ProxySettingsSection";
import { SettingsSidebar, type SettingsTabId } from "./SettingsSidebar";
import { StorageSettings } from "./StorageSettings";

export interface SettingsPanelProps {
  settings: Settings;
  saving: boolean;
  onSave: (patch: SettingsPatch) => Promise<void>;
  onClose?: () => void;
  defaultTab?: SettingsTabId;
}

export function SettingsPanel({
  settings,
  saving,
  onSave,
  onClose,
  defaultTab = "general",
}: SettingsPanelProps) {
  const [activeTab, setActiveTab] = useState<SettingsTabId>(defaultTab);
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
    <form
      id="settings-form"
      className="h-full flex flex-col bg-background text-foreground select-none"
      onSubmit={(e) => void handleSubmit(e, true)}
    >
      {/* Header bar */}
      <div className="dialog-title flex items-center justify-between px-4 py-2.5 border-b border-border bg-muted/40 shrink-0">
        <strong id="settings-title" className="text-sm font-semibold tracking-tight">
          Options
        </strong>
        <button
          type="button"
          className="close-button text-muted-foreground hover:text-foreground p-1 rounded transition-colors text-lg leading-none"
          data-close
          aria-label="Close"
          onClick={onClose}
        >
          ×
        </button>
      </div>

      {/* Main 2-pane body */}
      <div className="settings-main flex-1 min-h-0 flex overflow-hidden">
        {/* Left Category Sidebar */}
        <SettingsSidebar activeTab={activeTab} onSelectTab={setActiveTab} />

        {/* Right Active Category Content */}
        <div className="settings-content-wrapper flex-1 min-w-0 overflow-y-auto p-5 bg-background">
          {error && (
            <div
              id="settings-error"
              className="form-error mb-4 p-3 rounded-md bg-rose-50 dark:bg-rose-950/30 border border-rose-200 dark:border-rose-900 text-rose-700 dark:text-rose-400 text-xs"
              role="alert"
            >
              {error}
            </div>
          )}

          <div
            key={activeTab}
            role="tabpanel"
            aria-labelledby={`tab-${activeTab}`}
            className="settings-tab-pane animate-in fade-in-50 slide-in-from-right-2 duration-150 ease-out"
          >
            {activeTab === "general" && (
              <GeneralSettings draft={draft} onChange={handleChange} />
            )}
            {activeTab === "appearance" && (
              <AppearanceSettings draft={draft} onChange={handleChange} />
            )}
            {activeTab === "storage" && (
              <StorageSettings
                draft={draft}
                onChange={handleChange}
                onError={(msg) => setError(msg)}
              />
            )}
            {activeTab === "connection" && (
              <ConnectionSettings
                draft={draft}
                onChange={handleChange}
                onError={(msg) => setError(msg)}
              />
            )}
            {activeTab === "proxy" && (
              <ProxySettingsSection
                draft={draft}
                onChange={handleChange}
                onError={(msg) => setError(msg)}
              />
            )}
            {activeTab === "capture" && (
              <CaptureSettings draft={draft} onChange={handleChange} />
            )}
            {activeTab === "credentials" && <CredentialsSettings />}
            {activeTab === "media" && <MediaToolsSettings />}
            {activeTab === "automation" && (
              <CompletionActionsSettings draft={draft} onChange={handleChange} />
            )}
          </div>
        </div>
      </div>

      {/* Footer bar */}
      <div className="dialog-footer h-[48px] px-4 border-t border-border bg-muted/40 shrink-0 flex items-center justify-end gap-2.5">
        <button
          type="submit"
          className="button primary px-4 py-1.5 text-xs font-medium rounded-md bg-blue-600 hover:bg-blue-700 text-white transition-colors disabled:opacity-50"
          id="settings-submit"
          disabled={saving}
        >
          OK
        </button>
        <button
          type="button"
          className="button px-4 py-1.5 text-xs font-medium rounded-md border border-border hover:bg-muted transition-colors"
          id="settings-cancel"
          data-close
          onClick={onClose}
        >
          Cancel
        </button>
        <button
          type="button"
          className="button px-4 py-1.5 text-xs font-medium rounded-md border border-border hover:bg-muted transition-colors disabled:opacity-50"
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
