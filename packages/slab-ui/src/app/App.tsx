import { useCallback, useEffect, useRef } from "react";
import { Outlet, useLocation, useNavigate, useSearchParams } from "react-router-dom";
import { QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { useMutationObserverTarget } from "@mantine/hooks";
import { sortBy } from "lodash-es";

import { ErrorBoundary } from "@slab/ui/components/error-boundary";
import { Toaster } from "@slab/components/sonner";
import { TooltipProvider } from "@slab/components/tooltip";
import api from "@slab/api";
import { queryClient } from "@slab/ui/lib/query-client";
import { AppLanguageSync, SetupGuard } from "./app-guards";
import {
  getPluginHost,
  readPluginThemeSnapshot,
} from "@slab/core/platform/plugin-host";
import { useWorkspaceState } from "@slab/ui/pages/workspace/hooks/use-workspace-state";
import { RUNTIME_PLUGINS_QUERY_KEY } from "@slab/ui/pages/plugins/hooks/use-runtime-plugins";
import { isPluginRunning } from "@slab/ui/pages/plugins/utils";
import { useWorkspaceUiStore } from "@slab/ui/store/useWorkspaceUiStore";
import { GUARDRAIL_PMIDS, useGuardrailFlag } from "@slab/ui/lib/guardrail-flags";

const PLUGIN_THEME_OBSERVER_OPTIONS: MutationObserverInit = {
  attributes: true,
  attributeFilter: ["class", "style"],
};

function getDocumentElement() {
  return document.documentElement;
}

/** Publishes the document theme as a snapshot to the plugin host (desktop). */
function PluginThemeSync() {
  const animationFrameRef = useRef(0);
  const publishTheme = useCallback(() => {
    window.cancelAnimationFrame(animationFrameRef.current);
    animationFrameRef.current = window.requestAnimationFrame(() => {
      void getPluginHost().setThemeSnapshot(readPluginThemeSnapshot()).catch((error) => {
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
  const [searchParams] = useSearchParams();
  // A `?session=` deep link intentionally pins the Assistant to a specific
  // session; honor it instead of bouncing `/` to `/workspace` (which would
  // drop the query). Lets concurrent e2e browsers each bind their own session.
  const hasSessionDeepLink = searchParams.has("session");
  const workspaceQueryClient = useQueryClient();
  const initialPathRef = useRef(location.pathname);
  const redirectedWorkspaceRootRef = useRef<string | null>(null);
  const appliedPluginConfigSignatureRef = useRef<string | null>(null);

  const workspaceQuery = useWorkspaceState();
  const workspace = workspaceQuery.data?.current ?? null;
  const workspaceConfig = workspaceQuery.data?.config ?? null;
  // A workspace opened FROM the assistant page's Sender dropdown must not fire
  // the `/` → `/workspace` redirect — the whole point of the live switch is to
  // keep chatting on the same page. Session-scoped (see the store).
  const assistantPinnedWorkspaceRoot = useWorkspaceUiStore(
    (state) => state.assistantPinnedWorkspaceRoot,
  );

  const {
    data: pluginRows,
    refetch: refetchPlugins,
    isFetching: pluginsFetching,
  } = api.useQuery("get", "/v1/plugins", undefined, {
    enabled: Boolean(workspace),
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
      redirectedWorkspaceRootRef.current !== workspace.rootPath &&
      workspace.rootPath !== assistantPinnedWorkspaceRoot &&
      !hasSessionDeepLink
    ) {
      redirectedWorkspaceRootRef.current = workspace.rootPath;
      navigate("/workspace", { replace: true });
    }
  }, [assistantPinnedWorkspaceRoot, hasSessionDeepLink, navigate, workspace]);

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

function WorkspaceLazyRollbackPreload() {
  const workspaceMonacoLazyEnabled = useGuardrailFlag(GUARDRAIL_PMIDS.workspaceMonacoLazy);

  useEffect(() => {
    if (workspaceMonacoLazyEnabled) {
      return;
    }

    void import("@slab/ui/pages/workspace");
  }, [workspaceMonacoLazyEnabled]);

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
          <WorkspaceLazyRollbackPreload />
          <Outlet />
          <Toaster />
        </QueryClientProvider>
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
