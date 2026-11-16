import { useEffect } from "react";
import { WindowRouter } from "@/app/routes";
import { type WindowMode, type WindowRoute } from "@/app/window-mode";
import { useDocumentTitle } from "@/lib/hooks/use-document-title";
import { call } from "@/lib/ipc/client";

const ROUTE_TITLES: Record<WindowMode, string> = {
  main: "Open Download Manager",
  options: "Options",
  progress: "Download progress",
  add: "Download File Info",
  scheduler: "Scheduler",
  integration: "Browser integration",
};

/** The application root. Declaratively sets the window title and renders the route. */
export function App({ route }: { route: WindowRoute }) {
  useDocumentTitle(ROUTE_TITLES[route.mode]);

  useEffect(() => {
    if (route.mode === "main") {
      void call("mark_ui_ready").catch(() => {});
    }
  }, [route.mode]);

  return <WindowRouter route={route} />;
}
