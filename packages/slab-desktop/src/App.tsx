import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { ErrorBoundary } from "@/components/error-boundary";
import { Toaster } from "@slab/components/sonner";
import { TooltipProvider } from "@slab/components/tooltip";
import api, { queryClient } from "@/lib/api";
import { TASK_POLL_INTERVAL_MS } from "@/pages/setup/const";
import AppRoutes from "@/routes";

/**
 * Checks whether the one-time setup wizard has been completed on every
 * navigation to a non-setup route. Redirects to /setup when either:
 *   - the server reports `initialized: false`, or
 *   - the server is unreachable.
 *
 * The guard uses the shared API query client so Tauri route mapping and
 * polling behaviour stay aligned with the rest of the app.
 */
function SetupGuard() {
  const navigate = useNavigate();
  const location = useLocation();
  const isSetupRoute = location.pathname === "/setup";

  const { data: setupStatus, error: setupStatusError } = api.useQuery(
    "get",
    "/v1/setup/status",
    undefined,
    {
      enabled: !isSetupRoute,
      refetchInterval: isSetupRoute ? false : TASK_POLL_INTERVAL_MS,
      refetchIntervalInBackground: true,
      retry: false,
    }
  );

  useEffect(() => {
    if (isSetupRoute) {
      return;
    }

    if (setupStatusError || setupStatus?.initialized === false) {
      navigate("/setup", { replace: true });
    }
  }, [isSetupRoute, navigate, setupStatus?.initialized, setupStatusError]);

  return null;
}

function App() {
  return (
    <ErrorBoundary>
      <TooltipProvider>
        <QueryClientProvider client={queryClient}>
          <SetupGuard />
          <AppRoutes />
          <Toaster />
        </QueryClientProvider>
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
