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
import { type CapturePolicy } from "@/types/ipc";
import { type SettingsDraft } from "../model/settings-draft";

const MODES: ReadonlyArray<{
  value: CapturePolicy["confirmationMode"];
  label: string;
}> = [
  { value: "askForUnknown", label: "Ask confirmation for unknown types" },
  { value: "automatic", label: "Automatic (allow recognized types only)" },
  { value: "alwaysAsk", label: "Always show confirmation dialog" },
];

export interface CaptureSettingsProps {
  draft: SettingsDraft;
  onChange: <K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) => void;
}

export function CaptureSettings({ draft, onChange }: CaptureSettingsProps) {
  return (
    <fieldset className="rounded-md border p-4 space-y-4">
      <legend className="px-2 text-sm font-semibold">
        File Types & Browser Capture
      </legend>

      <div className="flex items-center gap-2">
        <Checkbox
          id="capture-enabled"
          checked={draft.captureEnabled}
          onCheckedChange={(checked) =>
            onChange("captureEnabled", checked === true)
          }
        />
        <Label
          htmlFor="capture-enabled"
          className="text-sm font-normal cursor-pointer"
        >
          Automatically capture downloads from browser
        </Label>
      </div>

      <div className="space-y-3 pl-6">
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="capture-mode">Confirmation mode:</Label>
          <Select
            value={draft.captureMode}
            onValueChange={(val) =>
              onChange(
                "captureMode",
                val as CapturePolicy["confirmationMode"],
              )
            }
          >
            <SelectTrigger id="capture-mode" className="w-72">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {MODES.map((m) => (
                <SelectItem key={m.value} value={m.value}>
                  {m.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <Label htmlFor="capture-file-types">
            Automatically capture following file types:
          </Label>
          <Input
            id="capture-file-types"
            value={draft.captureFileTypesText}
            placeholder="3GP 7Z APK AVI DMG EXE ISO MKV MP4 PDF ZIP ..."
            onChange={(e) => onChange("captureFileTypesText", e.target.value)}
          />
        </div>

        <div className="space-y-1">
          <Label htmlFor="capture-excluded-hosts">
            Don't capture from the following sites (comma-separated):
          </Label>
          <Input
            id="capture-excluded-hosts"
            value={draft.captureExcludedHostsText}
            placeholder="blocked.com, *.internal.corp"
            onChange={(e) => onChange("captureExcludedHostsText", e.target.value)}
          />
        </div>

        <div className="space-y-1">
          <Label htmlFor="capture-excluded-addresses">
            Don't capture from the following addresses/prefixes:
          </Label>
          <Input
            id="capture-excluded-addresses"
            value={draft.captureExcludedAddressesText}
            placeholder="192.168.1.100, /sensitive/, https://internal..."
            onChange={(e) =>
              onChange("captureExcludedAddressesText", e.target.value)
            }
          />
        </div>
      </div>
    </fieldset>
  );
}
