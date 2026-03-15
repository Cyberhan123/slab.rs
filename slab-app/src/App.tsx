import { useEffect, useRef } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import AppRoutes from "@/routes";
import './styles/globals.css'
import { TooltipProvider } from "@/components/ui/tooltip"
import { Toaster } from "@/components/ui/sonner"
import { ErrorBoundary } from "@/components/error-boundary"
import { QueryClientProvider } from '@tanstack/react-query'
import { queryClient } from "@/lib/api"

const SETUP_STATUS_URL = 'http://localhost:3000/v1/setup/status';

/**
 * On every cold start, check whether the one-time setup wizard has been
 * completed.  If not, redirect to /setup.  Once the wizard marks the
 * environment as initialized it navigates back to "/" itself, so subsequent
 * launches will skip this check immediately.
 */
function SetupGuard() {
  const navigate = useNavigate();
  const location = useLocation();
  const checked = useRef(false);

  useEffect(() => {
    // Only perform the check once per app mount, and never while the user
    // is already on the setup page (avoids an infinite redirect loop).
    if (checked.current || location.pathname === '/setup') return;
    checked.current = true;

    fetch(SETUP_STATUS_URL)
      .then((r) => r.json())
      .then((data: { initialized?: boolean }) => {
        if (!data.initialized) {
          navigate('/setup', { replace: true });
        }
      })
      .catch(() => {
        // Server not reachable yet – go to setup so the user sees a proper
        // error with guidance instead of a blank or broken main page.
        navigate('/setup', { replace: true });
      });
  }, [navigate, location.pathname]);

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
