import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { App } from "@/app/App";
import { Providers } from "@/app/providers";
import { readWindowRoute } from "@/app/window-mode";
import "@/index.css";

let windowLabel: string | undefined;
try {
  const internals = (
    window as unknown as {
      __TAURI_INTERNALS__?: {
        metadata?: {
          currentWebview?: { label?: string };
          currentWindow?: { label?: string };
        };
      };
    }
  ).__TAURI_INTERNALS__;
  windowLabel =
    internals?.metadata?.currentWebview?.label ??
    internals?.metadata?.currentWindow?.label ??
    getCurrentWebviewWindow().label;
} catch {
  // Running in non-Tauri browser context or test runner
}

const route = readWindowRoute(windowLabel, window.location.search);
const ROUTE_WINDOW_CLASS: Record<string, string> = {
  options: "options-window",
  progress: "progress-window",
  integration: "integration-window",
  scheduler: "scheduler-window",
  add: "add-window",
  torrents: "torrents-window",
};
const routeClass = ROUTE_WINDOW_CLASS[route.mode];
if (routeClass) {
  document.body.classList.add(routeClass);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Providers>
      <App route={route} />
    </Providers>
  </React.StrictMode>,
);
