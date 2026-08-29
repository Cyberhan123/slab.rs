import {
  createContext,
  useContext,
  useMemo,
  type ReactNode,
} from "react";
import type { QueryClient } from "@tanstack/react-query";

import type { SlabPorts } from "@slab/core";
import { queryClient as defaultQueryClient } from "../lib/query-client";

/**
 * Dependencies injected by each shell (desktop/web) at assembly time.
 *
 * Shells pick the concrete infra adapters (`@slab/core/infra/*`) — UI code
 * only ever sees the port interfaces through this context.
 */
export interface SlabDependencies {
  ports: SlabPorts;
  queryClient?: QueryClient;
}

const SlabContext = createContext<SlabDependencies | null>(null);

export function SlabProvider({
  deps,
  children,
}: {
  deps: SlabDependencies;
  children: ReactNode;
}) {
  const value = useMemo(
    () => ({ ports: deps.ports, queryClient: deps.queryClient }),
    [deps.ports, deps.queryClient],
  );

  return <SlabContext.Provider value={value}>{children}</SlabContext.Provider>;
}

/** Access the injected platform ports. Throws when used outside a SlabProvider. */
export function useSlab(): Required<SlabDependencies> {
  const deps = useContext(SlabContext);
  if (!deps) {
    throw new Error("useSlab must be used within a SlabProvider");
  }
  return {
    ports: deps.ports,
    queryClient: deps.queryClient ?? defaultQueryClient,
  };
}
