import { Sun, Moon, Laptop } from "lucide-react";
import { type AppTheme } from "@/types/ipc";
import { type SettingsDraft } from "../model/settings-draft";

export interface AppearanceSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
}

export function AppearanceSettings({ draft, onChange }: AppearanceSettingsProps) {
  const themes: { value: AppTheme; label: string; desc: string; icon: typeof Sun }[] = [
    {
      value: "system",
      label: "System Theme",
      desc: "Automatically sync with your operating system light and dark preference.",
      icon: Laptop,
    },
    {
      value: "light",
      label: "Light Mode",
      desc: "Clean high-contrast bright palette with crisp neutral borders.",
      icon: Sun,
    },
    {
      value: "dark",
      label: "Dark Mode",
      desc: "Deep dark theme designed for low-light environments and reduced eye strain.",
      icon: Moon,
    },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-semibold text-foreground mb-1">Color Theme</h3>
        <p className="text-xs text-muted-foreground">
          Choose your interface theme. System dynamically tracks macOS and Windows appearance changes.
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        {themes.map((t) => {
          const Icon = t.icon;
          const isSelected = draft.theme === t.value;
          return (
            <button
              key={t.value}
              type="button"
              onClick={() => onChange("theme", t.value)}
              className={`flex flex-col items-start p-3.5 rounded-lg border text-left transition-all cursor-pointer ${
                isSelected
                  ? "border-blue-600 bg-blue-50/50 dark:border-blue-500 dark:bg-blue-950/20 ring-1 ring-blue-600 dark:ring-blue-500"
                  : "border-border hover:border-border/80 hover:bg-muted/40"
              }`}
            >
              <div className="flex items-center gap-2 mb-1.5">
                <Icon
                  size={16}
                  className={isSelected ? "text-blue-600 dark:text-blue-400" : "text-muted-foreground"}
                />
                <span className={`text-xs font-semibold ${isSelected ? "text-blue-600 dark:text-blue-400" : "text-foreground"}`}>
                  {t.label}
                </span>
              </div>
              <span className="text-[11px] text-muted-foreground leading-relaxed">
                {t.desc}
              </span>
            </button>
          );
        })}
      </div>

      <fieldset className="rounded-lg border border-border p-4 bg-muted/20">
        <legend className="text-xs font-medium px-1 text-foreground">Theme Details</legend>
        <div className="form-line">
          <label htmlFor="app-theme" className="text-xs">Active Theme:</label>
          <select
            id="app-theme"
            className="text-xs rounded border border-border px-2 py-1 bg-background"
            value={draft.theme}
            onChange={(e) => onChange("theme", e.target.value as AppTheme)}
          >
            <option value="system">System (Match OS)</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
        <p className="field-help text-[11px] text-muted-foreground mt-2">
          System mode automatically updates the application palette when the host OS changes between light and dark modes.
        </p>
      </fieldset>
    </div>
  );
}
