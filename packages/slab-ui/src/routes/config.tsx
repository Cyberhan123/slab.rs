import App from "@slab/ui/app/App";
import Layout from "@slab/ui/layouts";
import type { DesktopRouteObject } from "./route-meta";
import { assistantRoutes } from "./modules/assistant";
import { mediaRoutes } from "./modules/media";
import {
  aboutRoute,
  hubRoute,
  settingsRoute,
  taskRoute,
  themePreviewRoute,
} from "./modules/system";
import { pluginRoutes } from "./modules/plugins";
import { setupRoute } from "./modules/setup";
import { workspaceRoute } from "./modules/workspace";

function DesktopLayoutRoute() {
  return <Layout routes={staticDesktopRoutes} />;
}

export const staticDesktopRoutes: DesktopRouteObject[] = [
  setupRoute,
  {
    path: "/",
    element: <DesktopLayoutRoute />,
    children: [
      ...assistantRoutes,
      workspaceRoute,
      ...mediaRoutes,
      hubRoute,
      taskRoute,
      ...pluginRoutes,
      settingsRoute,
      aboutRoute,
    ],
  },
  themePreviewRoute,
] satisfies DesktopRouteObject[];

export function createDesktopRoutes(): DesktopRouteObject[] {
  return [
    {
      path: "/",
      element: <App />,
      children: [
        ...staticDesktopRoutes,
        {
          path: "*",
          element: null,
        },
      ],
    },
  ];
}
