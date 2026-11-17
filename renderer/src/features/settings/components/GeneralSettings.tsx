import { type SettingsDraft } from "../model/settings-draft";

export interface GeneralSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
}

export function GeneralSettings({ draft, onChange }: GeneralSettingsProps) {
  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-sm font-semibold text-foreground mb-1">General Application Behavior</h3>
        <p className="text-xs text-muted-foreground">
          Configure startup options, system tray persistence, and automatic clipboard link monitoring.
        </p>
      </div>

      <fieldset className="rounded-lg border border-border p-4 bg-muted/20">
        <legend className="text-xs font-medium px-1 text-foreground">System Integration</legend>
        <label className="checkbox-label flex items-center gap-2 cursor-pointer text-xs" htmlFor="start-with-windows">
          <input
            type="checkbox"
            id="start-with-windows"
            checked={draft.startWithWindows}
            onChange={(e) => onChange("startWithWindows", e.target.checked)}
          />
          Start Open Download Manager on system startup and minimize to tray
        </label>
        <p className="field-help text-[11px] text-muted-foreground mt-1.5 ml-5">
          The application will run silently in the system tray until a download starts or you open the main window.
        </p>
      </fieldset>

      <fieldset className="rounded-lg border border-border p-4 bg-muted/20">
        <legend className="text-xs font-medium px-1 text-foreground">Clipboard Link Monitoring</legend>
        <label className="checkbox-label flex items-center gap-2 cursor-pointer text-xs" htmlFor="clipboard-monitoring-enabled">
          <input
            type="checkbox"
            id="clipboard-monitoring-enabled"
            checked={draft.clipboardMonitoringEnabled}
            onChange={(e) =>
              onChange("clipboardMonitoringEnabled", e.target.checked)
            }
          />
          Monitor clipboard for downloadable links (HTTP / HTTPS)
        </label>
        <p className="field-help text-[11px] text-muted-foreground mt-1.5 ml-5">
          When enabled, Open Download Manager detects copied links matching download categories and offers to download them. Clipboard contents are never logged or sent to any remote server.
        </p>
        <div id="clipboard-suboptions" className="ml-5 mt-3 pt-2 border-t border-border/50">
          <label className="checkbox-label flex items-center gap-2 cursor-pointer text-xs" htmlFor="clipboard-auto-enqueue">
            <input
              type="checkbox"
              id="clipboard-auto-enqueue"
              checked={draft.clipboardAutoEnqueue}
              onChange={(e) =>
                onChange("clipboardAutoEnqueue", e.target.checked)
              }
            />
            Automatically enqueue detected links without a confirmation prompt
          </label>
        </div>
      </fieldset>
    </div>
  );
}
