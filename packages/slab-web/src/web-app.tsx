import { useState } from "react";
import { RouterProvider, createBrowserRouter } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { createWebPorts } from "@slab/core";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
import { WebApp as WebAppShell } from "@slab/ui/app/web-app";
// Deep paths, not the routes barrel: the barrel re-exports the desktop
// assembly (every feature module) and would drag it into the web bundle.
import { createSlabRoutes } from "@slab/ui/routes/create-slab-routes";
import { lazyAssistantRoutes } from "@slab/ui/routes/modules/assistant-lazy";
import { setupRoute } from "@slab/ui/routes/modules/setup";
import { HealthStatus } from "./health-status";

/**
 * Web shell assembly: install the web platform ports, the shared query
 * client, and the shared route modules. Mounts the shell-agnostic guards
 * (setup gate redirects to /setup when the server is uninitialized — hence
 * setupRoute must be mounted) plus the lazy assistant island that keeps
 * monaco/workspace out of the web bundle.
 */
export function WebApp() {
  const [router] = useState(() =>
    createBrowserRouter(
      createSlabRoutes({
        app: <WebAppShell />,
        rootChildren: [...lazyAssistantRoutes, setupRoute],
      }),
    ),
  );

  return (
    <SlabProvider deps={{ ports: createWebPorts(), queryClient }}>
      <QueryClientProvider client={queryClient}>
        <div className="min-h-screen bg-app-canvas px-6 py-8 text-foreground">
          <HealthStatus />
          <RouterProvider router={router} />
        </div>
      </QueryClientProvider>
    </SlabProvider>
  );
}
