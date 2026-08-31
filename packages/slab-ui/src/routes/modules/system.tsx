import { lazy, Suspense } from "react";
import {
  ClipboardList,
  Info,
  Package,
  Palette,
  Settings,
} from "lucide-react";

import About from "@slab/ui/pages/about";
import Hub from "@slab/ui/pages/hub";
import SettingsPage from "@slab/ui/pages/settings";
import Task from "@slab/ui/pages/task";
import type { SlabRouteObject } from "../route-meta";

// Lazy AND dev-guarded: the top-level `lazy()` call would otherwise survive
// tree-shaking as a side-effect statement and drag the preview chunk into
// production builds. With the `import.meta.env.DEV` ternary the dynamic import
// sits in a dead branch and the chunk is never emitted.
const ThemePreview = import.meta.env.DEV
  ? lazy(() =>
      import("@slab/ui/components/theme-preview").then((m) => ({
        default: m.ThemePreview,
      })),
    )
  : null;

function ThemePreviewRouteElement() {
  if (ThemePreview === null) {
    return null;
  }

  return (
    <Suspense fallback={null}>
      <ThemePreview />
    </Suspense>
  );
}

export const hubRoute = {
  path: "hub",
  meta: {
    title: "Hub",
    subtitle: "Models Repository",
    icon: Package,
    sidebar: {
      group: "primary",
      labelKey: "layouts.sidebar.items.hub",
    },
  },
  element: <Hub />,
} satisfies SlabRouteObject;

export const taskRoute = {
  path: "task",
  meta: {
    title: "Tasks",
    subtitle: "Track and manage system tasks",
    icon: ClipboardList,
    sidebar: {
      group: "primary",
      labelKey: "layouts.sidebar.items.task",
    },
  },
  element: <Task />,
} satisfies SlabRouteObject;

export const settingsRoute = {
  path: "settings",
  meta: {
    title: "Settings",
    subtitle: "Configure app and backend options",
    icon: Settings,
    sidebar: {
      group: "footer",
      labelKey: "layouts.sidebar.items.settings",
    },
  },
  element: <SettingsPage />,
} satisfies SlabRouteObject;

export const aboutRoute = {
  path: "about",
  meta: {
    title: "About",
    subtitle: "Project and runtime information",
    icon: Info,
  },
  element: <About />,
} satisfies SlabRouteObject;

export const themePreviewRoute = {
  path: "/theme-preview",
  meta: {
    title: "Theme Preview",
    subtitle: "Preview UI components and design tokens",
    icon: Palette,
  },
  element: <ThemePreviewRouteElement />,
} satisfies SlabRouteObject;
