import { useCallback, useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { QueryClientProvider, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMutationObserverTarget } from "@mantine/hooks";
import { sortBy } from "lodash-es";

import { ErrorBoundary } from "@/components/error-boundary";
import {
  applyAppLanguagePreference,
  isAppLanguagePreference,
} from "@slab/i18n";
import { Toaster } from "@slab/components/sonner";
import { TooltipProvider } from "@slab/components/tooltip";
import api from "@slab/api";
import { queryClient } from "@/lib/query-client";
import { isTauri } from "@/hooks/use-tauri";
import {
  pluginSetThemeSnapshot,
  readPluginThemeSnapshot,
} from "@/lib/plugin-host-bridge";
import {
  WORKSPACE_STATE_QUERY_KEY,
  workspaceState,
} from "@/lib/workspace-bridge";
import { RUNTIME_PLUGINS_QUERY_KEY } from "@/pages/plugins/hooks/use-runtime-plugins";
import { isPluginRunning } from "@/pages/plugins/utils";
import AppRoutes from "@/routes";

const PLUGIN_THEME_OBSERVER_OPTIONS: MutationObserverInit = {
  attributes: true,
  attributeFilter: ["class", "style"],
};

function getDocumentElement() {
  return document.documentElement;
}

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
      // The setup guard is a redirect gate; boot-time transport failures should
      // be observed on the next explicit probe instead of retried into navigation.
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
  const animationFrameRef = useRef(0);
  const publishTheme = useCallback(() => {
    window.cancelAnimationFrame(animationFrameRef.current);
    animationFrameRef.current = window.requestAnimationFrame(() => {
      void pluginSetThemeSnapshot(readPluginThemeSnapshot()).catch((error) => {
        console.warn("failed to publish plugin theme snapshot", error);
      });
    });
  }, []);

  useMutationObserverTarget(publishTheme, PLUGIN_THEME_OBSERVER_OPTIONS, getDocumentElement);

  useEffect(() => {
    publishTheme();
    return () => {
      window.cancelAnimationFrame(animationFrameRef.current);
    };
  }, [publishTheme]);

  return null;
}

function WorkspaceModeSync() {
  const navigate = useNavigate();
  const location = useLocation();
  const workspaceQueryClient = useQueryClient();
  const initialPathRef = useRef(location.pathname);
  const redirectedWorkspaceRootRef = useRef<string | null>(null);
  const appliedPluginConfigSignatureRef = useRef<string | null>(null);
  const isDesktopTauri = isTauri();

  const workspaceQuery = useQuery({
    queryKey: WORKSPACE_STATE_QUERY_KEY,
    queryFn: workspaceState,
    enabled: isDesktopTauri,
    // This probes the Tauri workspace bridge, not the HTTP API; failures are
    // resolved by the bridge's own reconnect path and should not be retried here.
    retry: false,
  });
  const workspace = workspaceQuery.data?.current ?? null;
  const workspaceConfig = workspaceQuery.data?.config ?? null;

  const {
    data: pluginRows,
    refetch: refetchPlugins,
    isFetching: pluginsFetching,
  } = api.useQuery("get", "/v1/plugins", undefined, {
    enabled: isDesktopTauri && Boolean(workspace),
    retry: 1,
  });
  const stopPluginMutation = api.useMutation("post", "/v1/plugins/{id}/stop", {
    meta: {
      skipGlobalErrorToast: true,
    },
  });

  useEffect(() => {
    if (
      initialPathRef.current === "/" &&
      workspace &&
      redirectedWorkspaceRootRef.current !== workspace.rootPath
    ) {
      redirectedWorkspaceRootRef.current = workspace.rootPath;
      navigate("/workspace", { replace: true });
    }
  }, [navigate, workspace]);

  useEffect(() => {
    if (!workspace) {
      appliedPluginConfigSignatureRef.current = null;
    }
  }, [workspace]);

  useEffect(() => {
    const disabledPluginIds = sortBy(
      Object.entries(workspaceConfig?.plugins ?? {})
        .filter(([, preference]) => preference.enabled === false)
        .map(([pluginId]) => pluginId),
    );
    const disabledPluginIdSet = new Set(disabledPluginIds);
    const disabledRunningPluginIds = sortBy(
      (pluginRows ?? [])
        .filter((plugin) => disabledPluginIdSet.has(plugin.id) && isPluginRunning(plugin))
        .map((plugin) => plugin.id),
    );
    const configSignature = workspace
      ? `${workspace.rootPath}:${disabledPluginIds.join(",")}:${disabledRunningPluginIds.join(",")}`
      : null;

    if (
      !workspace ||
      !workspaceConfig ||
      !pluginRows ||
      pluginsFetching ||
      appliedPluginConfigSignatureRef.current === configSignature
    ) {
      return;
    }

    const activeConfigSignature = configSignature;
    const activePlugins = pluginRows;
    let cancelled = false;

    async function applyWorkspacePluginConfig() {
      try {
        await Promise.all(activePlugins.map(async (plugin) => {
          if (disabledPluginIdSet.has(plugin.id) && isPluginRunning(plugin)) {
            await stopPluginMutation.mutateAsync({
              params: { path: { id: plugin.id } },
              // Omit `lastError`: the backend preserves the prior diagnostic on a
              // manual stop, and sending null here would otherwise erase it.
              body: {},
            });
          }
        }));

        if (!cancelled) {
          appliedPluginConfigSignatureRef.current = activeConfigSignature;
          await Promise.all([
            refetchPlugins(),
            workspaceQueryClient.invalidateQueries({ queryKey: RUNTIME_PLUGINS_QUERY_KEY }),
          ]);
        }
      } catch (error) {
        if (!cancelled) {
          console.warn("failed to apply workspace plugin preferences", error);
        }
      }
    }

    void applyWorkspacePluginConfig();

    return () => {
      cancelled = true;
    };
  }, [
    pluginRows,
    pluginsFetching,
    refetchPlugins,
    stopPluginMutation,
    workspace,
    workspaceConfig,
    workspaceQueryClient,
  ]);

  return null;
}

function App() {
  return (
    <ErrorBoundary>
      <TooltipProvider>
        <QueryClientProvider client={queryClient}>
          <SetupGuard />
          <WorkspaceModeSync />
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
