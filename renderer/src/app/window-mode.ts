/**
 * Routes auxiliary and main windows. Reads window label first (synchronous
 * via Tauri v2 getCurrentWebviewWindow().label), falling back to query strings
 * for backward compatibility and testing without a DOM.
 */

export type WindowMode =
  | "main"
  | "progress"
  | "options"
  | "add"
  | "scheduler"
  | "integration"
  | "torrents";

export type WindowRoute =
  | { mode: "main" }
  | { mode: "options" }
  | { mode: "progress"; downloadId: string }
  | { mode: "integration" }
  | { mode: "scheduler" }
  | { mode: "torrents" }
  | { mode: "add"; paste: boolean; scheduledAt?: string };

export function readWindowRoute(
  labelOrSearch?: string,
  searchFallback?: string,
): WindowRoute {
  let label = labelOrSearch;
  let search = searchFallback;

  // Backward compatibility: If first parameter is a query string (starts with '?')
  // or contains '=', treat it as the search string.
  if (labelOrSearch && (labelOrSearch.startsWith("?") || labelOrSearch.includes("="))) {
    search = labelOrSearch;
    label = undefined;
  }

  // 1. Primary: Match by window label (Tauri v2 synchronous label routing)
  if (label) {
    if (label === "options") return { mode: "options" };
    if (label === "scheduler") return { mode: "scheduler" };
    if (label === "integration") return { mode: "integration" };
    if (label === "torrents") return { mode: "torrents" };
    if (label === "add") {
      const params = search ? new URLSearchParams(search) : null;
      return {
        mode: "add",
        paste: params?.has("paste") ?? false,
        scheduledAt: params?.get("scheduled") || undefined,
      };
    }
    if (label.startsWith("progress-")) {
      const downloadId = label.slice("progress-".length);
      return { mode: "progress", downloadId };
    }
    if (label === "main") {
      return { mode: "main" };
    }
  }

  // 2. Secondary fallback: Query parameters
  if (search) {
    const params = new URLSearchParams(search);
    const downloadId = params.get("progress");
    if (downloadId) return { mode: "progress", downloadId };

    if (params.get("options") === "1") return { mode: "options" };
    if (params.get("scheduler") === "1") return { mode: "scheduler" };
    if (params.get("integration") === "1") return { mode: "integration" };
    if (params.get("torrents") === "1") return { mode: "torrents" };

    if (params.get("add") === "1") {
      return {
        mode: "add",
        paste: params.has("paste"),
        scheduledAt: params.get("scheduled") || undefined,
      };
    }
  }

  return { mode: "main" };
}
