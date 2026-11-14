import { WindowRouter } from "@/app/routes";
import { type WindowRoute } from "@/app/window-mode";

/** The application root. The window's query string decides the screen. */
export function App({ route }: { route: WindowRoute }) {
  return <WindowRouter route={route} />;
}
