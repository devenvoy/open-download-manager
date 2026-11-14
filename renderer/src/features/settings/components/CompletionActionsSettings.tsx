import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  type PowerKindOption,
  type SettingsDraft,
} from "../model/settings-draft";

const POWER_OPTIONS: ReadonlyArray<{
  value: PowerKindOption;
  label: string;
}> = [
  { value: "none", label: "Do nothing" },
  { value: "Sleep", label: "Sleep" },
  { value: "Hibernate", label: "Hibernate" },
  { value: "Shutdown", label: "Shut down computer" },
];

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
    <fieldset className="rounded-md border p-4 space-y-4">
      <legend className="px-2 text-sm font-semibold">
        Post-Download Actions
      </legend>

      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Checkbox
            id="settings-play-sound"
            checked={draft.playSound}
            onCheckedChange={(checked) =>
              onChange("playSound", checked === true)
            }
          />
          <Label
            htmlFor="settings-play-sound"
            className="text-sm font-normal cursor-pointer"
          >
            Play sound on download completion
          </Label>
        </div>
        {draft.playSound && (
          <div
            id="sound-options-group"
            className="space-y-1 pl-6"
          >
            <Label htmlFor="settings-sound-file">
              Custom sound file (optional):
            </Label>
            <Input
              id="settings-sound-file"
              value={draft.soundFile}
              placeholder="Default chime"
              onChange={(e) => onChange("soundFile", e.target.value)}
            />
          </div>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Checkbox
            id="settings-scan-antivirus"
            checked={draft.scanAntivirus}
            onCheckedChange={(checked) =>
              onChange("scanAntivirus", checked === true)
            }
          />
          <Label
            htmlFor="settings-scan-antivirus"
            className="text-sm font-normal cursor-pointer"
          >
            Scan completed downloads with antivirus
          </Label>
        </div>
        {draft.scanAntivirus && (
          <div
            id="antivirus-options-group"
            className="space-y-3 pl-6"
          >
            <div className="space-y-1">
              <Label htmlFor="settings-scanner-executable">
                Scanner executable:
              </Label>
              <Input
                id="settings-scanner-executable"
                value={draft.scannerExecutable}
                placeholder="e.g. /usr/bin/clamscan or MpCmdRun.exe"
                onChange={(e) => onChange("scannerExecutable", e.target.value)}
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="settings-scanner-args">
                Arguments (use &#123;file&#125; for path):
              </Label>
              <Input
                id="settings-scanner-args"
                value={draft.scannerArgsText}
                placeholder="--scan {file}"
                onChange={(e) => onChange("scannerArgsText", e.target.value)}
              />
            </div>
          </div>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Checkbox
            id="settings-post-command"
            checked={draft.postCommand}
            onCheckedChange={(checked) =>
              onChange("postCommand", checked === true)
            }
          />
          <Label
            htmlFor="settings-post-command"
            className="text-sm font-normal cursor-pointer"
          >
            Execute custom command after completion
          </Label>
        </div>
        {draft.postCommand && (
          <div
            id="post-command-options-group"
            className="space-y-3 pl-6"
          >
            <div className="space-y-1">
              <Label htmlFor="settings-command-executable">
                Command executable:
              </Label>
              <Input
                id="settings-command-executable"
                value={draft.commandExecutable}
                placeholder="e.g. /usr/local/bin/script.sh"
                onChange={(e) => onChange("commandExecutable", e.target.value)}
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="settings-command-args">
                Arguments (use &#123;file&#125; for path):
              </Label>
              <Input
                id="settings-command-args"
                value={draft.commandArgsText}
                placeholder="{file}"
                onChange={(e) => onChange("commandArgsText", e.target.value)}
              />
            </div>
          </div>
        )}
      </div>

      <div className="space-y-2 pt-2">
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="settings-power-action">
            When all downloads finish:
          </Label>
          <Select
            value={draft.powerActionKind}
            onValueChange={(val) =>
              onChange("powerActionKind", val as PowerKindOption)
            }
          >
            <SelectTrigger id="settings-power-action" className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {POWER_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        {draft.powerActionKind !== "none" && (
          <div className="flex items-center gap-2 pl-6">
            <Checkbox
              id="settings-power-confirm"
              checked={draft.powerConfirm}
              onCheckedChange={(checked) =>
                onChange("powerConfirm", checked === true)
              }
            />
            <Label
              htmlFor="settings-power-confirm"
              className="text-xs font-normal cursor-pointer"
            >
              Require explicit confirmation for power action
            </Label>
          </div>
        )}
      </div>
    </fieldset>
  );
}
