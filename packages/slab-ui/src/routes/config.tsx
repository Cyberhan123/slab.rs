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

// Dev-only theme preview: `import.meta.env.DEV` is statically replaced, so the
// conditional below keeps the route (and its lazy chunk) out of production
// builds entirely — /theme-preview does not exist in prod.
const devOnlyRoutes: SlabRouteObject[] = import.meta.env.DEV
  ? [themePreviewRoute]
  : [];

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
  ...devOnlyRoutes,
] satisfies SlabRouteObject[];

export function createDesktopRoutes(): SlabRouteObject[] {
  return createSlabRoutes({
    app: <App />,
    layoutChildren: desktopLayoutChildren,
    rootChildren: [setupRoute, ...devOnlyRoutes],
  });
}
