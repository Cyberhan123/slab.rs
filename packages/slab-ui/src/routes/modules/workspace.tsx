import { lazy, Suspense } from "react";
import { FolderKanban } from "lucide-react";
import { Spinner } from "@slab/components/spinner";

import type { DesktopRouteObject } from "../route-meta";

const WorkspacePage = lazy(() => import("@slab/ui/pages/workspace"));

function WorkspaceRouteElement() {
  return (
    <Suspense
      fallback={
        <div className="flex h-full w-full items-center justify-center">
          <Spinner className="h-8 w-8" />
        </div>
      }
    >
      <WorkspacePage />
    </Suspense>
  );
}

export const workspaceRoute = {
  path: "workspace",
  meta: {
    title: "Workspace",
    subtitle: "Open and edit local project files",
    icon: FolderKanban,
    sidebar: {
      group: "primary",
      labelKey: "layouts.sidebar.items.workspace",
    },
  },
  element: <WorkspaceRouteElement />,
} satisfies DesktopRouteObject;
