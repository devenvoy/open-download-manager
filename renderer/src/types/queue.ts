/** Queue and schedule DTOs — mirrors src-tauri/src/domain/queue/mod.rs. */

/** `MissedRunPolicy` */
export type MissedRunPolicy = "runOnStartup" | "skip";

/** `QueueDisposition` — what happens to the items when a queue is deleted. */
export type QueueDisposition = "moveToDefault" | "removeDownloads";

/** `QueueSchedule` */
export interface QueueSchedule {
  enabled: boolean;
  /** ISO-8601 day numbers, 1 = Monday … 7 = Sunday. */
  daysOfWeek: number[];
  /** 24h `HH:MM`. */
  startTime: string;
  stopTime: string | null;
  timezone: string;
  missedRunPolicy: MissedRunPolicy;
  nextRunAt: string | null;
  lastRunAt: string | null;
}

/** `DownloadQueue` */
export interface DownloadQueue {
  id: string;
  name: string;
  enabled: boolean;
  running: boolean;
  concurrencyLimit: number;
  speedLimitKbps: number;
  itemIds: string[];
  schedule: QueueSchedule | null;
}
