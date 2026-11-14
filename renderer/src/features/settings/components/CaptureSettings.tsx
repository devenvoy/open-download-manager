import { type CapturePolicy } from "@/types/ipc";
import { type SettingsDraft } from "../model/settings-draft";

export interface CaptureSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
}

export function CaptureSettings({ draft, onChange }: CaptureSettingsProps) {
  return (
    <fieldset>
      <legend>File Types &amp; Browser Capture</legend>

      <label className="checkbox-label" htmlFor="capture-enabled">
        <input
          type="checkbox"
          id="capture-enabled"
          checked={draft.captureEnabled}
          onChange={(e) => onChange("captureEnabled", e.target.checked)}
        />
        Automatically capture downloads from browser
      </label>

      {draft.captureEnabled && (
        <>
          <div className="form-line">
            <label htmlFor="capture-mode">Confirmation mode:</label>
            <select
              id="capture-mode"
              value={draft.captureMode}
              onChange={(e) =>
                onChange(
                  "captureMode",
                  e.target.value as CapturePolicy["confirmationMode"],
                )
              }
            >
              <option value="askForUnknown">
                Ask confirmation for unknown types
              </option>
              <option value="automatic">
                Automatic (allow recognized types only)
              </option>
              <option value="alwaysAsk">
                Always show confirmation dialog
              </option>
            </select>
          </div>

          <div className="form-line">
            <label htmlFor="capture-file-types">
              Automatically capture following file types:
            </label>
            <input
              type="text"
              id="capture-file-types"
              value={draft.captureFileTypesText}
              placeholder="3GP 7Z APK AVI DMG EXE ISO MKV MP4 PDF ZIP ..."
              onChange={(e) =>
                onChange("captureFileTypesText", e.target.value)
              }
            />
          </div>

          <div className="form-line">
            <label htmlFor="capture-excluded-hosts">
              Don't capture from the following sites (comma-separated):
            </label>
            <input
              type="text"
              id="capture-excluded-hosts"
              value={draft.captureExcludedHostsText}
              placeholder="blocked.com, *.internal.corp"
              onChange={(e) =>
                onChange("captureExcludedHostsText", e.target.value)
              }
            />
          </div>

          <div className="form-line">
            <label htmlFor="capture-excluded-addresses">
              Don't capture from the following addresses/prefixes:
            </label>
            <input
              type="text"
              id="capture-excluded-addresses"
              value={draft.captureExcludedAddressesText}
              placeholder="192.168.1.100, /sensitive/, https://internal..."
              onChange={(e) =>
                onChange("captureExcludedAddressesText", e.target.value)
              }
            />
          </div>
        </>
      )}
    </fieldset>
  );
}
