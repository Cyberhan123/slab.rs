import { useState } from "react";
import { RouterProvider, createBrowserRouter } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { createH5Ports } from "@slab/core";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
import { WebApp as WebAppShell } from "@slab/ui/app/web-app";
// Deep paths, not the routes barrel: the barrel re-exports the desktop
// assembly (every feature module) and would drag it into the H5 bundle.
import { createSlabRoutes } from "@slab/ui/routes/create-slab-routes";
import { lazyAssistantRoutes } from "@slab/ui/routes/modules/assistant-lazy";
import { setupRoute } from "@slab/ui/routes/modules/setup";
import { HealthStatus } from "./health-status";
import { MobileShell } from "./mobile-shell";

/**
 * Mobile H5 shell assembly: web-flavored ports, safe-area aware shell, and a
 * page-direct layout (no desktop sidebar). Mounts the shell-agnostic guards
 * (setup gate + language sync) and the lazy assistant island so
 * monaco/workspace code stays out of the H5 bundle.
 */
export function H5App() {
  const [router] = useState(() =>
    createBrowserRouter(
      createSlabRoutes({
        app: <WebAppShell />,
        rootChildren: [...lazyAssistantRoutes, setupRoute],
      }),
    ),
  );

  return (
    <SlabProvider deps={{ ports: createH5Ports(), queryClient }}>
      <QueryClientProvider client={queryClient}>
        <MobileShell>
          <HealthStatus />
          <RouterProvider router={router} />
        </MobileShell>
      </QueryClientProvider>
    </SlabProvider>
  );
}
