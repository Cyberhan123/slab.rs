import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import "@slab/components/globals.css";
import { createDesktopBrowserRouter } from "./router";
import "@slab/i18n";

const router = createDesktopBrowserRouter();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>,
);
