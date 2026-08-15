import React from "react";
import ReactDOM from "react-dom/client";
import "@slab/components/globals.css";
import "@slab/i18n";

import { WebApp } from "./web-app";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WebApp />
  </React.StrictMode>,
);
