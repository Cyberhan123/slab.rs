import { Outlet } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";

import { ErrorBoundary } from "@slab/ui/components/error-boundary";
import { Toaster } from "@slab/components/sonner";
import { TooltipProvider } from "@slab/components/tooltip";
import { queryClient } from "@slab/ui/lib/query-client";
import { AppLanguageSync, SetupGuard } from "./app-guards";

/**
 * App shell for the non-desktop shells (web/h5): the shell-agnostic guards
 * only — setup gate + language sync. Deliberately excludes the desktop-only
 * syncs: WorkspaceModeSync would redirect `/` to a route these shells don't
 * mount, PluginThemeSync is a no-op without the Tauri plugin host, and the
 * monaco rollback preload must stay out of non-desktop bundles (which is also
 * why this is a separate module rather than a variant prop on the desktop
 * App — importing App.tsx would pull the workspace preload chunk into the
 * build graph).
 */
export function WebApp() {
  return (
    <ErrorBoundary>
      <TooltipProvider>
        <QueryClientProvider client={queryClient}>
          <SetupGuard />
          <AppLanguageSync />
          <Outlet />
          <Toaster />
        </QueryClientProvider>
      </TooltipProvider>
    </ErrorBoundary>
  );
}
