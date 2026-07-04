import { lazy, Suspense } from "react";
import { Package } from "lucide-react";
import { Spinner } from "@slab/components/spinner";

import type { HeaderMeta } from "@/layouts/header";
import { HeaderProvider } from "@/layouts/header-provider";
import type { DesktopRouteObject } from "../route-meta";

const SetupPage = lazy(() => import("@/pages/setup"));

const setupRouteMeta = {
  title: "Setup",
  subtitle: "Initialize local runtime dependencies",
  icon: Package,
} satisfies HeaderMeta;

function SetupRouteElement() {
  return (
    <HeaderProvider defaultMeta={setupRouteMeta}>
      <Suspense
        fallback={
          <div className="flex h-screen items-center justify-center">
            <Spinner className="h-8 w-8" />
          </div>
        }
      >
        <SetupPage />
      </Suspense>
    </HeaderProvider>
  );
}

export const setupRoute = {
  path: "/setup",
  meta: setupRouteMeta,
  element: <SetupRouteElement />,
} satisfies DesktopRouteObject;
