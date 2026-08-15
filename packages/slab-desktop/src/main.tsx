import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import "@slab/components/globals.css";
import { createDesktopBrowserRouter } from "@/routes";
import { assembleDesktopPlatform } from "@/platform/desktop-ports";
import "@slab/i18n";

// Install the Tauri platform adapters into @slab/core's seams before any
// harness/image code runs.
assembleDesktopPlatform();

const router = createDesktopBrowserRouter();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>,
);
