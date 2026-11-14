import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { type DownloadQueue, type MissedRunPolicy } from "@/types/ipc";
import {
  DAY_LABELS,
  canRenameQueue,
  defaultSchedule,
  draftFromQueue,
  validateQueueDraft,
  type QueueDraft,
} from "../model/queue-view";

export interface QueueEditDialogProps {
  open: boolean;
  queue: DownloadQueue | null;
  saving: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (draft: QueueDraft) => void;
}

export function QueueEditDialog({
  open,
  queue,
  saving,
  onOpenChange,
  onSave,
}: QueueEditDialogProps) {
  const [draft, setDraft] = useState<QueueDraft>(() => draftFromQueue(queue));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setDraft(draftFromQueue(queue));
      setError(null);
    }
  }, [open, queue]);

  const schedule = draft.schedule;

  function patch(next: Partial<QueueDraft>) {
    setDraft((current) => ({ ...current, ...next }));
  }

  function toggleSchedule(enabled: boolean) {
    patch({ schedule: enabled ? (schedule ?? defaultSchedule()) : null });
  }

  function toggleDay(day: number, checked: boolean) {
    if (!schedule) return;
    const days = checked
      ? [...new Set([...schedule.daysOfWeek, day])].sort((a, b) => a - b)
      : schedule.daysOfWeek.filter((value) => value !== day);
    patch({ schedule: { ...schedule, daysOfWeek: days } });
  }

  function submit() {
    const problem = validateQueueDraft(draft);
    if (problem) {
      setError(problem);
      return;
    }
    onSave({ ...draft, name: draft.name.trim() });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {queue ? `Edit Queue: ${queue.name}` : "Create Download Queue"}
          </DialogTitle>
          <DialogDescription>
            A queue runs its downloads with its own concurrency and speed limit,
            and can start and stop on a recurring schedule.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="queue-name">Queue name</Label>
            <Input
              id="queue-name"
              value={draft.name}
              disabled={!canRenameQueue(queue)}
              maxLength={100}
              onChange={(event) => patch({ name: event.target.value })}
              placeholder="e.g. Nightly, Work"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <Label htmlFor="queue-concurrency">Max active downloads</Label>
              <Input
                id="queue-concurrency"
                type="number"
                min={1}
                max={10}
                value={draft.concurrencyLimit}
                onChange={(event) =>
                  patch({ concurrencyLimit: Number(event.target.value) })
                }
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="queue-speed">Speed limit (KB/s, 0 = off)</Label>
              <Input
                id="queue-speed"
                type="number"
                min={0}
                value={draft.speedLimitKbps}
                onChange={(event) =>
                  patch({ speedLimitKbps: Number(event.target.value) })
                }
              />
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Switch
              id="queue-schedule"
              checked={Boolean(schedule?.enabled)}
              onCheckedChange={toggleSchedule}
            />
            <Label htmlFor="queue-schedule">Enable recurring schedule</Label>
          </div>

          {schedule?.enabled ? (
            <div className="space-y-3 rounded-md border border-border p-3">
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <Label htmlFor="queue-start">Start time</Label>
                  <Input
                    id="queue-start"
                    type="time"
                    value={schedule.startTime}
                    onChange={(event) =>
                      patch({
                        schedule: { ...schedule, startTime: event.target.value },
                      })
                    }
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="queue-stop">Stop time (optional)</Label>
                  <Input
                    id="queue-stop"
                    type="time"
                    value={schedule.stopTime ?? ""}
                    onChange={(event) =>
                      patch({
                        schedule: {
                          ...schedule,
                          stopTime: event.target.value || null,
                        },
                      })
                    }
                  />
                </div>
              </div>

              <fieldset className="space-y-2">
                <legend className="text-sm font-medium">Days of week</legend>
                <div className="flex flex-wrap gap-3">
                  {DAY_LABELS.map((day) => (
                    <label
                      key={day.value}
                      className="flex items-center gap-2 text-sm"
                    >
                      <Checkbox
                        checked={schedule.daysOfWeek.includes(day.value)}
                        onCheckedChange={(checked) =>
                          toggleDay(day.value, checked === true)
                        }
                      />
                      {day.label}
                    </label>
                  ))}
                </div>
              </fieldset>

              <div className="space-y-2">
                <Label htmlFor="queue-missed">Missed run policy</Label>
                <Select
                  value={schedule.missedRunPolicy}
                  onValueChange={(value) =>
                    patch({
                      schedule: {
                        ...schedule,
                        missedRunPolicy: value as MissedRunPolicy,
                      },
                    })
                  }
                >
                  <SelectTrigger id="queue-missed">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="runOnStartup">
                      Run on startup (catch up if the app was closed)
                    </SelectItem>
                    <SelectItem value="skip">Skip until the next run</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <p className="text-xs text-muted-foreground">
                Times use the local system timezone. Start and stop apply only
                to this queue.
              </p>
            </div>
          ) : null}

          {error ? (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={saving}>
            Save Queue
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
