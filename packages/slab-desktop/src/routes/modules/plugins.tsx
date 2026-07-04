import { useLocation } from "react-router-dom";
import { Puzzle } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@slab/components/alert";
import { Spinner } from "@slab/components/spinner";

import Plugins from "@/pages/plugins";
import { PluginWebviewPage } from "@/pages/plugins/components/plugin-webview-page";
import { useRuntimePlugins } from "@/pages/plugins/hooks/use-runtime-plugins";
import type { DesktopRouteObject } from "../route-meta";

function PluginContributionRoutePage() {
  const location = useLocation();
  const { data: runtimePlugins = [], isLoading } = useRuntimePlugins();
  const pathname = location.pathname.replace(/\/+$/, "") || "/";
  const plugin = runtimePlugins.find((candidate) =>
    candidate.valid &&
    candidate.enabled &&
    candidate.uiEntry &&
    candidate.uiUrl &&
    (candidate.contributions?.routes ?? []).some((route) =>
      (route.path.replace(/\/+$/, "") || "/") === pathname
    )
  );

  if (plugin) {
    return <PluginWebviewPage plugin={plugin} />;
  }

  if (isLoading) {
    return (
      <div className="flex h-full w-full items-center justify-center text-muted-foreground">
        <Spinner className="mr-2 size-4" />
        Loading plugin...
      </div>
    );
  }

  return (
    <div className="flex h-full w-full items-center justify-center p-6">
      <Alert variant="destructive" className="max-w-xl">
        <AlertTitle>Plugin route unavailable</AlertTitle>
        <AlertDescription>
          No enabled plugin owns this route.
        </AlertDescription>
      </Alert>
    </div>
  );
}

export const pluginRoutes: DesktopRouteObject[] = [
  {
    path: "plugins",
    meta: {
      title: "Plugins",
      subtitle: "Run workspace plugins with Extism runtime",
      icon: Puzzle,
      sidebar: {
        group: "primary",
        labelKey: "layouts.sidebar.items.plugins",
      },
    },
    element: <Plugins />,
  },
  {
    path: "plugins/*",
    element: <PluginContributionRoutePage />,
  },
];
