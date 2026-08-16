import type { RouteObject } from "react-router-dom";
import type { HeaderMeta } from "@slab/ui/layouts/header";

export type SlabRouteSidebarGroup = "primary" | "footer";

export type SlabRouteMeta = HeaderMeta & {
  sidebar?: {
    group: SlabRouteSidebarGroup;
    labelKey?: string;
    label?: string;
    end?: boolean;
  };
};

export type SlabRouteObject = RouteObject & {
  children?: SlabRouteObject[];
  meta?: SlabRouteMeta;
};

export type SlabRouteEntry = {
  path: string;
  route: SlabRouteObject;
};

function normalizeRoutePath(path: string) {
  const normalized = path.replace(/\/+$/, "");
  return normalized || "/";
}

function joinRoutePath(parentPath: string, childPath: string) {
  const normalizedParent = normalizeRoutePath(parentPath);
  const normalizedChild = childPath.replace(/^\/+/, "");

  if (!normalizedChild) {
    return normalizedParent;
  }

  if (normalizedParent === "/") {
    return `/${normalizedChild}`;
  }

  return `${normalizedParent}/${normalizedChild}`;
}

export function getRoutePath(route: SlabRouteObject, parentPath = "") {
  if (route.index) {
    return normalizeRoutePath(parentPath || "/");
  }

  if (!route.path) {
    return normalizeRoutePath(parentPath || "/");
  }

  if (route.path.startsWith("/")) {
    return normalizeRoutePath(route.path);
  }

  return joinRoutePath(parentPath || "/", route.path);
}

export function getSlabRouteEntries(
  routes: readonly SlabRouteObject[],
  parentPath = "",
): SlabRouteEntry[] {
  return routes.flatMap((route) => {
    const path = getRoutePath(route, parentPath);
    const entries = [{ path, route }];

    if (!route.children) {
      return entries;
    }

    return [...entries, ...getSlabRouteEntries(route.children, path)];
  });
}

export function getHeaderMetaForPath(
  pathname: string,
  fallback: HeaderMeta,
  routes: readonly SlabRouteObject[],
): HeaderMeta {
  const normalizedPathname = normalizeRoutePath(pathname);
  const routeEntry = getSlabRouteEntries(routes)
    .filter(({ route }) => route.meta)
    .filter(({ path }) => normalizedPathname === path || normalizedPathname.startsWith(`${path}/`))
    .toSorted((a, b) => b.path.length - a.path.length)[0];

  return routeEntry?.route.meta ?? fallback;
}
