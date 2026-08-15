import { Outlet, useLocation } from "react-router-dom"

import { WorkspaceStage } from "@slab/components/workspace"
import { ErrorBoundary } from "@slab/ui/components/error-boundary"
import FooterStatusBar from "@slab/ui/layouts/footer-status-bar"
import Header, { DEFAULT_HEADER_META } from "@slab/ui/layouts/header"
import { HeaderProvider } from "@slab/ui/layouts/header-provider"
import { AppSidebar } from "@slab/ui/layouts/sidebar"
import { getHeaderMetaForPath } from "@slab/ui/routes/route-meta"
import type { DesktopRouteObject } from "@slab/ui/routes/route-meta"

type LayoutProps = {
  routes: readonly DesktopRouteObject[]
}

export default function Layout({ routes }: LayoutProps) {
  const location = useLocation()
  const { pathname } = location
  const headerMeta = getHeaderMetaForPath(pathname, DEFAULT_HEADER_META, routes)
  const isChatShell = pathname === "/"

  return (
    <div className="workspace-shell flex h-screen min-h-0 w-full flex-col overflow-hidden">
      <HeaderProvider defaultMeta={headerMeta}>
        <div className="flex min-h-0 w-full flex-1">
          <AppSidebar routes={routes} variant={isChatShell ? "chat" : "default"} />
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <Header />
            <WorkspaceStage
              className="min-h-0 flex-1 !rounded-none !border-0 !bg-transparent !shadow-none"
            >
              <main
                className={
                  isChatShell
                    ? "flex min-h-0 flex-1 overflow-hidden bg-[var(--shell-card)] p-0"
                    : "flex min-h-0 flex-1 overflow-hidden bg-[var(--shell-card)] px-[var(--shell-content-gutter)] pb-[var(--shell-content-gutter)] pt-4"
                }
              >
                <ErrorBoundary key={location.key} variant="page">
                  <div className="min-h-0 flex-1 flex">
                    <Outlet />
                  </div>
                </ErrorBoundary>
              </main>
            </WorkspaceStage>
          </div>
        </div>
        <FooterStatusBar variant={isChatShell ? "chat" : "default"} />
      </HeaderProvider>
    </div>
  )
}
