import React from "react";
import ReactDOM from "react-dom/client";
import { Providers } from "@/app/providers";
import { TorrentsScreen } from "@/features/torrents";
import "@/styles.css";
import "@/torrents.css";
import "@/index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Providers>
      <TorrentsScreen />
    </Providers>
  </React.StrictMode>,
);
