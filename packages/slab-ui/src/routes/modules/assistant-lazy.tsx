import { Suspense, lazy } from "react";
import { Navigate } from "react-router-dom";
import { BotMessageSquare } from "lucide-react";

import type { SlabRouteObject } from "../route-meta";

/**
 * Lazy variant of {@link assistantRoutes} for shells that don't eagerly mount
 * the assistant page: the dynamic import keeps the assistant chunk (and
 * anything it pulls in) out of the shell's main chunk. Kept in a separate
 * file so eagerly-mounted desktop routes never import this module.
 */
const LazyAssistant = lazy(async () => {
  const mod = await import("@slab/ui/pages/assistant");
  return { default: mod.default };
});

export const lazyAssistantRoutes: SlabRouteObject[] = [
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
    element: (
      <Suspense fallback={null}>
        <LazyAssistant />
      </Suspense>
    ),
  },
  {
    path: "agent",
    element: <Navigate to="/" replace />,
  },
];
