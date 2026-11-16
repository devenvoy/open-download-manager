import { useEffect } from "react";
import { AddDownloadScreen } from "@/features/add-download";
import { DownloadsScreen } from "@/features/downloads";
import { IntegrationScreen } from "@/features/integration";
import { ProgressScreen } from "@/features/progress";
import { SchedulerScreen } from "@/features/scheduler";
import { SettingsScreen } from "@/features/settings";
import { TorrentsScreen } from "@/features/torrents";
import { type WindowMode, type WindowRoute } from "./window-mode";

const ROUTE_WINDOW_CLASS: Record<WindowMode, string | null> = {
  main: null,
  options: "options-window",
  progress: "progress-window",
  integration: "integration-window",
  scheduler: "scheduler-window",
  add: "add-window",
  torrents: "torrents-window",
};

/** Declaratively syncs window-level styling for auxiliary windows. */
function useWindowShell(mode: WindowMode) {
  useEffect(() => {
    const className = ROUTE_WINDOW_CLASS[mode];
    if (className) {
      document.body.classList.add(className);
      return () => {
        document.body.classList.remove(className);
      };
    }
  }, [mode]);
}

function renderScreen(route: WindowRoute): React.ReactElement {
  switch (route.mode) {
    case "main":
      return <DownloadsScreen />;
    case "options":
      return <SettingsScreen />;
    case "progress":
      return <ProgressScreen id={route.downloadId} />;
    case "integration":
      return <IntegrationScreen />;
    case "scheduler":
      return <SchedulerScreen />;
    case "torrents":
      return <TorrentsScreen />;
    case "add":
      return (
        <AddDownloadScreen
          paste={route.paste}
          scheduledAt={route.scheduledAt}
        />
      );
    default: {
      const _exhaustive: never = route;
      return _exhaustive;
    }
  }
}

export function WindowRouter({ route }: { route: WindowRoute }) {
  useWindowShell(route.mode);
  return renderScreen(route);
}
