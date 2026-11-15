import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/app/App";
import { Providers } from "@/app/providers";
import { readWindowRoute } from "@/app/window-mode";
import "@/index.css";

const route = readWindowRoute(window.location.search);
const ROUTE_WINDOW_CLASS: Record<string, string> = {
  options: "options-window",
  progress: "progress-window",
  integration: "integration-window",
  scheduler: "scheduler-window",
  add: "add-window",
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
