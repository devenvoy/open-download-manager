import { AddDownloadScreen } from "@/features/add-download";
import { DownloadsScreen } from "@/features/downloads";
import { IntegrationScreen } from "@/features/integration";
import { ProgressScreen } from "@/features/progress";
import { SchedulerScreen } from "@/features/scheduler";
import { SettingsDialog, SettingsScreen } from "@/features/settings";
import { type WindowRoute } from "./window-mode";

export function WindowRouter({ route }: { route: WindowRoute }) {
  switch (route.mode) {
    case "main":
      return <DownloadsScreen headerActions={<SettingsDialog />} />;
    case "options":
      return <SettingsScreen />;
    case "progress":
      return <ProgressScreen id={route.downloadId ?? ""} />;
    case "integration":
      return <IntegrationScreen />;
    case "scheduler":
      return <SchedulerScreen />;
    case "add":
      return (
        <AddDownloadScreen
          paste={Boolean(route.paste)}
          scheduledAt={route.scheduledAt}
        />
      );
  }
}
