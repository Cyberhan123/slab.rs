import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import "@slab/components/globals.css";
import { createDesktopBrowserRouter } from "@slab/ui/routes";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
import { assembleDesktopPlatform, createDesktopPorts } from "./platform/desktop-ports";
import "@slab/i18n";

// Install the Tauri platform adapters into @slab/core's seams before any
// harness/image code runs.
assembleDesktopPlatform();

const router = createDesktopBrowserRouter();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SlabProvider deps={{ ports: createDesktopPorts(), queryClient }}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </SlabProvider>
  </React.StrictMode>,
);
