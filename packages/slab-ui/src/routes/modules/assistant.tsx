import { Navigate } from "react-router-dom";
import { BotMessageSquare } from "lucide-react";

import Assistant from "@slab/ui/pages/assistant";
import type { DesktopRouteObject } from "../route-meta";

export const assistantRoutes: DesktopRouteObject[] = [
  {
    index: true,
    meta: {
      title: "Assistant",
      subtitle: "Talk with AI Assistants",
      icon: BotMessageSquare,
      sidebar: {
        group: "primary",
        labelKey: "layouts.sidebar.items.assistant",
        end: true,
      },
    },
    element: <Assistant />,
  },
  {
    path: "agent",
    element: <Navigate to="/" replace />,
  },
];
