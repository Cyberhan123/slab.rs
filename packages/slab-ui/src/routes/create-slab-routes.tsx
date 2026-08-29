import type { ReactElement } from "react";
import Layout from "@slab/ui/layouts";
import type { SlabRouteObject } from "./route-meta";

/**
 * Shell-agnostic route assembly. Shells pick the route modules they mount:
 * the desktop passes the full set (sidebar layout + every feature module),
 * the web shell mounts a lazy assistant island plus its chosen top-level
 * routes without the sidebar layout. Omitting `layoutChildren` skips the
 * sidebar Layout route entirely.
 *
 * This module deliberately imports no feature pages — only the Layout — so
 * lightweight shells can pull it (via `@slab/ui/routes/create-slab-routes`)
 * without dragging the desktop feature graph into their bundle. The desktop
 * feature set lives in `./config`.
 */
export interface SlabRoutesConfig {
  /** Root route element (guards etc.) — shells own their app shell. */
  app: ReactElement;
  /** Route groups mounted under the sidebar Layout route. */
  layoutChildren?: readonly SlabRouteObject[];
  /** Routes mounted at the app root, outside the sidebar Layout. */
  rootChildren?: readonly SlabRouteObject[];
}

export function createSlabRoutes(config: SlabRoutesConfig): SlabRouteObject[] {
  // The Layout consumes the same tree instance it lives in, so sidebar +
  // header-meta derivation always matches the mounted routes.
  const tree: SlabRouteObject[] = [...(config.rootChildren ?? [])];
  if (config.layoutChildren) {
    const layoutChildren = [...config.layoutChildren];
    tree.push({
      path: "/",
      element: <Layout routes={tree} />,
      children: layoutChildren,
    });
  }
  tree.push({ path: "*", element: null });
  return [
    {
      path: "/",
      element: config.app,
      children: tree,
    },
  ];
}
