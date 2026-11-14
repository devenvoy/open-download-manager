import { useEffect } from "react";
import { WindowRouter } from "@/app/routes";
import { type WindowRoute } from "@/app/window-mode";

/** The application root. The window's query string decides the screen and body class. */
export function App({ route }: { route: WindowRoute }) {
  useEffect(() => {
    const body = document.body;
    body.classList.remove(
      "progress-window",
      "integration-window",
      "options-window",
      "add-window",
      "scheduler-window",
      "torrents-window",
    );

    switch (route.mode) {
      case "options":
        body.classList.add("options-window");
        document.title = "Options";
        break;
      case "progress":
        body.classList.add("progress-window");
        document.title = "Download progress";
        break;
      case "add":
        body.classList.add("add-window");
        document.title = "Download File Info";
        break;
      case "scheduler":
        body.classList.add("scheduler-window");
        document.title = "Scheduler";
        break;
      case "integration":
        body.classList.add("integration-window");
        document.title = "Browser integration";
        break;
      default:
        document.title = "Open Download Manager";
        break;
    }
  }, [route.mode]);

  return <WindowRouter route={route} />;
}
