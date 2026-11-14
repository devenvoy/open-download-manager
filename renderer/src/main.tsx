import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/app/App";
import { Providers } from "@/app/providers";
import { readWindowRoute } from "@/app/window-mode";
import "@/index.css";
import "@/styles.css";
import "@/torrents.css";

const route = readWindowRoute(window.location.search);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Providers>
      <App route={route} />
    </Providers>
  </React.StrictMode>,
);
