import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import "@slab/components/globals.css";
import { createDesktopBrowserRouter } from "@slab/ui/routes";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
import { createDesktopPorts } from "./platform/desktop-ports";

/**
 * App bootstrap, imported dynamically from `main.tsx` after the API health
 * gate passes. The dynamic import is what defers the store modules (zustand
 * `persist` hydrates at store-module import time), so it cannot be replaced
 * by a render-level gate.
 */
export function renderApp(): void {
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
}
