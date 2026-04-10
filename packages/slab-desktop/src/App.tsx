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
 * navigation to a non-setup route. Redirects to /setup only when the server
 * responds and reports `initialized: false`.
 *
 * The desktop host now spawns `slab-server` asynchronously, so transient
 * transport errors during boot should not be treated as a setup signal.
 */
function SetupGuard() {
  const navigate = useNavigate();
  const location = useLocation();
  const isSetupRoute = location.pathname === "/setup";

  const { data: setupStatus } = api.useQuery(
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

    if (setupStatus?.initialized === false) {
      navigate("/setup", { replace: true });
    }
  }, [isSetupRoute, navigate, setupStatus?.initialized]);

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
