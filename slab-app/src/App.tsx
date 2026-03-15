import { useEffect, useRef, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import AppRoutes from "@/routes";
import './styles/globals.css'
import { TooltipProvider } from "@/components/ui/tooltip"
import { Toaster } from "@/components/ui/sonner"
import { ErrorBoundary } from "@/components/error-boundary"
import { QueryClientProvider } from '@tanstack/react-query'
import { queryClient } from "@/lib/api"
import { SERVER_BASE_URL } from '@/lib/config';

const SETUP_STATUS_URL = `${SERVER_BASE_URL}/v1/setup/status`;

/**
 * Checks whether the one-time setup wizard has been completed on every
 * navigation to a non-setup route.  Redirects to /setup when either:
 *   - the server reports `initialized: false`, or
 *   - the server is unreachable (so the user sees the setup page's error UI
 *     instead of a blank or broken main page).
 *
 * The result is cached in component state so repeated navigations within the
 * same session don't re-fetch.  Once the wizard calls `complete_setup` and
 * the flag is persisted, the next cold-start will skip the wizard entirely.
 */
function SetupGuard() {
  const navigate = useNavigate();
  const location = useLocation();
  // tri-state: null = not yet checked, true = initialized, false = needs setup
  const [initialized, setInitialized] = useState<boolean | null>(null);

  useEffect(() => {
    // Never redirect while the user is already on the setup page.
    if (location.pathname === '/setup') return;
    // If we have already confirmed initialization this session, skip the fetch.
    if (initialized === true) return;

    fetch(SETUP_STATUS_URL)
      .then(async (r) => {
        if (!r.ok) {
          // Non-2xx from the server — treat as "not ready".
          setInitialized(false);
          navigate('/setup', { replace: true });
          return;
        }
        const data = (await r.json()) as { initialized?: boolean };
        if (!data.initialized) {
          setInitialized(false);
          navigate('/setup', { replace: true });
        } else {
          setInitialized(true);
        }
      })
      .catch(() => {
        // Server not reachable yet – go to setup so the user sees a proper
        // error with guidance instead of a blank or broken main page.
        setInitialized(false);
        navigate('/setup', { replace: true });
      });
  // Re-evaluate on every pathname change so a manual navigation to "/" after
  // an incomplete setup is still caught.
  }, [navigate, location.pathname, initialized]);

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
