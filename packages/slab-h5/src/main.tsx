import React from "react";
import ReactDOM from "react-dom/client";
import "@slab/components/globals.css";
import "@slab/i18n";

import { H5App } from "./h5-app";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <H5App />
  </React.StrictMode>,
);
