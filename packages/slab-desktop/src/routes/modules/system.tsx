import {
  ClipboardList,
  Info,
  Package,
  Palette,
  Settings,
} from "lucide-react";

import { ThemePreview } from "@/components/theme-preview";
import About from "@/pages/about";
import Hub from "@/pages/hub";
import SettingsPage from "@/pages/settings";
import Task from "@/pages/task";
import type { DesktopRouteObject } from "../route-meta";

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
} satisfies DesktopRouteObject;

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
} satisfies DesktopRouteObject;

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
} satisfies DesktopRouteObject;

export const aboutRoute = {
  path: "about",
  meta: {
    title: "About",
    subtitle: "Project and runtime information",
    icon: Info,
  },
  element: <About />,
} satisfies DesktopRouteObject;

export const themePreviewRoute = {
  path: "/theme-preview",
  meta: {
    title: "Theme Preview",
    subtitle: "Preview UI components and design tokens",
    icon: Palette,
  },
  element: <ThemePreview />,
} satisfies DesktopRouteObject;
