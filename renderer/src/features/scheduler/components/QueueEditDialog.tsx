import { useEffect, useState } from "react";
import { ClassicDialog } from "@/components/feedback";
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

  if (!open) return null;

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
    <ClassicDialog
      id="queue-edit-dialog"
      className="queue-edit-dialog"
      open={open}
      onClose={() => onOpenChange(false)}
      title="Download Queue"
      titleId="queue-edit-title"
    >
      <form
        id="queue-edit-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="dialog-body">
          <div className="form-line">
            <label htmlFor="queue-edit-name">Queue Name:</label>
            <input
              id="queue-edit-name"
              required
              maxLength={100}
              placeholder="e.g. Nightly, Work"
              value={draft.name}
              disabled={!canRenameQueue(queue)}
              onChange={(e) => patch({ name: e.target.value })}
            />
          </div>
          <div className="form-line">
            <label htmlFor="queue-edit-concurrency">Max Active Downloads:</label>
            <input
              id="queue-edit-concurrency"
              type="number"
              min={1}
              max={10}
              required
              value={draft.concurrencyLimit}
              onChange={(e) =>
                patch({ concurrencyLimit: Number(e.target.value) || 1 })
              }
            />
          </div>
          <div className="form-line">
            <label htmlFor="queue-edit-speed">
              Speed Limit (KB/s, 0 = unlimited):
            </label>
            <input
              id="queue-edit-speed"
              type="number"
              min={0}
              max={1000000}
              value={draft.speedLimitKbps}
              onChange={(e) =>
                patch({ speedLimitKbps: Number(e.target.value) || 0 })
              }
            />
          </div>
          <fieldset className="schedule-fieldset">
            <legend>
              <label
                className="checkbox-label"
                htmlFor="queue-edit-schedule-enabled"
              >
                <input
                  type="checkbox"
                  id="queue-edit-schedule-enabled"
                  checked={Boolean(schedule)}
                  onChange={(e) => toggleSchedule(e.target.checked)}
                />
                Enable Recurring Schedule
              </label>
            </legend>
            {schedule && (
              <div id="queue-schedule-fields" className="queue-schedule-fields">
                <div className="form-line">
                  <label htmlFor="queue-edit-start-time">Start Time (24h):</label>
                  <input
                    id="queue-edit-start-time"
                    type="time"
                    value={schedule.startTime}
                    onChange={(e) =>
                      patch({
                        schedule: { ...schedule, startTime: e.target.value },
                      })
                    }
                  />
                </div>
                <div className="form-line">
                  <label htmlFor="queue-edit-stop-time">
                    Stop Time (optional):
                  </label>
                  <input
                    id="queue-edit-stop-time"
                    type="time"
                    value={schedule.stopTime ?? ""}
                    onChange={(e) =>
                      patch({
                        schedule: {
                          ...schedule,
                          stopTime: e.target.value || null,
                        },
                      })
                    }
                  />
                </div>
                <div className="form-line">
                  <label>Days of Week:</label>
                  <div className="days-selector">
                    {DAY_LABELS.map((day) => (
                      <label key={day.value} className="day-chip">
                        <input
                          type="checkbox"
                          name="q-day"
                          value={day.value}
                          checked={schedule.daysOfWeek.includes(day.value)}
                          onChange={(e) => toggleDay(day.value, e.target.checked)}
                        />
                        {day.label}
                      </label>
                    ))}
                  </div>
                </div>
                <div className="form-line">
                  <label htmlFor="queue-edit-missed-policy">
                    Missed Run Policy:
                  </label>
                  <select
                    id="queue-edit-missed-policy"
                    value={schedule.missedRunPolicy}
                    onChange={(e) =>
                      patch({
                        schedule: {
                          ...schedule,
                          missedRunPolicy: e.target.value as MissedRunPolicy,
                        },
                      })
                    }
                  >
                    <option value="runOnStartup">
                      Run on Startup (Catch-up if app was closed)
                    </option>
                    <option value="skip">
                      Skip until Next Scheduled Run
                    </option>
                  </select>
                </div>
                <p className="field-help">
                  Timezone: Local system time. Start and stop actions apply only
                  to this queue.
                </p>
              </div>
            )}
          </fieldset>
          {error && (
            <p className="form-error" id="queue-edit-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <div className="dialog-footer">
          <button
            className="button"
            id="queue-edit-cancel-btn"
            type="button"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </button>
          <button
            className="button primary"
            type="submit"
            disabled={saving}
          >
            Save Queue
          </button>
        </div>
      </form>
    </ClassicDialog>
  );
}
