import { createBrowserRouter } from "react-router-dom";

import { createDesktopRoutes } from "./config";

export { createDesktopRoutes, staticDesktopRoutes } from "./config";
export {
  getDesktopRouteEntries,
  getHeaderMetaForPath,
} from "./route-meta";
export type {
  DesktopRouteMeta,
  DesktopRouteObject,
} from "./route-meta";

export function createDesktopBrowserRouter() {
  return createBrowserRouter(createDesktopRoutes());
}
