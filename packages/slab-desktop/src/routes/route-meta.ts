import type { RouteObject } from "react-router-dom";
import type { HeaderMeta } from "@/layouts/header";

export type DesktopRouteSidebarGroup = "primary" | "footer";

export type DesktopRouteMeta = HeaderMeta & {
  sidebar?: {
    group: DesktopRouteSidebarGroup;
    labelKey?: string;
    label?: string;
    end?: boolean;
  };
};

export type DesktopRouteObject = RouteObject & {
  children?: DesktopRouteObject[];
  meta?: DesktopRouteMeta;
};

export type DesktopRouteEntry = {
  path: string;
  route: DesktopRouteObject;
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

export function getRoutePath(route: DesktopRouteObject, parentPath = "") {
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

export function getDesktopRouteEntries(
  routes: readonly DesktopRouteObject[],
  parentPath = "",
): DesktopRouteEntry[] {
  return routes.flatMap((route) => {
    const path = getRoutePath(route, parentPath);
    const entries = [{ path, route }];

    if (!route.children) {
      return entries;
    }

    return [...entries, ...getDesktopRouteEntries(route.children, path)];
  });
}

export function getHeaderMetaForPath(
  pathname: string,
  fallback: HeaderMeta,
  routes: readonly DesktopRouteObject[],
): HeaderMeta {
  const normalizedPathname = normalizeRoutePath(pathname);
  const routeEntry = getDesktopRouteEntries(routes)
    .filter(({ route }) => route.meta)
    .filter(({ path }) => normalizedPathname === path || normalizedPathname.startsWith(`${path}/`))
    .toSorted((a, b) => b.path.length - a.path.length)[0];

  return routeEntry?.route.meta ?? fallback;
}
