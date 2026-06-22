import { lazy, Suspense, useEffect } from "react";
import { Navigate, useRoutes } from "react-router-dom";
import Assistant from "@/pages/assistant";
import About from "@/pages/about";
import Settings from "@/pages/settings";
import { ThemePreview } from "@/components/theme-preview";
import Layout from "@/layouts";
import Hub from "@/pages/hub";
import Task from "@/pages/task";
import Audio from "@/pages/audio";
import Video from "@/pages/video";
import Image from "@/pages/image";
import Plugins from "@/pages/plugins";
import { PluginWebviewPage } from "@/pages/plugins/components/plugin-webview-page";
import { useRuntimePlugins } from "@/pages/plugins/hooks/use-runtime-plugins";
import { Spinner } from "@slab/components/spinner";
import { GlobalHeaderProvider } from "@/layouts/global-header-provider";
import { GUARDRAIL_PMIDS, useGuardrailFlag } from "@/lib/guardrail-flags";

// Lazy-load the setup page so it doesn't bloat the main bundle.
const SetupPage = lazy(() => import("@/pages/setup"));
const WorkspacePage = lazy(() => import("@/pages/workspace"));

function WorkspaceLazyRollbackPreload() {
  const workspaceMonacoLazyEnabled = useGuardrailFlag(GUARDRAIL_PMIDS.workspaceMonacoLazy);

  useEffect(() => {
    if (workspaceMonacoLazyEnabled) {
      return;
    }

    void import("@/pages/workspace");
  }, [workspaceMonacoLazyEnabled]);

  return null;
}

function AppRoutes() {
  const { data: runtimePlugins = [] } = useRuntimePlugins();
  const pluginRoutes = runtimePlugins
    .filter((plugin) => plugin.valid && plugin.enabled && plugin.uiEntry && plugin.uiUrl)
    .flatMap((plugin) =>
      (plugin.contributions?.routes ?? []).map((route) => ({
        path: route.path.replace(/^\//, ''),
        element: <PluginWebviewPage plugin={plugin} />,
      })),
    );

  const routes = useRoutes([
    {
      // The setup wizard lives outside the main Layout so it gets a clean,
      // full-screen canvas.  It navigates to "/" once setup is complete.
      path: '/setup',
      element: (
        <GlobalHeaderProvider>
          <Suspense
            fallback={
              <div className="flex h-screen items-center justify-center">
                <Spinner className="h-8 w-8" />
              </div>
            }
          >
            <SetupPage />
          </Suspense>
        </GlobalHeaderProvider>
      ),
    },
    {
      path: '/',
      element: <Layout />,
      children: [
        { index: true, element: <Assistant /> },
        { path: 'agent', element: <Navigate to="/" replace /> },
        { path: 'image', element: <Image /> },
        { path: 'audio', element: <Audio /> },
        { path: 'video', element: <Video /> },
        { path: 'hub', element: <Hub /> },
        {
          path: 'workspace',
          element: (
            <Suspense
              fallback={
                <div className="flex h-full w-full items-center justify-center">
                  <Spinner className="h-8 w-8" />
                </div>
              }
            >
              <WorkspacePage />
            </Suspense>
          ),
        },
        { path: 'plugins', element: <Plugins /> },
        ...pluginRoutes,
        { path: 'task', element: <Task /> },
        { path: 'settings', element: <Settings /> },
        { path: 'about', element: <About /> },
      ],
    },
    { path: "/theme-preview", element: <ThemePreview /> },
  ]);

  return (
    <>
      <WorkspaceLazyRollbackPreload />
      {routes}
    </>
  );
}

export default AppRoutes;
