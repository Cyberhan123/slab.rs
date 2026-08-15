import { useEffect, useState } from "react";
import { RouterProvider, createBrowserRouter } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { createWebPorts } from "@slab/core";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
import { HealthStatus } from "./health-status";

/**
 * Web shell assembly: install the web platform ports, the shared query
 * client, and the shared routes. The minimal shell mounts the assistant
 * page plus a health probe; more route modules can be added as the web
 * feature set grows.
 */
export function WebApp() {
  const [router] = useState(() =>
    createBrowserRouter([
      {
        path: "/",
        element: <AssistantRoute />,
      },
    ]),
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

/** Dynamic island so monaco/workspace code stays out of the web bundle. */
function AssistantRoute() {
  const [Assistant, setAssistant] = useState<React.ComponentType | null>(null);

  useEffect(() => {
    let cancelled = false;
    void import("@slab/ui/pages/assistant").then((mod) => {
      if (!cancelled) setAssistant(() => mod.default);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!Assistant) return null;
  return <Assistant />;
}
