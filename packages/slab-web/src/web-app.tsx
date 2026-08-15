import { useState } from "react";
import { Outlet, RouterProvider, createBrowserRouter } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { createWebPorts } from "@slab/core";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
// Deep paths, not the routes barrel: the barrel re-exports the desktop
// assembly (every feature module) and would drag it into the web bundle.
import { createSlabRoutes } from "@slab/ui/routes/create-slab-routes";
import { lazyAssistantRoutes } from "@slab/ui/routes/modules/assistant-lazy";
import { HealthStatus } from "./health-status";

/**
 * Web shell assembly: install the web platform ports, the shared query
 * client, and the shared route modules. The minimal shell mounts the lazy
 * assistant island (keeps monaco/workspace out of the web bundle); more route
 * modules can be added as the web feature set grows.
 */
export function WebApp() {
  const [router] = useState(() =>
    createBrowserRouter(
      // Minimal root for now (guards land with the web App variant); the
      // lazy assistant island keeps the heavy chunks out of the main bundle.
      createSlabRoutes({ app: <Outlet />, rootChildren: [...lazyAssistantRoutes] }),
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
