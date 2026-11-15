/**
 * TypeScript mirrors of the Rust DTOs.
 *
 * The types live in domain modules; this file is the barrel the rest of the
 * app imports from. `Snapshot` stays here because it aggregates every domain.
 */

import { type DownloadSummary } from "./download";
import { type IntegrationStatus } from "./integration";
import { type Settings } from "./settings";

export * from "./credentials";
export * from "./download";
export * from "./integration";
export * from "./media";
export * from "./queue";
export * from "./settings";
export * from "./torrent";

/** `Snapshot` — src-tauri/src/domain/snapshot.rs */
export interface Snapshot {
  downloads: DownloadSummary[];
  settings: Settings;
  integration: IntegrationStatus;
  fileIcons: Record<string, string>;
}
