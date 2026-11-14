import { type DownloadQueue, type QueueSchedule } from "@/types/ipc";

/** Pure presentational rules for the Scheduler window. */

export const DAY_LABELS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 1, label: "Mon" },
  { value: 2, label: "Tue" },
  { value: 3, label: "Wed" },
  { value: 4, label: "Thu" },
  { value: 5, label: "Fri" },
  { value: 6, label: "Sat" },
  { value: 7, label: "Sun" },
];

export function queueTitle(queue: DownloadQueue): string {
  return queue.id === "default" ? `${queue.name} (Default)` : queue.name;
}

export function queueMeta(queue: DownloadQueue): string {
  const items = Array.isArray(queue.itemIds) ? queue.itemIds.length : 0;
  const speed =
    queue.speedLimitKbps > 0 ? `${queue.speedLimitKbps} KB/s` : "Unlimited speed";
  let schedule = "No schedule";
  if (queue.schedule?.enabled) {
    schedule = `Schedule: ${queue.schedule.startTime}${
      queue.schedule.stopTime ? ` - ${queue.schedule.stopTime}` : ""
    }`;
    if (queue.schedule.nextRunAt) {
      schedule += ` (Next: ${formatWhen(queue.schedule.nextRunAt)})`;
    }
  }
  return `${items} download${items === 1 ? "" : "s"} • Max ${queue.concurrencyLimit} active • ${speed} • ${schedule}`;
}

export function formatWhen(value: string | null | undefined): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

/**
 * A local `YYYY-MM-DDTHH:MM` value for `<input type="datetime-local">` and
 * for the Rust side, which parses the same string as local time.
 */
export function localInputValue(timestamp: number): string {
  const date = new Date(timestamp);
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

/** The defaults the legacy queue editor opened with. */
export function defaultSchedule(): QueueSchedule {
  return {
    enabled: true,
    daysOfWeek: [1, 2, 3, 4, 5],
    startTime: "22:00",
    stopTime: "06:00",
    timezone: "local",
    missedRunPolicy: "runOnStartup",
    nextRunAt: null,
    lastRunAt: null,
  };
}

export interface QueueDraft {
  name: string;
  concurrencyLimit: number;
  speedLimitKbps: number;
  schedule: QueueSchedule | null;
}

export function draftFromQueue(queue: DownloadQueue | null): QueueDraft {
  if (!queue) {
    return {
      name: "",
      concurrencyLimit: 2,
      speedLimitKbps: 0,
      schedule: null,
    };
  }
  return {
    name: queue.name,
    concurrencyLimit: queue.concurrencyLimit,
    speedLimitKbps: queue.speedLimitKbps,
    schedule: queue.schedule,
  };
}

export function validateQueueDraft(draft: QueueDraft): string | null {
  const name = draft.name.trim();
  if (!name) return "Give the queue a name.";
  if (name.length > 100) return "Queue names are limited to 100 characters.";
  if (!Number.isInteger(draft.concurrencyLimit) || draft.concurrencyLimit < 1 || draft.concurrencyLimit > 10) {
    return "Max active downloads must be a whole number between 1 and 10.";
  }
  if (!Number.isInteger(draft.speedLimitKbps) || draft.speedLimitKbps < 0) {
    return "The speed limit must be 0 or a positive whole number.";
  }
  if (draft.schedule) {
    if (!/^\d{2}:\d{2}$/.test(draft.schedule.startTime)) {
      return "Choose a valid start time.";
    }
    if (draft.schedule.stopTime && !/^\d{2}:\d{2}$/.test(draft.schedule.stopTime)) {
      return "Choose a valid stop time.";
    }
  }
  return null;
}

/**
 * `update_queue` takes `Option<Option<QueueSchedule>>` on the Rust side.
 * `undefined` keeps the current schedule; `null` clears it. The editor always
 * sends a definite answer, so a ticked schedule replaces and a cleared one
 * removes.
 */
export function schedulePatch(schedule: QueueSchedule | null): QueueSchedule | null {
  return schedule;
}

/** The default queue cannot be deleted or renamed. */
export function canDeleteQueue(queue: DownloadQueue): boolean {
  return queue.id !== "default";
}

export function canRenameQueue(queue: DownloadQueue | null): boolean {
  return !queue || queue.id !== "default";
}
