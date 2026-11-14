import { useEffect, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { type TorrentSettings } from "@/types/ipc";
import { validateSettingsInput } from "../model/torrent-view";
import { torrentsApi } from "../api";

export interface TorrentSettingsDialogProps {
  settings: TorrentSettings;
  saving: boolean;
  onSave: (settings: TorrentSettings) => void;
}

export function TorrentSettingsDialog({
  settings,
  saving,
  onSave,
}: TorrentSettingsDialogProps) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<TorrentSettings>(settings);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setDraft(settings);
      setError(null);
    }
  }, [open, settings]);

  function patch<K extends keyof TorrentSettings>(key: K, value: TorrentSettings[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  async function browse() {
    const picked = await torrentsApi.chooseDirectory();
    if (picked) patch("downloadDirectory", picked);
  }

  function save() {
    const problem = validateSettingsInput(draft);
    if (problem) {
      setError(problem);
      return;
    }
    onSave(draft);
    setOpen(false);
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline">Torrent settings</Button>
      </DialogTrigger>
      <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Torrent settings</DialogTitle>
          <DialogDescription>
            These settings are stored separately from the download manager.
            Changing the port or the network toggles needs an engine restart.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <Row
            id="torrent-enabled"
            label="Enable torrent support"
            control={
              <Switch
                id="torrent-enabled"
                checked={draft.enabled}
                onCheckedChange={(checked) => patch("enabled", checked)}
              />
            }
          />

          <div className="space-y-2">
            <Label htmlFor="torrent-directory">Download folder</Label>
            <div className="flex gap-2">
              <Input
                id="torrent-directory"
                value={draft.downloadDirectory ?? ""}
                onChange={(event) =>
                  patch("downloadDirectory", event.target.value || null)
                }
                placeholder="Downloads/Torrents"
              />
              <Button type="button" variant="outline" onClick={browse}>
                Browse
              </Button>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Field
              id="torrent-port"
              label="Listen port (0 = random)"
              value={draft.listenPort}
              onChange={(value) => patch("listenPort", value)}
            />
            <Field
              id="torrent-peers"
              label="Max peers per torrent"
              value={draft.maxPeersPerTorrent}
              onChange={(value) => patch("maxPeersPerTorrent", value)}
            />
            <Field
              id="torrent-down-limit"
              label="Download limit (KB/s, 0 = off)"
              value={draft.downloadLimitKbps}
              onChange={(value) => patch("downloadLimitKbps", value)}
            />
            <Field
              id="torrent-up-limit"
              label="Upload limit (KB/s, 0 = off)"
              value={draft.uploadLimitKbps}
              onChange={(value) => patch("uploadLimitKbps", value)}
            />
            <Field
              id="torrent-ratio"
              label="Seed ratio limit (0 = off)"
              value={draft.seedRatioLimit}
              step="0.01"
              onChange={(value) => patch("seedRatioLimit", value)}
            />
            <Field
              id="torrent-seed-time"
              label="Seed time limit (minutes)"
              value={draft.seedTimeLimitMinutes}
              onChange={(value) => patch("seedTimeLimitMinutes", value)}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Toggle
              id="torrent-dht"
              label="DHT"
              checked={draft.enableDht}
              onChange={(value) => patch("enableDht", value)}
            />
            <Toggle
              id="torrent-upnp"
              label="UPnP port mapping"
              checked={draft.enableUpnp}
              onChange={(value) => patch("enableUpnp", value)}
            />
            <Toggle
              id="torrent-utp"
              label="uTP"
              checked={draft.enableUtp}
              onChange={(value) => patch("enableUtp", value)}
            />
            <Toggle
              id="torrent-start-paused"
              label="Add new torrents paused"
              checked={draft.startPaused}
              onChange={(value) => patch("startPaused", value)}
            />
            <Toggle
              id="torrent-notify"
              label="Notify when a torrent finishes"
              checked={draft.notifyOnComplete}
              onChange={(value) => patch("notifyOnComplete", value)}
            />
          </div>

          {error ? (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button onClick={save} disabled={saving}>
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Row({
  id,
  label,
  control,
}: {
  id: string;
  label: string;
  control: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <Label htmlFor={id}>{label}</Label>
      {control}
    </div>
  );
}

function Toggle({
  id,
  label,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
      <Label htmlFor={id} className="font-normal">
        {label}
      </Label>
    </div>
  );
}

function Field({
  id,
  label,
  value,
  step,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  step?: string;
  onChange: (value: number) => void;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type="number"
        min={0}
        step={step}
        value={Number.isFinite(value) ? value : 0}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </div>
  );
}
