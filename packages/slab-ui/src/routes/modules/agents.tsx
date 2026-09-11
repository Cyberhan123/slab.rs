import { ScrollText } from "lucide-react";

import AgentRolloutsPage from "@slab/ui/pages/agent-rollouts";
import type { SlabRouteObject } from "../route-meta";

/**
 * Rollout debug viewer ("Agent 调试追踪"): read-only inspection of the rollout
 * true source + trace bundles. The sidebar entry is hidden unless the
 * `agent.debug` setting is on (see `layouts/sidebar.tsx`); the server 404s the
 * endpoints regardless, so the route is safe to register unconditionally.
 */
export const agentRolloutsRoute = {
  path: "agent-rollouts",
  meta: {
    title: "Rollouts",
    subtitle: "Inspect rollout sessions and trace bundles",
    icon: ScrollText,
    sidebar: {
      group: "footer",
      labelKey: "layouts.sidebar.items.agentRollouts",
    },
  },
  element: <AgentRolloutsPage />,
} satisfies SlabRouteObject;
