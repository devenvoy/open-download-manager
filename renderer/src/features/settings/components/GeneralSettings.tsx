import { type AppTheme } from "@/types/ipc";
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
    <>
      <fieldset>
        <legend>Appearance</legend>
        <div className="form-line">
          <label htmlFor="app-theme">Theme:</label>
          <select
            id="app-theme"
            value={draft.theme}
            onChange={(e) => onChange("theme", e.target.value as AppTheme)}
          >
            <option value="system">System (Match OS)</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
        <p className="field-help">
          System dynamically tracks your operating system light/dark mode. Light
          and Dark use a fixed high-contrast palette.
        </p>
      </fieldset>

      <fieldset>
        <legend>General</legend>
        <label className="checkbox-label" htmlFor="start-with-windows">
          <input
            type="checkbox"
            id="start-with-windows"
            checked={draft.startWithWindows}
            onChange={(e) => onChange("startWithWindows", e.target.checked)}
          />
          Start Open Download Manager with Windows and keep it in the system tray
        </label>
        <p className="field-help">
          Automatic startup stays in the tray until you open the app or start a
          download.
        </p>
      </fieldset>

      <fieldset>
        <legend>Clipboard Monitoring</legend>
        <label className="checkbox-label" htmlFor="clipboard-monitoring-enabled">
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
        <p className="field-help">
          When enabled, Open Download Manager detects copied download links and
          prompts you to add them. Clipboard contents are never logged, stored,
          or sent across the network.
        </p>
        <div id="clipboard-suboptions" style={{ marginLeft: 20, marginTop: 6 }}>
          <label className="checkbox-label" htmlFor="clipboard-auto-enqueue">
            <input
              type="checkbox"
              id="clipboard-auto-enqueue"
              checked={draft.clipboardAutoEnqueue}
              onChange={(e) =>
                onChange("clipboardAutoEnqueue", e.target.checked)
              }
            />
            Automatically add detected links without confirmation prompt
          </label>
        </div>
      </fieldset>
    </>
  );
}
