import { Outlet, useLocation } from "react-router-dom"

import { WorkspaceStage } from "@slab/components/workspace"
import FooterStatusBar from "@/layouts/footer-status-bar"
import { GlobalHeaderProvider } from "@/layouts/global-header-provider"
import Header from "@/layouts/header"
import { AppSidebar } from "@/layouts/sidebar"
import { cn } from "@/lib/utils"

export default function Layout() {
  const { pathname } = useLocation()
  const isChatShell = pathname === "/"

  return (
    <div className="workspace-shell flex h-screen min-h-0 w-full flex-col overflow-hidden">
      <GlobalHeaderProvider>
        <div className="flex min-h-0 w-full flex-1">
          <AppSidebar variant={isChatShell ? "chat" : "default"} />
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <Header variant={isChatShell ? "chat" : "default"} />
            <WorkspaceStage
              className="min-h-0 flex-1 !rounded-none !border-0 !bg-transparent !shadow-none"
            >
              <main
                className={cn(
                  "flex min-h-0 flex-1 overflow-hidden bg-[var(--shell-card)]",
                  isChatShell
                    ? "p-0"
                    : "px-[var(--shell-content-gutter)] pb-[var(--shell-content-gutter)] pt-4"
                )}
              >
                <Outlet />
              </main>
            </WorkspaceStage>
          </div>
        </div>
        <FooterStatusBar variant={isChatShell ? "chat" : "default"} />
      </GlobalHeaderProvider>
    </div>
  )
}
