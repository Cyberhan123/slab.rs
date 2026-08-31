import { useQuery } from "@tanstack/react-query"

import {
  workspaceState,
  WORKSPACE_STATE_QUERY_KEY,
} from "@slab/core/workspace/bridge"

export { WORKSPACE_STATE_QUERY_KEY }

/**
 * Shared workspace-state query for the app shell (`WorkspaceModeSync`), the
 * assistant new-chat landing, and the workspace page. Workspace state is
 * fetched over the /v1/workspace HTTP API. The bridge has its own recovery
 * path, so React Query retry would duplicate local probes.
 */
export function useWorkspaceState() {
  return useQuery({
    queryKey: WORKSPACE_STATE_QUERY_KEY,
    queryFn: workspaceState,
    retry: false,
  })
}
