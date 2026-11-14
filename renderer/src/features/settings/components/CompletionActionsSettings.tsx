import {
  type PowerKindOption,
  type SettingsDraft,
} from "../model/settings-draft";

export interface CompletionActionsSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
}

export function CompletionActionsSettings({
  draft,
  onChange,
}: CompletionActionsSettingsProps) {
  return (
    <fieldset>
      <legend>Post-Download Actions</legend>

      <label className="checkbox-label" htmlFor="settings-play-sound">
        <input
          type="checkbox"
          id="settings-play-sound"
          checked={draft.playSound}
          onChange={(e) => onChange("playSound", e.target.checked)}
        />
        Play sound on download completion
      </label>
      {draft.playSound && (
        <div
          className="form-line"
          id="sound-options-group"
          style={{ marginLeft: 20, marginBottom: 8 }}
        >
          <label htmlFor="settings-sound-file">
            Custom sound file (optional):
          </label>
          <input
            type="text"
            id="settings-sound-file"
            placeholder="Default chime"
            value={draft.soundFile}
            onChange={(e) => onChange("soundFile", e.target.value)}
          />
        </div>
      )}

      <label className="checkbox-label" htmlFor="settings-scan-antivirus">
        <input
          type="checkbox"
          id="settings-scan-antivirus"
          checked={draft.scanAntivirus}
          onChange={(e) => onChange("scanAntivirus", e.target.checked)}
        />
        Scan completed downloads with antivirus
      </label>
      {draft.scanAntivirus && (
        <div
          id="antivirus-options-group"
          style={{ marginLeft: 20, marginBottom: 8 }}
        >
          <div className="form-line">
            <label htmlFor="settings-scanner-executable">
              Scanner executable:
            </label>
            <input
              type="text"
              id="settings-scanner-executable"
              placeholder="e.g. /usr/bin/clamscan or MpCmdRun.exe"
              value={draft.scannerExecutable}
              onChange={(e) =>
                onChange("scannerExecutable", e.target.value)
              }
            />
          </div>
          <div className="form-line">
            <label htmlFor="settings-scanner-args">
              Arguments (use &#123;file&#125; for path):
            </label>
            <input
              type="text"
              id="settings-scanner-args"
              placeholder="--scan {file}"
              value={draft.scannerArgsText}
              onChange={(e) => onChange("scannerArgsText", e.target.value)}
            />
          </div>
        </div>
      )}

      <label className="checkbox-label" htmlFor="settings-post-command">
        <input
          type="checkbox"
          id="settings-post-command"
          checked={draft.postCommand}
          onChange={(e) => onChange("postCommand", e.target.checked)}
        />
        Execute custom command after completion
      </label>
      {draft.postCommand && (
        <div
          id="post-command-options-group"
          style={{ marginLeft: 20, marginBottom: 8 }}
        >
          <div className="form-line">
            <label htmlFor="settings-command-executable">
              Command executable:
            </label>
            <input
              type="text"
              id="settings-command-executable"
              placeholder="e.g. /usr/local/bin/script.sh"
              value={draft.commandExecutable}
              onChange={(e) =>
                onChange("commandExecutable", e.target.value)
              }
            />
          </div>
          <div className="form-line">
            <label htmlFor="settings-command-args">
              Arguments (use &#123;file&#125; for path):
            </label>
            <input
              type="text"
              id="settings-command-args"
              placeholder="{file}"
              value={draft.commandArgsText}
              onChange={(e) => onChange("commandArgsText", e.target.value)}
            />
          </div>
        </div>
      )}

      <div className="form-line" style={{ marginTop: 8 }}>
        <label htmlFor="settings-power-action">When all downloads finish:</label>
        <select
          id="settings-power-action"
          value={draft.powerActionKind}
          onChange={(e) =>
            onChange("powerActionKind", e.target.value as PowerKindOption)
          }
        >
          <option value="none">Do nothing</option>
          <option value="Sleep">Sleep</option>
          <option value="Hibernate">Hibernate</option>
          <option value="Shutdown">Shut down computer</option>
        </select>
      </div>
      {draft.powerActionKind !== "none" && (
        <label
          className="checkbox-label"
          htmlFor="settings-power-confirm"
          style={{ marginLeft: 20 }}
        >
          <input
            type="checkbox"
            id="settings-power-confirm"
            checked={draft.powerConfirm}
            onChange={(e) =>
              onChange("powerConfirm", e.target.checked)
            }
          />
          Require explicit confirmation for power action
        </label>
      )}
    </fieldset>
  );
}
