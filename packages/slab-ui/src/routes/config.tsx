import App from "@slab/ui/app/App";
import Layout from "@slab/ui/layouts";
import type { SlabRouteObject } from "./route-meta";
import { createSlabRoutes } from "./create-slab-routes";
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

export { createSlabRoutes } from "./create-slab-routes";
export type { SlabRoutesConfig } from "./create-slab-routes";

// ── Desktop assembly (full route set; kept as the pre-factory exports) ──────

const desktopLayoutChildren: SlabRouteObject[] = [
  ...assistantRoutes,
  workspaceRoute,
  ...mediaRoutes,
  hubRoute,
  taskRoute,
  ...pluginRoutes,
  settingsRoute,
  aboutRoute,
];

function DesktopLayoutRoute() {
  return <Layout routes={staticDesktopRoutes} />;
}

export const staticDesktopRoutes: SlabRouteObject[] = [
  setupRoute,
  {
    path: "/",
    element: <DesktopLayoutRoute />,
    children: desktopLayoutChildren,
  },
  themePreviewRoute,
] satisfies SlabRouteObject[];

export function createDesktopRoutes(): SlabRouteObject[] {
  return createSlabRoutes({
    app: <App />,
    layoutChildren: desktopLayoutChildren,
    rootChildren: [setupRoute, themePreviewRoute],
  });
}
