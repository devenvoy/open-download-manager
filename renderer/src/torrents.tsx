import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { Providers } from "@/app/providers";
import { TorrentsScreen } from "@/features/torrents";
import { call } from "@/lib/ipc/client";
import "@/index.css";

document.body.classList.add("torrents-window");

function TorrentsApp() {
  useEffect(() => {
    void call("mark_ui_ready", { label: "torrents" }).catch(() => {});
    void call("window_ready", { label: "torrents" }).catch(() => {});
  }, []);

  return (
    <Providers>
      <TorrentsScreen />
    </Providers>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <TorrentsApp />
  </React.StrictMode>,
);
