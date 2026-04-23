import { useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { ErrorBoundary } from "@/components/error-boundary";
import {
  applyAppLanguagePreference,
  isAppLanguagePreference,
} from "@slab/i18n";
import { Toaster } from "@slab/components/sonner";
import { TooltipProvider } from "@slab/components/tooltip";
import api, { queryClient } from "@/lib/api";
import {
  pluginSetThemeSnapshot,
  readPluginThemeSnapshot,
} from "@/lib/plugin-host-bridge";
import AppRoutes from "@/routes";

/**
 * Checks whether the one-time setup wizard has been completed the first time
 * the shell needs it. Redirects to /setup only when the server responds and
 * reports `initialized: false`.
 *
 * The desktop host now spawns `slab-server` asynchronously, so transient
 * transport errors during boot should not be treated as a setup signal.
 */
function SetupGuard() {
  const navigate = useNavigate();
  const location = useLocation();
  const isSetupRoute = location.pathname === "/setup";

  const { data: setupStatus, refetch: refetchSetupStatus } = api.useQuery(
    "get",
    "/v1/setup/status",
    undefined,
    {
      enabled: !isSetupRoute,
      staleTime: 0,
      refetchOnMount: "always",
      refetchOnReconnect: true,
      refetchOnWindowFocus: true,
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

  useEffect(() => {
    if (isSetupRoute) {
      return;
    }

    void refetchSetupStatus();
  }, [isSetupRoute, location.pathname, refetchSetupStatus]);

  return null;
}

function AppLanguageSync() {
  const lastAppliedPreferenceRef = useRef<string | null>(null);
  const { data } = api.useQuery(
    "get",
    "/v1/settings/{pmid}",
    {
      params: {
        path: {
          pmid: "general.language",
        },
      },
    },
    {
      staleTime: Number.POSITIVE_INFINITY,
      refetchOnMount: false,
      refetchOnReconnect: true,
      refetchOnWindowFocus: true,
      retry: false,
    }
  );

  useEffect(() => {
    const preference = data?.effective_value;
    if (typeof preference !== "string" || !isAppLanguagePreference(preference)) {
      return;
    }

    if (lastAppliedPreferenceRef.current === preference) {
      return;
    }

    lastAppliedPreferenceRef.current = preference;
    void applyAppLanguagePreference(preference);
  }, [data?.effective_value]);

  return null;
}

function PluginThemeSync() {
  useEffect(() => {
    let animationFrame = 0;

    const publishTheme = () => {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = window.requestAnimationFrame(() => {
        void pluginSetThemeSnapshot(readPluginThemeSnapshot()).catch((error) => {
          console.warn("failed to publish plugin theme snapshot", error);
        });
      });
    };

    publishTheme();

    const observer = new MutationObserver(publishTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class", "style"],
    });

    return () => {
      window.cancelAnimationFrame(animationFrame);
      observer.disconnect();
    };
  }, []);

  return null;
}

function App() {
  return (
    <ErrorBoundary>
      <TooltipProvider>
        <QueryClientProvider client={queryClient}>
          <SetupGuard />
          <AppLanguageSync />
          <PluginThemeSync />
          <AppRoutes />
          <Toaster />
        </QueryClientProvider>
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
