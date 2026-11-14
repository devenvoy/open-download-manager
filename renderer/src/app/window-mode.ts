/**
 * The Rust side opens every window from `index.html` with a query string
 * (src-tauri/src/presentation/window_manager.rs). This module is the single
 * place that turns that string into a route; it is pure so it can be tested
 * without a DOM.
 */

export type WindowMode =
  | "main"
  | "progress"
  | "options"
  | "add"
  | "scheduler"
  | "integration";

export type WindowRoute =
  | { mode: "main" }
  | { mode: "options" }
  | { mode: "progress"; downloadId: string }
  | { mode: "integration" }
  | { mode: "scheduler" }
  | { mode: "add"; paste: boolean; scheduledAt?: string };

export function readWindowRoute(search: string): WindowRoute {
  const params = new URLSearchParams(search);

  const downloadId = params.get("progress");
  if (downloadId) return { mode: "progress", downloadId };

  if (params.get("options") === "1") return { mode: "options" };
  if (params.get("scheduler") === "1") return { mode: "scheduler" };
  if (params.get("integration") === "1") return { mode: "integration" };

  if (params.get("add") === "1") {
    return {
      mode: "add",
      paste: params.has("paste"),
      scheduledAt: params.get("scheduled") || undefined,
    };
  }

  return { mode: "main" };
}
