import { createBrowserRouter } from "react-router-dom";

import { createDesktopRoutes } from "./config";

export {
  createDesktopRoutes,
  staticDesktopRoutes,
} from "./config";
export { createSlabRoutes } from "./create-slab-routes";
export type { SlabRoutesConfig } from "./create-slab-routes";
export { lazyAssistantRoutes } from "./modules/assistant-lazy";
export {
  getSlabRouteEntries,
  getHeaderMetaForPath,
} from "./route-meta";
export type {
  SlabRouteEntry,
  SlabRouteMeta,
  SlabRouteObject,
  SlabRouteSidebarGroup,
} from "./route-meta";

export function createDesktopBrowserRouter() {
  return createBrowserRouter(createDesktopRoutes());
}
