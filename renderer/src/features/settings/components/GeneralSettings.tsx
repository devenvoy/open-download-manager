import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { type AppTheme } from "@/types/ipc";
import { type SettingsDraft } from "../model/settings-draft";

const THEMES: ReadonlyArray<{ value: AppTheme; label: string }> = [
  { value: "system", label: "System (Match OS)" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export interface GeneralSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
}

export function GeneralSettings({ draft, onChange }: GeneralSettingsProps) {
  return (
    <div className="space-y-6">
      <fieldset className="rounded-md border p-4">
        <legend className="px-2 text-sm font-semibold">Appearance</legend>
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-4">
            <Label htmlFor="app-theme" className="min-w-24">
              Theme:
            </Label>
            <Select
              value={draft.theme}
              onValueChange={(val) => onChange("theme", val as AppTheme)}
            >
              <SelectTrigger id="app-theme" className="w-56">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {THEMES.map((item) => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <p className="text-xs text-muted-foreground">
            System dynamically tracks your operating system light/dark mode.
            Light and Dark use a fixed high-contrast palette.
          </p>
        </div>
      </fieldset>

      <fieldset className="rounded-md border p-4">
        <legend className="px-2 text-sm font-semibold">General</legend>
        <div className="space-y-2">
          <div className="flex items-start gap-2">
            <Checkbox
              id="start-with-windows"
              checked={draft.startWithWindows}
              onCheckedChange={(checked) =>
                onChange("startWithWindows", checked === true)
              }
            />
            <Label
              htmlFor="start-with-windows"
              className="text-sm font-normal cursor-pointer leading-tight"
            >
              Start Open Download Manager with Windows and keep it in the
              system tray
            </Label>
          </div>
          <p className="text-xs text-muted-foreground">
            Automatic startup stays in the tray until you open the app or start
            a download.
          </p>
        </div>
      </fieldset>

      <fieldset className="rounded-md border p-4">
        <legend className="px-2 text-sm font-semibold">
          Clipboard Monitoring
        </legend>
        <div className="space-y-3">
          <div className="flex items-start gap-2">
            <Checkbox
              id="clipboard-monitoring-enabled"
              checked={draft.clipboardMonitoringEnabled}
              onCheckedChange={(checked) =>
                onChange("clipboardMonitoringEnabled", checked === true)
              }
            />
            <Label
              htmlFor="clipboard-monitoring-enabled"
              className="text-sm font-normal cursor-pointer leading-tight"
            >
              Monitor clipboard for downloadable links (HTTP / HTTPS)
            </Label>
          </div>
          <p className="text-xs text-muted-foreground">
            When enabled, Open Download Manager detects copied download links
            and prompts you to add them. Clipboard contents are never logged,
            stored, or sent across the network.
          </p>
          <div className="ml-6 flex items-start gap-2">
            <Checkbox
              id="clipboard-auto-enqueue"
              checked={draft.clipboardAutoEnqueue}
              disabled={!draft.clipboardMonitoringEnabled}
              onCheckedChange={(checked) =>
                onChange("clipboardAutoEnqueue", checked === true)
              }
            />
            <Label
              htmlFor="clipboard-auto-enqueue"
              className={`text-sm font-normal leading-tight ${
                draft.clipboardMonitoringEnabled
                  ? "cursor-pointer"
                  : "text-muted-foreground"
              }`}
            >
              Automatically add detected links without confirmation prompt
            </Label>
          </div>
        </div>
      </fieldset>
    </div>
  );
}
