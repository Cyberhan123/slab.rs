import { useEffect, useState } from "react";
import { RouterProvider, createBrowserRouter } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { createH5Ports } from "@slab/core";
import { queryClient } from "@slab/ui/lib/query-client";
import { SlabProvider } from "@slab/ui/provider/slab-provider";
import { HealthStatus } from "./health-status";
import { MobileShell } from "./mobile-shell";

/**
 * Mobile H5 shell assembly: web-flavored ports, safe-area aware shell, and a
 * page-direct layout (no desktop sidebar).
 */
export function H5App() {
  const [router] = useState(() =>
    createBrowserRouter([
      {
        path: "/",
        element: <AssistantRoute />,
      },
    ]),
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

/** Dynamic island so monaco/workspace code stays out of the H5 bundle. */
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
